use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use repopact_graph::{GraphEdge, RepositoryGraph};
use repopact_repository::{path_string, Repository, RepositorySnapshot};
use repopact_types::{
    hex_digest, AcceptanceCriterion, LifecycleStatus, PathState, ReadFact, ReadSet,
    RepositoryIdentity, Severity, WorkItem,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateWorkItem {
    pub title: String,
    pub status: String,
    pub date: String,
    pub owner_scope: String,
    #[serde(default)]
    pub affected_scopes: Vec<String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default = "default_concrete")]
    pub provenance: String,
    #[serde(default)]
    pub acceptance_criteria: Vec<AcceptanceCriterion>,
    /// Explicit operator attestation required by the repository's mandatory
    /// preflight contract. Missing values fail closed for older callers.
    #[serde(default)]
    pub preflight_confirmed_before_work_started: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

impl CreateWorkItem {
    pub fn new(title: impl Into<String>, date: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            status: "active".to_owned(),
            date: date.into(),
            owner_scope: "governance".to_owned(),
            affected_scopes: Vec::new(),
            depends_on: Vec::new(),
            provenance: "concrete".to_owned(),
            acceptance_criteria: Vec::new(),
            preflight_confirmed_before_work_started: false,
            idempotency_key: None,
        }
    }

    pub fn with_status(mut self, status: impl Into<String>) -> Self {
        self.status = status.into();
        self
    }
    pub fn with_owner_scope(mut self, owner_scope: impl Into<String>) -> Self {
        self.owner_scope = owner_scope.into();
        self
    }
    pub fn with_affected_scopes(mut self, scopes: Vec<String>) -> Self {
        self.affected_scopes = scopes;
        self
    }
    pub fn with_dependencies(mut self, dependencies: Vec<String>) -> Self {
        self.depends_on = dependencies;
        self
    }
    pub fn with_acceptance_criteria(mut self, criteria: Vec<AcceptanceCriterion>) -> Self {
        self.acceptance_criteria = criteria;
        self
    }
    pub fn confirm_preflight_before_work_started(mut self) -> Self {
        self.preflight_confirmed_before_work_started = true;
        self
    }
    pub fn with_idempotency_key(mut self, key: impl Into<String>) -> Self {
        self.idempotency_key = Some(key.into());
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkItemEdits {
    pub title: Option<String>,
    pub owner_scope: Option<String>,
    pub affected_scopes: Option<Vec<String>>,
    pub depends_on: Option<Vec<String>>,
    pub provenance: Option<String>,
    pub acceptance_criteria: Option<Vec<AcceptanceCriterion>>,
}

impl WorkItemEdits {
    pub fn title(title: impl Into<String>) -> Self {
        Self {
            title: Some(title.into()),
            ..Self::default()
        }
    }
    pub fn owner_scope(owner_scope: impl Into<String>) -> Self {
        Self {
            owner_scope: Some(owner_scope.into()),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditWorkItem {
    pub id: String,
    pub changes: WorkItemEdits,
    pub date: String,
}

impl EditWorkItem {
    pub fn new(id: impl Into<String>, changes: WorkItemEdits, date: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            changes,
            date: date.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionWorkItem {
    pub id: String,
    pub status: String,
}

impl TransitionWorkItem {
    pub fn new(id: impl Into<String>, status: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            status: status.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MutationRequest {
    CreateWorkItem(CreateWorkItem),
    EditWorkItem(EditWorkItem),
    TransitionWorkItem(TransitionWorkItem),
}

impl From<CreateWorkItem> for MutationRequest {
    fn from(request: CreateWorkItem) -> Self {
        Self::CreateWorkItem(request)
    }
}

impl From<EditWorkItem> for MutationRequest {
    fn from(request: EditWorkItem) -> Self {
        Self::EditWorkItem(request)
    }
}

impl From<TransitionWorkItem> for MutationRequest {
    fn from(request: TransitionWorkItem) -> Self {
        Self::TransitionWorkItem(request)
    }
}

impl MutationRequest {
    pub fn create_work_item(request: CreateWorkItem) -> Self {
        Self::CreateWorkItem(request)
    }
    pub fn edit_work_item(request: EditWorkItem) -> Self {
        Self::EditWorkItem(request)
    }
    pub fn transition_work_item(request: TransitionWorkItem) -> Self {
        Self::TransitionWorkItem(request)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PathOperation {
    Write { path: String, content: String },
    Move { from: String, to: String },
}

pub type FileOperation = PathOperation;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratedImpact {
    pub path: String,
    pub before_digest: String,
    pub after_digest: String,
    pub preview: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutationDiagnostic {
    pub code: String,
    pub message: String,
    pub path: Option<String>,
    pub blocking: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related_records: Vec<String>,
}

impl MutationDiagnostic {
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            path: None,
            blocking: true,
            related_records: Vec::new(),
        }
    }

    pub fn at(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutationPlan {
    pub repository_identity: RepositoryIdentity,
    pub read_set: ReadSet,
    pub plan_token: String,
    pub request: MutationRequest,
    pub file_operations: Vec<PathOperation>,
    pub generated_impacts: Vec<GeneratedImpact>,
    pub graph_impacts: Vec<GraphEdge>,
    pub diagnostics: Vec<MutationDiagnostic>,
    pub preview: String,
    #[serde(default)]
    pub metrics: MutationMetrics,
}

impl MutationPlan {
    pub fn is_applicable(&self) -> bool {
        self.diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.blocking)
    }
    pub fn has_errors(&self) -> bool {
        !self.is_applicable()
    }

    pub fn apply(&self) -> MutationResult {
        apply(self, &ApplyOptions::default())
    }

    pub fn apply_with_options(&self, options: &ApplyOptions) -> MutationResult {
        apply(self, options)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplyOptions {
    pub fail_before_operation: Option<usize>,
    pub fail_after_operation: Option<usize>,
    pub external_drift_at: Option<usize>,
    pub external_drift_path: Option<String>,
}

impl ApplyOptions {
    pub fn fail_after(operation: usize) -> Self {
        Self {
            fail_after_operation: Some(operation),
            ..Self::default()
        }
    }
    pub fn fail_before(operation: usize) -> Self {
        Self {
            fail_before_operation: Some(operation),
            ..Self::default()
        }
    }
    pub fn external_drift(operation: usize, path: impl Into<String>) -> Self {
        Self {
            external_drift_at: Some(operation),
            external_drift_path: Some(path.into()),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutationResult {
    pub success: bool,
    pub rolled_back: bool,
    pub plan_token: String,
    pub changed_paths: Vec<String>,
    pub diagnostics: Vec<MutationDiagnostic>,
    #[serde(default)]
    pub replayed: bool,
    #[serde(default)]
    pub metrics: MutationMetrics,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutationMetrics {
    pub snapshot_ms: u128,
    pub planning_ms: u128,
    pub engine_execution_ms: u128,
    pub proposal_lock_wait_ms: u128,
    pub idempotency_lookup_ms: u128,
    pub stale_check_ms: u128,
    pub critical_validation_ms: u128,
    pub authority_checks_ms: u128,
    pub work_schema_ms: u128,
    pub dependency_graph_ms: u128,
    pub maintenance_diagnostics_ms: u128,
    pub filesystem_staging_ms: u128,
    pub atomic_commit_ms: u128,
    pub dashboard_regeneration_ms: u128,
    pub records_examined: usize,
    pub source_files_read: usize,
    pub external_integrations_ms: u128,
}

pub fn plan(snapshot: &RepositorySnapshot, request: MutationRequest) -> MutationPlan {
    plan_at(snapshot, request, SystemTime::now())
}

/// Plan against a caller-supplied clock instant. Production callers use
/// `plan`; deterministic tests can verify the exact UTC preflight timestamp.
pub fn plan_at(
    snapshot: &RepositorySnapshot,
    request: MutationRequest,
    now: SystemTime,
) -> MutationPlan {
    let repository = snapshot.repository();
    let identity = snapshot.identity();
    let mut diagnostics = Vec::new();
    let mut operations = Vec::new();
    let mut extra_absent = Vec::new();
    let mut metrics = MutationMetrics::default();
    let request_for_graph = request.clone();

    if has_wi050_surface(repository.root()) {
        diagnostics.push(MutationDiagnostic::error(
            "mutation.unsupported-wi050",
            "WI050 admission/guard/enforcement semantics are outside the WI054 native mutation boundary",
        ));
        return empty_plan(snapshot, request, diagnostics);
    }

    match &request {
        MutationRequest::CreateWorkItem(request) => {
            if !request.preflight_confirmed_before_work_started {
                diagnostics.push(MutationDiagnostic::error(
                    "mutation.preflight-unconfirmed",
                    "operator confirmation is required to record that this work item was registered before implementation began",
                ));
                return empty_plan(snapshot, request.clone(), diagnostics);
            }
            let Some(status) = LifecycleStatus::parse(&request.status) else {
                diagnostics.push(MutationDiagnostic::error(
                    "mutation.status-invalid",
                    format!("unknown work-item status '{}'", request.status),
                ));
                return empty_plan(snapshot, request.clone(), diagnostics);
            };
            let id = format!("{:03}", next_work_id(snapshot.index()));
            let slug = slugify(&request.title);
            let directory = format!("work/{}/{id}-{slug}", status.as_str());
            let manifest_path = format!("{directory}/work-item.json");
            let readme_path = format!("{directory}/README.md");
            let created_at = match format_utc_timestamp(now) {
                Ok(timestamp) => timestamp,
                Err(error) => {
                    diagnostics.push(MutationDiagnostic::error(
                        "mutation.preflight-time-invalid",
                        error,
                    ));
                    return empty_plan(snapshot, request.clone(), diagnostics);
                }
            };
            let (manifest, _) =
                create_manifest(request, &id, status.as_str(), repository, &created_at);
            operations.push(PathOperation::Write {
                path: manifest_path.clone(),
                content: manifest,
            });
            operations.push(PathOperation::Write {
                path: readme_path.clone(),
                content: create_readme(request, &id, status.as_str()),
            });
            extra_absent.extend([directory, manifest_path, readme_path]);
        }
        MutationRequest::EditWorkItem(request) => {
            let Some(record) = snapshot.index().work_item(&request.id) else {
                diagnostics.push(MutationDiagnostic::error(
                    "mutation.work-not-found",
                    format!("unknown work item '{}'", request.id),
                ));
                return empty_plan(snapshot, request.clone(), diagnostics);
            };
            match edited_manifest(record, &request.changes, &request.date) {
                Ok(content) => operations.push(PathOperation::Write {
                    path: repository.relative_path(&record.path),
                    content,
                }),
                Err(error) => diagnostics.push(
                    MutationDiagnostic::error("mutation.edit-invalid", error)
                        .at(repository.relative_path(&record.path)),
                ),
            }
        }
        MutationRequest::TransitionWorkItem(request) => {
            let Some(status) = LifecycleStatus::parse(&request.status) else {
                diagnostics.push(MutationDiagnostic::error(
                    "mutation.status-invalid",
                    format!("unknown work-item status '{}'", request.status),
                ));
                return empty_plan(snapshot, request.clone(), diagnostics);
            };
            let Some(record) = snapshot.index().work_item(&request.id) else {
                diagnostics.push(MutationDiagnostic::error(
                    "mutation.work-not-found",
                    format!("unknown work item '{}'", request.id),
                ));
                return empty_plan(snapshot, request.clone(), diagnostics);
            };
            let Some(item) = record
                .value
                .clone()
                .ok()
                .and_then(|value| serde_json::from_value::<WorkItem>(value).ok())
            else {
                diagnostics.push(
                    MutationDiagnostic::error(
                        "mutation.work-invalid",
                        "cannot transition a work item whose JSON is not valid".to_owned(),
                    )
                    .at(repository.relative_path(&record.path)),
                );
                return empty_plan(snapshot, request.clone(), diagnostics);
            };
            let Some(source_directory) = record.path.parent() else {
                return empty_plan(snapshot, request.clone(), diagnostics);
            };
            let Some(source_status) = source_directory
                .parent()
                .and_then(Path::file_name)
                .and_then(|name| name.to_str())
            else {
                return empty_plan(snapshot, request.clone(), diagnostics);
            };
            let Some(name) = source_directory.file_name().and_then(|name| name.to_str()) else {
                return empty_plan(snapshot, request.clone(), diagnostics);
            };
            let destination_directory = repository
                .root()
                .join("work")
                .join(status.as_str())
                .join(name);
            let source_relative = repository.relative_path(source_directory);
            let destination_relative = repository.relative_path(&destination_directory);
            if source_status == status.as_str() {
                let mut value = record.value.clone().unwrap_or(Value::Null);
                if let Some(object) = value.as_object_mut() {
                    object.insert(
                        "status".to_owned(),
                        Value::String(status.as_str().to_owned()),
                    );
                }
                operations.push(PathOperation::Write {
                    path: repository.relative_path(&record.path),
                    content: canonical_json(&value),
                });
            } else {
                let mut value = record.value.clone().unwrap_or(Value::Null);
                if let Some(object) = value.as_object_mut() {
                    object.insert(
                        "status".to_owned(),
                        Value::String(status.as_str().to_owned()),
                    );
                }
                operations.push(PathOperation::Move {
                    from: source_relative.clone(),
                    to: destination_relative.clone(),
                });
                operations.push(PathOperation::Write {
                    path: format!("{destination_relative}/work-item.json"),
                    content: canonical_json(&value),
                });
                extra_absent.push(destination_relative);
            }
            let _ = item;
        }
    }

    if diagnostics.iter().any(|diagnostic| diagnostic.blocking) {
        return empty_plan(snapshot, request.clone(), diagnostics);
    }

    let proposal = match &request {
        MutationRequest::CreateWorkItem(item) if item.status == "proposed" => Some(item),
        _ => None,
    };
    if let Some(proposal) = proposal {
        let Some(key) = proposal.idempotency_key.as_deref() else {
            diagnostics.push(MutationDiagnostic::error(
                "work.proposal-key-required",
                "work.propose requires an idempotency key so retries can be resolved safely",
            ));
            return empty_plan(snapshot, request, diagnostics);
        };
        if key.trim().is_empty() || key.trim() != key || key.len() > 128 {
            diagnostics.push(MutationDiagnostic::error(
                "work.proposal-key-invalid",
                "proposal idempotency key must contain 1 to 128 non-whitespace characters",
            ));
            return empty_plan(snapshot, request, diagnostics);
        }
        let Some((manifest_path, manifest)) =
            operations.iter().find_map(|operation| match operation {
                PathOperation::Write { path, content } if path.ends_with("/work-item.json") => {
                    Some((path.clone(), content.clone()))
                }
                _ => None,
            })
        else {
            diagnostics.push(MutationDiagnostic::error(
                "work.proposal-candidate-missing",
                "proposal plan did not produce a work-item manifest",
            ));
            return empty_plan(snapshot, request, diagnostics);
        };
        let Some((_, readme)) = operations.iter().find_map(|operation| match operation {
            PathOperation::Write { path, content } if path.ends_with("/README.md") => {
                Some((path.clone(), content.clone()))
            }
            _ => None,
        }) else {
            diagnostics.push(MutationDiagnostic::error(
                "work.proposal-readme-missing",
                "proposal plan did not produce a narrative README",
            ));
            return empty_plan(snapshot, request, diagnostics);
        };
        let manifest_value = match serde_json::from_str::<Value>(&manifest) {
            Ok(value) => value,
            Err(error) => {
                diagnostics.push(MutationDiagnostic::error(
                    "work.proposal-manifest-invalid",
                    format!("generated proposal manifest is not valid JSON: {error}"),
                ));
                return empty_plan(snapshot, request, diagnostics);
            }
        };
        let directory = Path::new(&manifest_path)
            .parent()
            .map(path_string)
            .unwrap_or_default();
        let receipt = proposal_receipt(proposal, key, &manifest_value);
        let receipt_path = format!("{directory}/proposal-receipt.json");
        operations.push(PathOperation::Write {
            path: receipt_path.clone(),
            content: canonical_json(&receipt),
        });
        extra_absent.push(receipt_path);
        let validation_started = Instant::now();
        let (validation, validation_metrics) =
            repopact_validation::validate_proposal_candidate_detailed(
                snapshot,
                repository.root().join(&manifest_path),
                manifest_value,
                readme,
            );
        metrics.critical_validation_ms = validation_started.elapsed().as_millis();
        metrics.authority_checks_ms = validation_metrics.authority_ms;
        metrics.work_schema_ms = validation_metrics.work_schema_ms;
        metrics.dependency_graph_ms = validation_metrics.dependency_graph_ms;
        metrics.maintenance_diagnostics_ms = validation_metrics.maintenance_ms;
        metrics.records_examined = validation_metrics.records_examined;
        metrics.source_files_read = validation_metrics.source_files_read;
        for diagnostic in validation.diagnostics {
            diagnostics.push(MutationDiagnostic {
                code: diagnostic.code,
                message: diagnostic.message,
                path: diagnostic.path,
                blocking: diagnostic.severity == Severity::Error,
                related_records: diagnostic.related_records.unwrap_or_default(),
            });
        }
    } else {
        let dashboard = validate_candidate(repository.root(), &operations);
        match dashboard {
            Ok((content, candidate_diagnostics)) => {
                operations.push(PathOperation::Write {
                    path: "audits/reports/dashboard.md".to_owned(),
                    content: platform_text(&content),
                });
                diagnostics.extend(candidate_diagnostics);
            }
            Err(error) => diagnostics.push(MutationDiagnostic::error(
                "mutation.dashboard-render",
                error,
            )),
        }
    }
    let mut read_facts = snapshot.read_set().facts.clone();
    for relative in extra_absent {
        let path = repository.root().join(&relative);
        read_facts.push(ReadFact {
            path: relative,
            expected: repository.path_state(&path),
        });
    }
    let read_set = ReadSet::new(read_facts);
    let graph_impacts = if matches!(request, MutationRequest::CreateWorkItem(_)) {
        Vec::new()
    } else {
        let graph = RepositoryGraph::build(snapshot);
        graph_impacts(&graph, &request_for_graph)
    };
    let mut plan = MutationPlan {
        repository_identity: identity.clone(),
        plan_token: read_set.token(&identity),
        read_set,
        request,
        generated_impacts: impacts(repository, &operations),
        preview: preview(&operations),
        file_operations: operations,
        graph_impacts,
        diagnostics,
        metrics,
    };
    add_frozen_diagnostics(snapshot, &mut plan);
    plan.diagnostics.sort_by(|left, right| {
        (
            left.code.as_str(),
            left.path.as_deref(),
            left.message.as_str(),
        )
            .cmp(&(
                right.code.as_str(),
                right.path.as_deref(),
                right.message.as_str(),
            ))
    });
    plan
}

pub fn plan_mutation(snapshot: &RepositorySnapshot, request: MutationRequest) -> MutationPlan {
    plan(snapshot, request)
}

/// Look up the durable result of a proposal request without changing the
/// repository. `None` means the key has not reached the atomic commit point.
pub fn proposal_status(root: impl AsRef<Path>, key: &str) -> Result<Option<Value>, String> {
    validate_idempotency_key(key)?;
    read_proposal_receipt(root.as_ref(), key)
}

/// Create a proposal from one authoritative snapshot while holding the
/// cross-process proposal lock. The work directory rename is the commit point.
pub fn propose_work_item(root: impl AsRef<Path>, mut request: CreateWorkItem) -> MutationResult {
    request.status = "proposed".to_owned();
    let mut result = empty_result();
    let Some(key) = request.idempotency_key.as_deref() else {
        result.diagnostics.push(MutationDiagnostic::error(
            "work.proposal-key-required",
            "work.propose requires an idempotency key so retries can be resolved safely",
        ));
        return result;
    };
    if let Err(error) = validate_idempotency_key(key) {
        result.diagnostics.push(MutationDiagnostic::error(
            "work.proposal-key-invalid",
            error,
        ));
        return result;
    }
    if !request.preflight_confirmed_before_work_started {
        result.diagnostics.push(MutationDiagnostic::error(
            "mutation.preflight-unconfirmed",
            "explicit operator confirmation is required before recording proposed work",
        ));
        return result;
    }

    let repository = Repository::open(root);
    let lock_started = Instant::now();
    let _lock = match ProposalLock::acquire(repository.root()) {
        Ok(lock) => lock,
        Err(error) => {
            result.metrics.proposal_lock_wait_ms = lock_started.elapsed().as_millis();
            result.diagnostics.push(MutationDiagnostic::error(
                "work.proposal-lock-timeout",
                error,
            ));
            return result;
        }
    };
    result.metrics.proposal_lock_wait_ms = lock_started.elapsed().as_millis();
    let cleanup_started = Instant::now();
    if let Err(error) = ensure_safe_repository_directory(repository.root(), Path::new("work")) {
        result.metrics.filesystem_staging_ms += cleanup_started.elapsed().as_millis();
        result.diagnostics.push(MutationDiagnostic::error(
            "work.proposal-stage-cleanup-failed",
            error,
        ));
        return result;
    }
    if let Err(error) = clean_stale_proposal_staging(repository.root()) {
        result.metrics.filesystem_staging_ms += cleanup_started.elapsed().as_millis();
        result.diagnostics.push(MutationDiagnostic::error(
            "work.proposal-stage-cleanup-failed",
            error,
        ));
        return result;
    }
    result.metrics.filesystem_staging_ms += cleanup_started.elapsed().as_millis();
    let receipt_started = Instant::now();
    let receipt = resolve_proposal_receipt(repository.root(), key, &request, &mut result);
    result.metrics.idempotency_lookup_ms = receipt_started.elapsed().as_millis();
    match receipt {
        Ok(false) => return result,
        Ok(true) => {}
        Err(error) => {
            result.diagnostics.push(MutationDiagnostic::error(
                "work.proposal-receipt-read-failed",
                error,
            ));
            return result;
        }
    }
    let snapshot_started = Instant::now();
    let snapshot = repository.session().snapshot();
    result.metrics.snapshot_ms = snapshot_started.elapsed().as_millis();
    let planning_started = Instant::now();
    let plan = plan(&snapshot, MutationRequest::create_work_item(request));
    result.metrics.planning_ms = planning_started.elapsed().as_millis();
    result.metrics.critical_validation_ms = plan.metrics.critical_validation_ms;
    result.metrics.authority_checks_ms = plan.metrics.authority_checks_ms;
    result.metrics.work_schema_ms = plan.metrics.work_schema_ms;
    result.metrics.dependency_graph_ms = plan.metrics.dependency_graph_ms;
    result.metrics.maintenance_diagnostics_ms = plan.metrics.maintenance_diagnostics_ms;
    result.metrics.records_examined = plan.metrics.records_examined;
    result.metrics.source_files_read = plan.metrics.source_files_read;
    result.plan_token = plan.plan_token.clone();
    result.diagnostics = plan.diagnostics.clone();
    if !plan.is_applicable() {
        return result;
    }
    commit_proposal_locked(
        &repository,
        &plan,
        &snapshot,
        &ApplyOptions::default(),
        result,
    )
}

fn empty_result() -> MutationResult {
    MutationResult {
        success: false,
        rolled_back: false,
        plan_token: String::new(),
        changed_paths: Vec::new(),
        diagnostics: Vec::new(),
        replayed: false,
        metrics: MutationMetrics::default(),
    }
}

fn validate_idempotency_key(key: &str) -> Result<(), String> {
    if key.is_empty() || key.trim() != key || key.len() > 128 {
        Err("proposal idempotency key must contain 1 to 128 non-whitespace characters".to_owned())
    } else {
        Ok(())
    }
}

fn proposal_payload_digest(request: &CreateWorkItem) -> String {
    let payload = serde_json::json!({
        "title": request.title,
        "owner_scope": request.owner_scope,
        "affected_scopes": request.affected_scopes,
        "depends_on": request.depends_on,
        "provenance": request.provenance,
        "acceptance_criteria": request.acceptance_criteria,
    });
    let bytes = serde_json::to_vec(&payload).unwrap_or_default();
    hex_digest(&Sha256::digest(bytes))
}

fn proposal_receipt(request: &CreateWorkItem, key: &str, manifest: &Value) -> Value {
    let id = manifest
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    serde_json::json!({
        "format_version": 1,
        "idempotency_key": key,
        "request_digest": proposal_payload_digest(request),
        "work_item_id": id,
        "work_item_path": format!("work/proposed/{id}-{}", slugify(&request.title)),
    })
}

fn resolve_proposal_receipt(
    root: &Path,
    key: &str,
    request: &CreateWorkItem,
    result: &mut MutationResult,
) -> Result<bool, String> {
    let Some(receipt) = read_proposal_receipt(root, key)? else {
        return Ok(true);
    };
    let expected = proposal_payload_digest(request);
    if receipt.get("request_digest").and_then(Value::as_str) != Some(expected.as_str()) {
        result.diagnostics.push(MutationDiagnostic::error(
            "work.proposal-key-conflict",
            "this idempotency key is already bound to a different proposal payload",
        ));
        return Ok(false);
    }
    let path = receipt
        .get("current_work_item_path")
        .or_else(|| receipt.get("work_item_path"))
        .and_then(Value::as_str)
        .unwrap_or("work/proposed");
    result.success = true;
    result.replayed = true;
    result.changed_paths = vec![format!("{path}/work-item.json")];
    Ok(false)
}

fn read_proposal_receipt(root: &Path, key: &str) -> Result<Option<Value>, String> {
    let mut found: Option<Value> = None;
    let mut visited = 0usize;
    for status in LifecycleStatus::ALL {
        let directory = root.join("work").join(status);
        let metadata = match fs::symlink_metadata(&directory) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(format!(
                    "unable to inspect {} work directory: {error}",
                    status
                ))
            }
        };
        if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
            return Err(format!("{} work path is not a regular directory", status));
        }
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(format!("unable to inspect {} work items: {error}", status)),
        };
        for entry in entries {
            let entry =
                entry.map_err(|error| format!("unable to enumerate work items: {error}"))?;
            if !entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                continue;
            }
            visited += 1;
            if visited > 100_000 {
                return Err(
                    "work tree exceeds the 100000-directory proposal lookup bound".to_owned(),
                );
            }
            let path = entry.path().join("proposal-receipt.json");
            let metadata = match fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => {
                    return Err(format!(
                        "unable to inspect proposal receipt {}: {error}",
                        path.display()
                    ))
                }
            };
            if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
                return Err(format!(
                    "proposal receipt {} is not a regular file",
                    path.display()
                ));
            }
            if metadata.len() > 65_536 {
                return Err(format!(
                    "proposal receipt {} exceeds the 64 KiB read bound",
                    path.display()
                ));
            }
            let bytes = fs::read(&path).map_err(|error| {
                format!(
                    "unable to read proposal receipt {}: {error}",
                    path.display()
                )
            })?;
            let value: Value = serde_json::from_slice(&bytes)
                .map_err(|error| format!("invalid proposal receipt {}: {error}", path.display()))?;
            if value.get("idempotency_key").and_then(Value::as_str) == Some(key) {
                let item_path = entry.path().join("work-item.json");
                let item_metadata = fs::symlink_metadata(&item_path).map_err(|error| {
                    format!(
                        "proposal receipt {} has no regular work-item record: {error}",
                        path.display()
                    )
                })?;
                if !item_metadata.file_type().is_file() || item_metadata.file_type().is_symlink() {
                    return Err(format!(
                        "proposal receipt {} has no regular work-item record",
                        path.display()
                    ));
                }
                if found.is_some() {
                    return Err(format!(
                        "idempotency key '{key}' appears in more than one proposal receipt"
                    ));
                }
                let mut value = value;
                if let Some(object) = value.as_object_mut() {
                    let item_directory = entry.path();
                    let relative = item_directory
                        .strip_prefix(root)
                        .map(path_string)
                        .unwrap_or_else(|_| path_string(&item_directory));
                    object.insert("current_work_item_path".to_owned(), Value::String(relative));
                }
                found = Some(value);
            }
        }
    }
    Ok(found)
}

struct ProposalLock {
    file: File,
    #[cfg(windows)]
    overlapped: windows_sys::Win32::System::IO::OVERLAPPED,
}

impl ProposalLock {
    fn acquire(root: &Path) -> Result<Self, String> {
        let identity = root.to_string_lossy().to_ascii_lowercase();
        let key = hex_digest(Sha256::digest(identity.as_bytes()));
        let directory = std::env::temp_dir().join("repopact-proposal-locks");
        fs::create_dir_all(&directory)
            .map_err(|error| format!("unable to prepare proposal lock directory: {error}"))?;
        let path = directory.join(format!("{key}.lock"));
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|error| format!("unable to open proposal lock: {error}"))?;
        let deadline = Instant::now() + Duration::from_secs(2);
        #[cfg(windows)]
        let mut overlapped = windows_sys::Win32::System::IO::OVERLAPPED::default();
        loop {
            match try_lock_file(
                &file,
                #[cfg(windows)]
                &mut overlapped,
            ) {
                Ok(()) => {
                    return Ok(Self {
                        file,
                        #[cfg(windows)]
                        overlapped,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        return Err("proposal lock did not clear within 2000 ms; inspect proposal-status before retrying".to_owned());
                    }
                    thread::sleep(Duration::from_millis(20));
                }
                Err(error) => return Err(format!("unable to acquire proposal lock: {error}")),
            }
        }
    }
}

impl Drop for ProposalLock {
    fn drop(&mut self) {
        unlock_file(
            &self.file,
            #[cfg(windows)]
            &mut self.overlapped,
        );
    }
}

#[cfg(unix)]
fn try_lock_file(file: &File) -> std::io::Result<()> {
    use std::os::fd::AsRawFd;
    let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(unix)]
fn unlock_file(file: &File) {
    use std::os::fd::AsRawFd;
    let _ = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_UN) };
}

#[cfg(windows)]
fn try_lock_file(
    file: &File,
    overlapped: &mut windows_sys::Win32::System::IO::OVERLAPPED,
) -> std::io::Result<()> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        LockFileEx, LOCKFILE_EXCLUSIVE_LOCK, LOCKFILE_FAIL_IMMEDIATELY,
    };
    let result = unsafe {
        LockFileEx(
            file.as_raw_handle() as windows_sys::Win32::Foundation::HANDLE,
            LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY,
            0,
            1,
            0,
            overlapped,
        )
    };
    if result != 0 {
        Ok(())
    } else {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(33) {
            Err(std::io::Error::from(std::io::ErrorKind::WouldBlock))
        } else {
            Err(error)
        }
    }
}

#[cfg(windows)]
fn unlock_file(file: &File, overlapped: &mut windows_sys::Win32::System::IO::OVERLAPPED) {
    use std::os::windows::io::AsRawHandle;
    let _ = unsafe {
        windows_sys::Win32::Storage::FileSystem::UnlockFileEx(
            file.as_raw_handle() as windows_sys::Win32::Foundation::HANDLE,
            0,
            1,
            0,
            overlapped,
        )
    };
}

fn clean_stale_proposal_staging(root: &Path) -> Result<(), String> {
    let directory = root.join("work/.repopact-transactions");
    match fs::symlink_metadata(&directory) {
        Ok(metadata) if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() => {}
        Ok(_) => return Err("proposal staging path is not a regular directory".to_owned()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("unable to inspect proposal staging: {error}")),
    }
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("unable to inspect proposal staging: {error}")),
    };
    let mut removed = 0usize;
    for entry in entries {
        let entry =
            entry.map_err(|error| format!("unable to enumerate proposal staging: {error}"))?;
        if entry
            .file_name()
            .to_string_lossy()
            .starts_with(".repopact-stage-")
            && entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false)
        {
            fs::remove_dir_all(entry.path())
                .map_err(|error| format!("unable to remove stale proposal stage: {error}"))?;
            removed += 1;
            if removed > 1000 {
                return Err("proposal staging cleanup exceeded its 1000-directory bound".to_owned());
            }
        }
    }
    Ok(())
}

fn empty_plan(
    snapshot: &RepositorySnapshot,
    request: impl Into<MutationRequest>,
    diagnostics: Vec<MutationDiagnostic>,
) -> MutationPlan {
    let request = request.into();
    let identity = snapshot.identity();
    let read_set = snapshot.read_set().clone();
    MutationPlan {
        repository_identity: identity.clone(),
        read_set: read_set.clone(),
        plan_token: read_set.token(&identity),
        request,
        file_operations: Vec::new(),
        generated_impacts: Vec::new(),
        graph_impacts: Vec::new(),
        diagnostics,
        preview: String::new(),
        metrics: MutationMetrics::default(),
    }
}

fn create_manifest(
    request: &CreateWorkItem,
    id: &str,
    status: &str,
    repository: &Repository,
    created_at: &str,
) -> (String, WorkItem) {
    let criteria = if request.acceptance_criteria.is_empty() {
        // Preserve the long-standing headless `repopact new work-item`
        // starter record. The placeholder is explicit and must be replaced
        // with a verifiable outcome before work begins.
        vec![CanonicalCriterion {
            id: "AC-1".to_owned(),
            text: "TODO: define a verifiable observable outcome before implementation".to_owned(),
            state: "pending".to_owned(),
            evidence: Vec::new(),
            provenance: None,
        }]
    } else {
        request
            .acceptance_criteria
            .iter()
            .map(CanonicalCriterion::from)
            .collect()
    };
    let marker = repopact_types::PreflightMarker {
        created_before_work_started: request.preflight_confirmed_before_work_started,
        created_at: created_at.to_owned(),
        note: "The caller recorded the operator's preflight confirmation through the RepoPact work-item registration flow."
            .to_owned(),
    };
    let manifest = CanonicalWorkItem {
        schema: if repository.root().join("repopact/schemas").is_dir() {
            "../../../repopact/schemas/work-item.schema.json".to_owned()
        } else {
            "../../../schemas/work-item.schema.json".to_owned()
        },
        id: id.to_owned(),
        title: request.title.clone(),
        status: status.to_owned(),
        owner_scope: request.owner_scope.clone(),
        affected_scopes: request.affected_scopes.clone(),
        depends_on: request.depends_on.clone(),
        provenance: request.provenance.clone(),
        preflight: marker,
        acceptance_criteria: criteria,
        created: request.date.clone(),
        updated: request.date.clone(),
    };
    let value = serde_json::to_value(&manifest).unwrap_or(Value::Null);
    let typed = serde_json::from_value(value.clone()).unwrap_or_else(|_| WorkItem {
        id: id.to_owned(),
        title: request.title.clone(),
        status: status.to_owned(),
        owner_scope: request.owner_scope.clone(),
        affected_scopes: request.affected_scopes.clone(),
        depends_on: request.depends_on.clone(),
        provenance: request.provenance.clone(),
        preflight: Some(manifest.preflight.clone()),
        documentation_impact: None,
        acceptance_criteria: request.acceptance_criteria.clone(),
        created: request.date.clone(),
        updated: request.date.clone(),
    });
    (canonical_json(&value), typed)
}

#[derive(Serialize)]
struct CanonicalWorkItem {
    #[serde(rename = "$schema")]
    schema: String,
    id: String,
    title: String,
    status: String,
    owner_scope: String,
    affected_scopes: Vec<String>,
    depends_on: Vec<String>,
    provenance: String,
    preflight: repopact_types::PreflightMarker,
    acceptance_criteria: Vec<CanonicalCriterion>,
    created: String,
    updated: String,
}

#[derive(Serialize)]
struct CanonicalCriterion {
    id: String,
    text: String,
    state: String,
    evidence: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    provenance: Option<String>,
}

impl From<&AcceptanceCriterion> for CanonicalCriterion {
    fn from(criterion: &AcceptanceCriterion) -> Self {
        Self {
            id: criterion.id.clone(),
            text: criterion.text.clone(),
            state: criterion.state.clone(),
            evidence: criterion.evidence.clone(),
            provenance: (criterion.provenance != "concrete").then(|| criterion.provenance.clone()),
        }
    }
}

fn edited_manifest(
    record: &repopact_repository::IndexedRecord,
    changes: &WorkItemEdits,
    date: &str,
) -> Result<String, String> {
    let mut value = record
        .value
        .clone()
        .map_err(|error| format!("cannot edit malformed work-item JSON: {error}"))?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| "work-item JSON must be an object".to_owned())?;
    if let Some(value) = &changes.title {
        object.insert("title".to_owned(), Value::String(value.clone()));
    }
    if let Some(value) = &changes.owner_scope {
        object.insert("owner_scope".to_owned(), Value::String(value.clone()));
    }
    if let Some(value) = &changes.affected_scopes {
        object.insert(
            "affected_scopes".to_owned(),
            serde_json::to_value(value).map_err(|error| error.to_string())?,
        );
    }
    if let Some(value) = &changes.depends_on {
        object.insert(
            "depends_on".to_owned(),
            serde_json::to_value(value).map_err(|error| error.to_string())?,
        );
    }
    if let Some(value) = &changes.provenance {
        object.insert("provenance".to_owned(), Value::String(value.clone()));
    }
    if let Some(value) = &changes.acceptance_criteria {
        object.insert(
            "acceptance_criteria".to_owned(),
            serde_json::to_value(value).map_err(|error| error.to_string())?,
        );
    }
    object.insert("updated".to_owned(), Value::String(date.to_owned()));
    Ok(canonical_json(&value))
}

fn create_readme(request: &CreateWorkItem, id: &str, status: &str) -> String {
    let scopes = if request.affected_scopes.is_empty() {
        "none declared".to_owned()
    } else {
        request.affected_scopes.join(", ")
    };
    let dependencies = if request.depends_on.is_empty() {
        "none".to_owned()
    } else {
        request.depends_on.join(", ")
    };
    let criteria = if request.acceptance_criteria.is_empty() {
        "No acceptance criteria were supplied at registration. Define verifiable outcomes before implementation.".to_owned()
    } else {
        request
            .acceptance_criteria
            .iter()
            .map(|criterion| format!("- **{}** — {}", criterion.id, criterion.text))
            .collect::<Vec<_>>()
            .join("\n")
    };
    platform_text(&format!(
        "# {id} — {}\n\n> **Status**: {status}\n> **Owner scope**: {}\n> **Affected scopes**: {scopes}\n> **Depends on**: {dependencies}\n\n## Intent\n\nNo separate intent narrative was supplied at registration. The outcome is defined by the acceptance criteria below.\n\n## Decisions\n\nNo durable decisions were recorded at registration. Record material decisions in `decisions/`.\n\n## Scope\n\nThe declared affected scopes are listed above. No finer-grained file scope was supplied at registration.\n\n## Acceptance criteria\n\n{criteria}\n\n## Closeout\n\nLink actual evidence to every acceptance criterion before marking this work complete. Then move the work-item directory to `work/completed/` and regenerate the dashboard.\n",
        request.title, request.owner_scope
    ))
}

fn validate_candidate(
    root: &Path,
    operations: &[PathOperation],
) -> Result<(String, Vec<MutationDiagnostic>), String> {
    let temporary = std::env::temp_dir().join(format!(
        "repopact-mutation-plan-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    fs::create_dir_all(&temporary).map_err(|error| error.to_string())?;
    let result = (|| {
        copy_tree(root, &temporary)?;
        for operation in operations {
            apply_operation(&temporary, operation)?;
        }
        let dashboard = repopact_validation::render_dashboard(&temporary)?;
        let dashboard_path = temporary.join("audits/reports/dashboard.md");
        fs::write(&dashboard_path, platform_text(&dashboard)).map_err(|error| error.to_string())?;
        let report = repopact_validation::validate(&temporary);
        let diagnostics = report
            .diagnostics
            .into_iter()
            .map(|diagnostic| MutationDiagnostic {
                code: diagnostic.code,
                message: diagnostic.message,
                path: diagnostic.path,
                blocking: diagnostic.severity == Severity::Error,
                related_records: diagnostic.related_records.unwrap_or_default(),
            })
            .collect();
        Ok((dashboard, diagnostics))
    })();
    let _ = fs::remove_dir_all(&temporary);
    result
}

fn format_utc_timestamp(time: SystemTime) -> Result<String, String> {
    let elapsed = time
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "system clock is before the Unix epoch".to_owned())?;
    let days = i64::try_from(elapsed.as_secs() / 86_400)
        .map_err(|_| "system clock timestamp is outside the supported range".to_owned())?;
    let seconds = elapsed.as_secs() % 86_400;
    let (year, month, day) = civil_from_days(days);
    if !(0..=9999).contains(&year) {
        return Err("system clock timestamp is outside the RFC 3339 year range".to_owned());
    }
    Ok(format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        seconds / 3600,
        (seconds % 3600) / 60,
        seconds % 60
    ))
}

fn civil_from_days(days_since_epoch: i64) -> (i64, i64, i64) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year, month, day)
}

fn copy_tree(source: &Path, destination: &Path) -> Result<(), String> {
    copy_tree_inner(source, destination, true)
}

fn copy_tree_inner(source: &Path, destination: &Path, repository_root: bool) -> Result<(), String> {
    fs::create_dir_all(destination).map_err(|error| error.to_string())?;
    let mut entries = fs::read_dir(source)
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| path_string(&left.path()).cmp(&path_string(&right.path())));
    for entry in entries {
        let name = entry.file_name().to_string_lossy().to_string();
        let from = entry.path();
        let kind = entry.file_type().map_err(|error| error.to_string())?;
        if name == "build" {
            if repository_root && kind.is_dir() {
                copy_governed_build_inputs(&from, destination)?;
            }
            continue;
        }
        if matches!(
            name.as_str(),
            ".git"
                | "target"
                | "__pycache__"
                | ".venv"
                | ".pytest_cache"
                | "node_modules"
                | "dist"
                | "worktrees"
                | "fixtures"
        ) {
            continue;
        }
        let to = destination.join(&name);
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            copy_tree_inner(&from, &to, false)?;
        } else if kind.is_file() {
            fs::copy(&from, &to).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

fn copy_governed_build_inputs(build_source: &Path, destination: &Path) -> Result<(), String> {
    for name in ["semantic-input-manifest.json", "semantic-input-policy.json"] {
        let source = build_source.join(name);
        match fs::symlink_metadata(&source) {
            Ok(metadata) if metadata.file_type().is_file() => {}
            Ok(_) => continue,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.to_string()),
        }
        let target_dir = destination.join("build");
        fs::create_dir_all(&target_dir).map_err(|error| error.to_string())?;
        fs::copy(&source, target_dir.join(name)).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn apply_operation(root: &Path, operation: &PathOperation) -> Result<(), String> {
    match operation {
        PathOperation::Write { path, content } => {
            let path = root.join(path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            fs::write(path, content).map_err(|error| error.to_string())
        }
        PathOperation::Move { from, to } => {
            let from = root.join(from);
            let to = root.join(to);
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            fs::rename(from, to).map_err(|error| error.to_string())
        }
    }
}

fn impacts(repository: &Repository, operations: &[PathOperation]) -> Vec<GeneratedImpact> {
    operations
        .iter()
        .filter_map(|operation| match operation {
            PathOperation::Write { path, content } => {
                let absolute = repository.root().join(path);
                let before = digest_state(&repository.path_state(&absolute));
                let after = hex_digest(Sha256::digest(content.as_bytes()));
                Some(GeneratedImpact {
                    path: path.clone(),
                    before_digest: before,
                    after_digest: after,
                    preview: format!("--- {path}\n+++ {path}\n{content}"),
                    reason: if path == "audits/reports/dashboard.md" {
                        "owned generated dashboard projection".to_owned()
                    } else {
                        "durable work-item record".to_owned()
                    },
                })
            }
            PathOperation::Move { from, to } => Some(GeneratedImpact {
                path: format!("{from} -> {to}"),
                before_digest: digest_state(&repository.path_state(&repository.root().join(from))),
                after_digest: digest_state(&repository.path_state(&repository.root().join(from))),
                preview: format!("rename {from} -> {to}"),
                reason: "complete work-item directory lifecycle transition".to_owned(),
            }),
        })
        .collect()
}

fn preview(operations: &[PathOperation]) -> String {
    operations
        .iter()
        .map(|operation| match operation {
            PathOperation::Write { path, content } => format!("--- {path}\n+++ {path}\n{content}"),
            PathOperation::Move { from, to } => format!("rename {from} -> {to}"),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn graph_impacts(graph: &RepositoryGraph, request: &MutationRequest) -> Vec<GraphEdge> {
    let id = match request {
        MutationRequest::CreateWorkItem(_) => None,
        MutationRequest::EditWorkItem(request) => Some(request.id.as_str()),
        MutationRequest::TransitionWorkItem(request) => Some(request.id.as_str()),
    };
    id.map(|id| graph.edges_from(&format!("work:{id}")))
        .unwrap_or_default()
}

fn add_frozen_diagnostics(snapshot: &RepositorySnapshot, plan: &mut MutationPlan) {
    let Some(frozen) = &snapshot.index().frozen_surface else {
        return;
    };
    let Some(value) = frozen.value.as_ref().ok() else {
        return;
    };
    for operation in &plan.file_operations {
        let paths = match operation {
            PathOperation::Write { path, .. } => vec![path.as_str()],
            PathOperation::Move { from, to } => vec![from.as_str(), to.as_str()],
        };
        for path in paths {
            for entry in value
                .get("protected")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let glob = entry.get("glob").and_then(Value::as_str).unwrap_or("");
                if wildcard_match(path, glob) {
                    let reason = entry
                        .get("reason")
                        .and_then(Value::as_str)
                        .unwrap_or("protected frozen-surface path");
                    plan.diagnostics.push(
                        MutationDiagnostic::error(
                            "mutation.frozen-surface",
                            format!("native apply is closed for protected path '{path}': {reason}"),
                        )
                        .at(path),
                    );
                }
            }
        }
    }
}

pub fn apply(plan: &MutationPlan, options: &ApplyOptions) -> MutationResult {
    let mut result = MutationResult {
        success: false,
        rolled_back: false,
        plan_token: plan.plan_token.clone(),
        changed_paths: Vec::new(),
        diagnostics: plan.diagnostics.clone(),
        replayed: false,
        metrics: plan.metrics.clone(),
    };
    if matches!(&plan.request, MutationRequest::CreateWorkItem(item) if item.status == "proposed") {
        return apply_proposal(plan, options, result);
    }
    if !plan.is_applicable() {
        return result;
    }
    let repository = Repository::open(&plan.repository_identity.root);
    if repository.identity() != plan.repository_identity {
        result.diagnostics.push(MutationDiagnostic::error(
            "mutation.repository-identity",
            "repository identity changed since planning",
        ));
        return result;
    }
    if let Some(stale) = stale_read_fact(&repository, &plan.read_set) {
        result.diagnostics.push(
            MutationDiagnostic::error(
                "mutation.plan-stale",
                format!(
                    "plan read fact for '{}' no longer matches; re-plan before apply",
                    stale.path
                ),
            )
            .at(stale.path),
        );
        return result;
    }

    let roots = operation_roots(plan, &repository);
    let preimages = capture_preimages(&repository, &roots);
    let mut expected = roots
        .iter()
        .map(|path| {
            (
                path.clone(),
                repository.path_state(&repository.root().join(path)),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for (index, operation) in plan.file_operations.iter().enumerate() {
        if options.external_drift_at == Some(index) {
            if let Some(path) = options.external_drift_path.as_deref() {
                let absolute = repository.root().join(path);
                if let Some(parent) = absolute.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                if let Err(error) = fs::write(&absolute, b"external drift injected during apply") {
                    return failure_with_rollback(
                        &repository,
                        &roots,
                        &preimages,
                        result,
                        format!("unable to inject external drift: {error}"),
                    );
                }
            }
        }
        if options.fail_before_operation == Some(index) {
            return failure_with_rollback(
                &repository,
                &roots,
                &preimages,
                result,
                format!("failure injected before operation {index}"),
            );
        }
        if let Some(stale_path) = operation_drift(operation, &repository, &expected) {
            return failure_with_rollback(&repository, &roots, &preimages, result, format!("target '{stale_path}' changed during apply; refusing to merge external content"));
        }
        if let Err(error) = apply_operation(repository.root(), operation) {
            return failure_with_rollback(
                &repository,
                &roots,
                &preimages,
                result,
                format!("apply failed: {error}"),
            );
        }
        refresh_expected_paths(&repository, &mut expected, operation_paths(operation));
        if options.fail_after_operation == Some(index) {
            return failure_with_rollback(
                &repository,
                &roots,
                &preimages,
                result,
                format!("failure injected after operation {index}"),
            );
        }
    }
    let post = repopact_validation::validate(repository.root());
    if post.has_errors() {
        let messages = post
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.clone())
            .collect::<Vec<_>>()
            .join("; ");
        return failure_with_rollback(
            &repository,
            &roots,
            &preimages,
            result,
            format!("post-state validation failed: {messages}"),
        );
    }
    result.success = true;
    result.changed_paths = plan
        .file_operations
        .iter()
        .flat_map(operation_paths)
        .map(|path| path_string(&path))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    result
}

fn apply_proposal(
    plan: &MutationPlan,
    options: &ApplyOptions,
    mut result: MutationResult,
) -> MutationResult {
    if !plan.is_applicable() {
        return result;
    }
    let request = match &plan.request {
        MutationRequest::CreateWorkItem(request) => request,
        _ => return result,
    };
    let Some(key) = request.idempotency_key.as_deref() else {
        result.diagnostics.push(MutationDiagnostic::error(
            "work.proposal-key-required",
            "proposal cannot be applied without its idempotency key",
        ));
        return result;
    };
    if let Err(error) = validate_idempotency_key(key) {
        result.diagnostics.push(MutationDiagnostic::error(
            "work.proposal-key-invalid",
            error,
        ));
        return result;
    }
    let repository = Repository::open(&plan.repository_identity.root);
    let lock_started = Instant::now();
    let _lock = match ProposalLock::acquire(repository.root()) {
        Ok(lock) => lock,
        Err(error) => {
            result.metrics.proposal_lock_wait_ms = lock_started.elapsed().as_millis();
            result.diagnostics.push(MutationDiagnostic::error(
                "work.proposal-lock-timeout",
                error,
            ));
            return result;
        }
    };
    result.metrics.proposal_lock_wait_ms = lock_started.elapsed().as_millis();
    let cleanup_started = Instant::now();
    if let Err(error) = ensure_safe_repository_directory(repository.root(), Path::new("work")) {
        result.metrics.filesystem_staging_ms += cleanup_started.elapsed().as_millis();
        result.diagnostics.push(MutationDiagnostic::error(
            "work.proposal-stage-cleanup-failed",
            error,
        ));
        return result;
    }
    if let Err(error) = clean_stale_proposal_staging(repository.root()) {
        result.metrics.filesystem_staging_ms += cleanup_started.elapsed().as_millis();
        result.diagnostics.push(MutationDiagnostic::error(
            "work.proposal-stage-cleanup-failed",
            error,
        ));
        return result;
    }
    result.metrics.filesystem_staging_ms += cleanup_started.elapsed().as_millis();
    let receipt_started = Instant::now();
    let receipt = resolve_proposal_receipt(repository.root(), key, request, &mut result);
    result.metrics.idempotency_lookup_ms = receipt_started.elapsed().as_millis();
    match receipt {
        Ok(false) => return result,
        Ok(true) => {}
        Err(error) => {
            result.diagnostics.push(MutationDiagnostic::error(
                "work.proposal-receipt-read-failed",
                error,
            ));
            return result;
        }
    }
    if repository.identity() != plan.repository_identity {
        result.diagnostics.push(MutationDiagnostic::error(
            "mutation.repository-identity",
            "repository identity changed since proposal planning",
        ));
        return result;
    }

    let snapshot_started = Instant::now();
    let fresh = repository.session().snapshot();
    result.metrics.snapshot_ms += snapshot_started.elapsed().as_millis();
    let stale_started = Instant::now();
    let stale_plan = stale_read_fact(&repository, &plan.read_set).is_some();
    result.metrics.stale_check_ms = stale_started.elapsed().as_millis();
    let planned_id = plan
        .file_operations
        .iter()
        .find_map(|operation| match operation {
            PathOperation::Write { path, content } if path.ends_with("/work-item.json") => {
                serde_json::from_str::<Value>(content)
                    .ok()
                    .and_then(|value| value.get("id").and_then(Value::as_str).map(str::to_owned))
            }
            _ => None,
        });
    let allocated_id_now = planned_id
        .as_deref()
        .is_some_and(|id| fresh.index().work_item(id).is_some());
    let effective_plan = if stale_plan || allocated_id_now {
        let started = Instant::now();
        let replanned = plan_mutation(&fresh, plan.request.clone());
        result.metrics.planning_ms += started.elapsed().as_millis();
        result.metrics.critical_validation_ms += replanned.metrics.critical_validation_ms;
        result.metrics.authority_checks_ms = replanned.metrics.authority_checks_ms;
        result.metrics.work_schema_ms = replanned.metrics.work_schema_ms;
        result.metrics.dependency_graph_ms = replanned.metrics.dependency_graph_ms;
        result.metrics.maintenance_diagnostics_ms = replanned.metrics.maintenance_diagnostics_ms;
        result.metrics.records_examined = replanned.metrics.records_examined;
        result.metrics.source_files_read = replanned.metrics.source_files_read;
        result.plan_token = replanned.plan_token.clone();
        result.diagnostics = replanned.diagnostics.clone();
        if !replanned.is_applicable() {
            return result;
        }
        return commit_proposal_locked(&repository, &replanned, &fresh, options, result);
    } else {
        plan.clone()
    };
    commit_proposal_locked(&repository, &effective_plan, &fresh, options, result)
}

fn commit_proposal_locked(
    repository: &Repository,
    plan: &MutationPlan,
    snapshot: &RepositorySnapshot,
    options: &ApplyOptions,
    mut result: MutationResult,
) -> MutationResult {
    let mut files = Vec::new();
    let mut manifest_path: Option<String> = None;
    let mut manifest_value: Option<Value> = None;
    for operation in &plan.file_operations {
        let PathOperation::Write { path, content } = operation else {
            result.diagnostics.push(MutationDiagnostic::error(
                "work.proposal-transaction-invalid",
                "proposal transaction contains a directory move",
            ));
            return result;
        };
        if path.ends_with("/work-item.json") {
            match serde_json::from_str::<Value>(content) {
                Ok(value) => {
                    manifest_path = Some(path.clone());
                    manifest_value = Some(value);
                }
                Err(error) => {
                    result.diagnostics.push(MutationDiagnostic::error(
                        "work.proposal-manifest-invalid",
                        format!("proposal manifest is not valid JSON: {error}"),
                    ));
                    return result;
                }
            }
        }
        files.push((path.clone(), content.as_bytes().to_vec()));
    }
    let (Some(manifest_path), Some(candidate)) = (manifest_path, manifest_value) else {
        result.diagnostics.push(MutationDiagnostic::error(
            "work.proposal-candidate-missing",
            "proposal transaction has no work-item manifest",
        ));
        return result;
    };
    let Some(directory) = Path::new(&manifest_path).parent() else {
        result.diagnostics.push(MutationDiagnostic::error(
            "work.proposal-transaction-invalid",
            "proposal manifest path has no parent directory",
        ));
        return result;
    };
    let destination = repository.root().join(directory);
    let relative_parent = directory.parent().unwrap_or_else(|| Path::new("."));
    if let Err(error) = ensure_safe_repository_directory(repository.root(), relative_parent) {
        result.diagnostics.push(MutationDiagnostic::error(
            "work.proposal-stage-failed",
            format!("unable to prepare safe proposal parent directory: {error}"),
        ));
        return result;
    }
    if fs::symlink_metadata(&destination).is_ok() {
        result.diagnostics.push(MutationDiagnostic::error(
            "work.proposal-id-collision",
            format!(
                "proposal destination '{}' already exists; retry with the same key",
                path_string(&destination)
            ),
        ));
        return result;
    }
    if options.fail_before_operation == Some(0) {
        result.diagnostics.push(MutationDiagnostic::error(
            "work.proposal-stage-failed",
            "proposal transaction failed before its atomic commit point",
        ));
        return result;
    }
    let stage_started = Instant::now();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let staging_root = match ensure_safe_repository_directory(
        repository.root(),
        Path::new("work/.repopact-transactions"),
    ) {
        Ok(path) => path,
        Err(error) => {
            result.metrics.filesystem_staging_ms += stage_started.elapsed().as_millis();
            result.diagnostics.push(MutationDiagnostic::error(
                "work.proposal-stage-failed",
                format!("unable to prepare same-volume proposal staging: {error}"),
            ));
            return result;
        }
    };
    let stage = staging_root.join(format!(".repopact-stage-{}-{nonce}", std::process::id()));
    if let Err(error) = fs::create_dir(&stage) {
        result.metrics.filesystem_staging_ms += stage_started.elapsed().as_millis();
        result.diagnostics.push(MutationDiagnostic::error(
            "work.proposal-stage-failed",
            format!("unable to create proposal staging directory: {error}"),
        ));
        return result;
    }
    let mut atomic_commit_ms = 0;
    let staged = (|| -> Result<(), String> {
        for (index, (path, bytes)) in files.iter().enumerate() {
            if options.fail_before_operation == Some(index) {
                return Err(format!(
                    "injected proposal-stage failure before staged file {index}"
                ));
            }
            let relative = Path::new(path);
            if relative.parent() != Some(directory) {
                return Err(format!(
                    "proposal path '{path}' escaped its transaction directory"
                ));
            }
            let name = relative
                .file_name()
                .ok_or_else(|| format!("proposal path '{path}' has no filename"))?;
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(stage.join(name))
                .map_err(|error| error.to_string())?;
            file.write_all(bytes).map_err(|error| error.to_string())?;
            file.sync_all().map_err(|error| error.to_string())?;
        }
        if fs::symlink_metadata(&destination).is_ok() {
            return Err("proposal destination appeared before atomic commit".to_owned());
        }
        let commit_started = Instant::now();
        fs::rename(&stage, &destination).map_err(|error| error.to_string())?;
        atomic_commit_ms = commit_started.elapsed().as_millis();
        Ok(())
    })();
    let elapsed = stage_started.elapsed().as_millis();
    result.metrics.filesystem_staging_ms += elapsed.saturating_sub(atomic_commit_ms);
    result.metrics.atomic_commit_ms = atomic_commit_ms;
    if let Err(error) = staged {
        let _ = fs::remove_dir_all(&stage);
        result.diagnostics.push(MutationDiagnostic::error(
            "work.proposal-commit-failed",
            format!("proposal was not committed: {error}"),
        ));
        return result;
    }
    result.success = true;
    result.changed_paths = files.iter().map(|(path, _)| path.clone()).collect();

    let dashboard_started = Instant::now();
    let dashboard = repopact_validation::render_dashboard_snapshot_with_work_item(
        snapshot,
        repository.root().join(&manifest_path),
        candidate,
    )
    .and_then(|content| {
        ensure_safe_repository_directory(repository.root(), Path::new("audits/reports"))?;
        atomic_write_projection(
            &repository.root().join("audits/reports/dashboard.md"),
            content.as_bytes(),
        )
        .map(|changed| (content, changed))
    });
    result.metrics.dashboard_regeneration_ms = dashboard_started.elapsed().as_millis();
    match dashboard {
        Ok((_, changed)) => {
            result
                .diagnostics
                .retain(|item| item.code != "dashboard.stale" && item.code != "dashboard.missing");
            if changed {
                result
                    .changed_paths
                    .push("audits/reports/dashboard.md".to_owned());
            }
        }
        Err(error) => result.diagnostics.push(MutationDiagnostic {
            code: "mutation.dashboard-regeneration-failed".to_owned(),
            message: format!("proposal committed; dashboard maintenance needs repair: {error}"),
            path: Some("audits/reports/dashboard.md".to_owned()),
            blocking: false,
            related_records: Vec::new(),
        }),
    }
    result.changed_paths.sort();
    result.changed_paths.dedup();
    result
}

fn ensure_safe_repository_directory(root: &Path, relative: &Path) -> Result<PathBuf, String> {
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(format!(
            "repository directory path '{}' is not a contained relative path",
            path_string(relative)
        ));
    }
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() => {
            }
            Ok(_) => {
                return Err(format!(
                    "repository path '{}' is not a regular directory",
                    path_string(&current)
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                match fs::create_dir(&current) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                        let metadata =
                            fs::symlink_metadata(&current).map_err(|error| error.to_string())?;
                        if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
                            return Err(format!(
                                "repository path '{}' is not a regular directory",
                                path_string(&current)
                            ));
                        }
                    }
                    Err(error) => return Err(error.to_string()),
                }
            }
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(current)
}

fn atomic_write_projection(path: &Path, content: &[u8]) -> Result<bool, String> {
    if fs::read(path).ok().as_deref() == Some(content) {
        return Ok(false);
    }
    let parent = path
        .parent()
        .ok_or_else(|| "projection has no parent directory".to_owned())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("dashboard.md");
    let temporary = parent.join(format!(".{name}.repopact-{nonce}.tmp"));
    let written = (|| -> Result<(), String> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| error.to_string())?;
        file.write_all(content).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        fs::rename(&temporary, path).map_err(|error| error.to_string())
    })();
    if written.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    written.map(|_| true)
}

fn failure_with_rollback(
    repository: &Repository,
    roots: &[PathBuf],
    preimages: &Preimages,
    mut result: MutationResult,
    message: String,
) -> MutationResult {
    result
        .diagnostics
        .push(MutationDiagnostic::error("mutation.apply-failed", message));
    match restore_preimages(repository, roots, preimages) {
        Ok(()) => result.rolled_back = true,
        Err(error) => result
            .diagnostics
            .push(MutationDiagnostic::error("mutation.rollback-failed", error)),
    }
    result
}

fn stale_read_fact(repository: &Repository, read_set: &ReadSet) -> Option<ReadFact> {
    read_set
        .facts
        .iter()
        .find(|fact| repository.path_state(&repository.root().join(&fact.path)) != fact.expected)
        .cloned()
}

fn operation_roots(plan: &MutationPlan, repository: &Repository) -> Vec<PathBuf> {
    let mut roots = BTreeSet::new();
    for operation in &plan.file_operations {
        match operation {
            PathOperation::Write { path, .. } => {
                let file = PathBuf::from(path);
                roots.insert(file.clone());
                if let Some(parent) = file
                    .parent()
                    .filter(|parent| !parent.as_os_str().is_empty())
                {
                    let absolute = repository.root().join(parent);
                    if repository.path_state(&absolute) == PathState::Absent {
                        roots.insert(parent.to_path_buf());
                    }
                }
            }
            PathOperation::Move { from, to } => {
                roots.insert(PathBuf::from(from));
                roots.insert(PathBuf::from(to));
            }
        }
    }
    roots.into_iter().collect()
}

fn operation_paths(operation: &PathOperation) -> Vec<PathBuf> {
    match operation {
        PathOperation::Write { path, .. } => vec![PathBuf::from(path)],
        PathOperation::Move { from, to } => vec![PathBuf::from(from), PathBuf::from(to)],
    }
}

fn operation_drift(
    operation: &PathOperation,
    repository: &Repository,
    expected: &BTreeMap<PathBuf, PathState>,
) -> Option<String> {
    operation_paths(operation)
        .into_iter()
        .find(|path| {
            repository.path_state(&repository.root().join(path))
                != expected.get(path).cloned().unwrap_or(PathState::Absent)
        })
        .map(|path| path_string(&path))
}

fn refresh_expected_paths(
    repository: &Repository,
    expected: &mut BTreeMap<PathBuf, PathState>,
    paths: Vec<PathBuf>,
) {
    for path in paths {
        let absolute = repository.root().join(&path);
        expected.insert(path, repository.path_state(&absolute));
        if absolute.is_dir() {
            for file in repository.files_under(&absolute) {
                expected.insert(
                    PathBuf::from(repository.relative_path(&file)),
                    repository.path_state(&file),
                );
            }
        }
    }
}

#[derive(Debug, Default)]
struct Preimages {
    files: BTreeMap<PathBuf, Vec<u8>>,
    directories: BTreeSet<PathBuf>,
}

fn capture_preimages(repository: &Repository, roots: &[PathBuf]) -> Preimages {
    let mut preimages = Preimages::default();
    for root in roots {
        let absolute = repository.root().join(root);
        if absolute.is_file() {
            if let Ok(bytes) = fs::read(&absolute) {
                preimages.files.insert(root.clone(), bytes);
            }
        } else if absolute.is_dir() {
            collect_preimage_tree(repository.root(), &absolute, &mut preimages);
        }
    }
    preimages
}

fn collect_preimage_tree(root: &Path, current: &Path, preimages: &mut Preimages) {
    let relative = current.strip_prefix(root).unwrap_or(current).to_path_buf();
    preimages.directories.insert(relative);
    let Ok(entries) = fs::read_dir(current) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
            collect_preimage_tree(root, &path, preimages);
        } else if entry
            .file_type()
            .map(|kind| kind.is_file())
            .unwrap_or(false)
        {
            if let Ok(bytes) = fs::read(&path) {
                preimages.files.insert(
                    path.strip_prefix(root).unwrap_or(&path).to_path_buf(),
                    bytes,
                );
            }
        }
    }
}

fn restore_preimages(
    repository: &Repository,
    roots: &[PathBuf],
    preimages: &Preimages,
) -> Result<(), String> {
    let mut removal = roots
        .iter()
        .map(|root| repository.root().join(root))
        .collect::<Vec<_>>();
    removal.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    removal.dedup();
    for path in removal {
        remove_path(&path)?;
    }
    let mut directories = preimages
        .directories
        .iter()
        .map(|path| repository.root().join(path))
        .collect::<Vec<_>>();
    directories.sort_by_key(|path| path.components().count());
    for path in directories {
        fs::create_dir_all(path).map_err(|error| error.to_string())?;
    }
    for (relative, bytes) in &preimages.files {
        let path = repository.root().join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::write(path, bytes).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn remove_path(path: &Path) -> Result<(), String> {
    if path.is_dir() {
        fs::remove_dir_all(path).map_err(|error| error.to_string())
    } else if path.exists() {
        fs::remove_file(path).map_err(|error| error.to_string())
    } else {
        Ok(())
    }
}

fn digest_state(state: &PathState) -> String {
    match state {
        PathState::Absent => "absent".to_owned(),
        PathState::Present { digest, .. } => digest.clone(),
    }
}

fn canonical_json(value: &Value) -> String {
    platform_text(&(serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_owned()) + "\n"))
}

fn platform_text(value: &str) -> String {
    let normalized = value.replace("\r\n", "\n");
    if cfg!(windows) {
        normalized.replace('\n', "\r\n")
    } else {
        normalized
    }
}
fn default_concrete() -> String {
    "concrete".to_owned()
}

fn next_work_id(index: &repopact_repository::RecordIndex) -> u64 {
    index
        .work_items
        .iter()
        .filter_map(|record| {
            record
                .reference
                .path
                .rsplit('/')
                .nth(1)
                .and_then(|directory| directory.split('-').next())
                .and_then(|id| id.parse::<u64>().ok())
                .or_else(|| record.reference.id.parse::<u64>().ok())
        })
        .max()
        .unwrap_or(0)
        + 1
}

fn slugify(title: &str) -> String {
    let mut slug = String::new();
    let mut dash = false;
    for character in title.to_lowercase().chars() {
        if character.is_ascii_lowercase() || character.is_ascii_digit() {
            slug.push(character);
            dash = false;
        } else if !dash {
            slug.push('-');
            dash = true;
        }
    }
    slug.trim_matches('-').to_owned()
}

fn wildcard_match(value: &str, pattern: &str) -> bool {
    let value = value.as_bytes();
    let pattern = pattern.as_bytes();
    let mut previous = vec![false; value.len() + 1];
    previous[0] = true;
    for character in pattern {
        let mut current = vec![false; value.len() + 1];
        match character {
            b'*' => {
                current[0] = previous[0];
                for index in 1..=value.len() {
                    current[index] = previous[index] || current[index - 1];
                }
            }
            b'?' => {
                for index in 1..=value.len() {
                    current[index] = previous[index - 1];
                }
            }
            literal => {
                for index in 1..=value.len() {
                    current[index] = previous[index - 1] && value[index - 1] == *literal;
                }
            }
        }
        previous = current;
    }
    previous[value.len()]
}

fn has_wi050_surface(root: &Path) -> bool {
    [
        "governance/admission-policy.json",
        "governance/operator-authority.json",
        "governance/repository-registration.json",
    ]
    .into_iter()
    .any(|relative| root.join(relative).is_file())
        || root.join("evidence/admission").is_dir()
        || root.join("governance/adapters").is_dir()
}

#[cfg(test)]
mod tests {
    use super::*;
    use repopact_repository::RepositorySession;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn candidate_copy_includes_only_governed_semantic_build_inputs() {
        let root = std::env::temp_dir().join(format!(
            "repopact-build-input-copy-{}-{}",
            std::process::id(),
            TEST_ROOT_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let destination = root.join("candidate");
        let source = root.join("source");
        let build = source.join("build");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(build.join("target")).unwrap();
        fs::write(build.join("semantic-input-manifest.json"), "manifest\n").unwrap();
        fs::write(build.join("semantic-input-policy.json"), "policy\n").unwrap();
        fs::write(build.join("unrelated-cache.bin"), "large output\n").unwrap();
        fs::write(build.join("target/artifact.bin"), "cargo output\n").unwrap();

        copy_tree(&source, &destination).unwrap();

        assert_eq!(
            fs::read_to_string(destination.join("build/semantic-input-manifest.json")).unwrap(),
            "manifest\n"
        );
        assert_eq!(
            fs::read_to_string(destination.join("build/semantic-input-policy.json")).unwrap(),
            "policy\n"
        );
        assert!(!destination.join("build/unrelated-cache.bin").exists());
        assert!(!destination.join("build/target").exists());
        fs::remove_dir_all(root).unwrap();
    }

    fn fixture() -> (PathBuf, RepositorySnapshot) {
        let root = std::env::temp_dir().join(format!(
            "repopact-mutation-test-{}-{}",
            std::process::id(),
            TEST_ROOT_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&root);
        let valid =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../conformance/fixtures/valid");
        copy_tree(&valid, &root).unwrap();
        fs::remove_dir_all(root.join("work/completed/001-seed")).unwrap();
        fs::create_dir_all(root.join("work/active/001-one")).unwrap();
        fs::write(root.join("work/active/001-one/work-item.json"), r#"{"id":"001","title":"One","status":"active","owner_scope":"governance","affected_scopes":[],"depends_on":[],"acceptance_criteria":[{"id":"AC-1","text":"prove","state":"pending","evidence":[]}],"created":"2026-01-01","updated":"2026-01-01"}"#).unwrap();
        fs::write(
            root.join("work/active/001-one/README.md"),
            "# One\n\nIntent\n",
        )
        .unwrap();
        fs::write(root.join("work/active/001-one/extra.md"), "history\n").unwrap();
        fs::write(root.join("audits/reports/dashboard.md"), "old\n").unwrap();
        let snapshot = RepositorySession::open(&root).snapshot();
        (root, snapshot)
    }

    fn create_request(title: &str, date: &str) -> CreateWorkItem {
        CreateWorkItem::new(title, date)
            .confirm_preflight_before_work_started()
            .with_acceptance_criteria(vec![AcceptanceCriterion {
                id: "AC-01".to_owned(),
                text: "A reviewer can verify the registered outcome".to_owned(),
                state: "pending".to_owned(),
                evidence: Vec::new(),
                provenance: "concrete".to_owned(),
            }])
    }

    // ---- ROG-037/040 adversarial authority-boundary proof (Decision 0053
    // section 7): a graph fact claiming approval/ownership/waiver must
    // never change a mutation plan's diagnostics or applicability. ----

    #[test]
    fn a_tampered_durable_graph_claiming_approval_cannot_remove_canonical_lifecycle_blockers() {
        let (root, snapshot) = fixture();
        let request =
            MutationRequest::transition_work_item(TransitionWorkItem::new("001", "completed"));

        // Baseline: no durable graph exists at all for this repository.
        let baseline_plan = plan(&snapshot, request.clone());

        // Build a real durable graph, then tamper it with a fabricated
        // authority-like fact naming the exact work item this plan
        // targets as already approved/waived.
        repopact_graph::build_and_write(&snapshot).expect("baseline graph build");
        let shard_dir = root.join("rog/nodes");
        let shard_path = fs::read_dir(&shard_dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| {
                fs::read_to_string(path)
                    .map(|content| !content.trim().is_empty())
                    .unwrap_or(false)
            })
            .expect("at least one non-empty node shard");
        let mut content = fs::read_to_string(&shard_path).unwrap();
        content.push_str(
            r#"{"id":"work:001","kind":"work_item","label":"APPROVED - CRITERION AC-1 WAIVED, FROZEN SURFACE ACKNOWLEDGED","layer":"governance","node_role":"owner"}"#,
        );
        content.push('\n');
        fs::write(&shard_path, content).unwrap();

        let refreshed_snapshot = RepositorySession::open(&root).snapshot();
        let tampered_plan = plan(&refreshed_snapshot, request);

        assert!(!baseline_plan.is_applicable());
        assert!(!tampered_plan.is_applicable());
        assert!(baseline_plan
            .diagnostics
            .iter()
            .any(|item| { item.code == "work.completed-pending-criterion" && item.blocking }));
        assert!(
            tampered_plan
                .diagnostics
                .iter()
                .any(|item| { item.code == "work.completed-pending-criterion" && item.blocking }),
            "a fabricated graph claim cannot override the canonical work-item record"
        );
        assert!(tampered_plan.diagnostics.iter().all(|item| {
            !item.message.contains("APPROVED") && !item.message.contains("WAIVED")
        }));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn canonical_work_item_status_always_comes_from_the_work_item_record_never_the_graph() {
        // The work item's real status is "active" (see `fixture()`). A
        // durable graph node whose *label* claims "completed" must never
        // change what `snapshot.index()` -- the sole authority `plan()`
        // consults for lifecycle decisions -- reports.
        let (root, snapshot) = fixture();
        repopact_graph::build_and_write(&snapshot).expect("baseline graph build");
        let shard_dir = root.join("rog/nodes");
        let shard_path = fs::read_dir(&shard_dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| {
                fs::read_to_string(path)
                    .map(|content| !content.trim().is_empty())
                    .unwrap_or(false)
            })
            .unwrap();
        let mut content = fs::read_to_string(&shard_path).unwrap();
        content.push_str(
            r#"{"id":"work:001","kind":"work_item","label":"completed","layer":"governance"}"#,
        );
        content.push('\n');
        fs::write(&shard_path, content).unwrap();

        let refreshed_snapshot = RepositorySession::open(&root).snapshot();
        let record = refreshed_snapshot
            .index()
            .work_item("001")
            .expect("canonical work item record");
        let value = record.value.clone().expect("work item JSON parses");
        assert_eq!(
            value["status"], "active",
            "canonical work-item status must come only from work-item.json, never a graph label"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn planning_is_deterministic_and_does_not_write() {
        let (root, snapshot) = fixture();
        let before = fs::read(root.join("work/active/001-one/work-item.json")).unwrap();
        let request =
            MutationRequest::create_work_item(create_request("A New Thing", "2026-01-02"));
        let fixed_time = UNIX_EPOCH + std::time::Duration::from_secs(86_400 + 3_661);
        let first = plan_at(&snapshot, request.clone(), fixed_time);
        let second = plan_at(&snapshot, request, fixed_time);
        assert_eq!(first.plan_token, second.plan_token);
        assert_eq!(first.file_operations, second.file_operations);
        assert!(first.is_applicable(), "{:?}", first.diagnostics);
        assert_eq!(
            before,
            fs::read(root.join("work/active/001-one/work-item.json")).unwrap()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn create_requires_explicit_preflight_confirmation() {
        let (root, snapshot) = fixture();
        let unconfirmed = plan(
            &snapshot,
            MutationRequest::create_work_item(
                CreateWorkItem::new("Unconfirmed", "2026-01-02").with_acceptance_criteria(
                    create_request("seed", "2026-01-02").acceptance_criteria,
                ),
            ),
        );
        assert!(!unconfirmed.is_applicable());
        assert!(unconfirmed
            .diagnostics
            .iter()
            .any(|item| item.code == "mutation.preflight-unconfirmed"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn preflight_timestamp_uses_the_actual_injected_utc_instant_and_readme_has_no_template_placeholders(
    ) {
        let (root, snapshot) = fixture();
        let plan = plan_at(
            &snapshot,
            MutationRequest::create_work_item(create_request("Truthful Probe", "2026-01-02")),
            UNIX_EPOCH + std::time::Duration::from_secs(86_400 + 3_661),
        );
        assert!(plan.is_applicable(), "{:?}", plan.diagnostics);
        assert_eq!(
            format_utc_timestamp(UNIX_EPOCH + std::time::Duration::from_secs(86_400 + 3_661))
                .unwrap(),
            "1970-01-02T01:01:01Z"
        );
        let manifest = plan
            .file_operations
            .iter()
            .find_map(|operation| match operation {
                PathOperation::Write { path, content } if path.ends_with("work-item.json") => {
                    Some(content)
                }
                _ => None,
            })
            .unwrap();
        let value: Value = serde_json::from_str(manifest).unwrap();
        assert_eq!(value["preflight"]["created_at"], "1970-01-02T01:01:01Z");
        assert_eq!(value["preflight"]["created_before_work_started"], true);
        assert!(value["preflight"]["note"]
            .as_str()
            .unwrap()
            .contains("operator's preflight confirmation"));
        let readme = plan
            .file_operations
            .iter()
            .find_map(|operation| match operation {
                PathOperation::Write { path, content } if path.ends_with("README.md") => {
                    Some(content)
                }
                _ => None,
            })
            .unwrap();
        assert!(readme.contains("# 002 — Truthful Probe"));
        assert!(readme.contains("AC-01"));
        assert!(!readme.contains("<role>"));
        assert!(!readme.contains("<work item ids"));
        assert!(!readme.contains("What outcome this work produces"));
        assert!(!readme.contains("TODO"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn production_plan_timestamp_is_current_utc_not_midnight_from_the_requested_date() {
        let (root, snapshot) = fixture();
        let before = SystemTime::now();
        let plan = plan(
            &snapshot,
            MutationRequest::create_work_item(create_request("Current time probe", "2026-01-02")),
        );
        let after = SystemTime::now();
        assert!(plan.is_applicable(), "{:?}", plan.diagnostics);
        let manifest = plan
            .file_operations
            .iter()
            .find_map(|operation| match operation {
                PathOperation::Write { path, content } if path.ends_with("work-item.json") => {
                    Some(content)
                }
                _ => None,
            })
            .unwrap();
        let value: Value = serde_json::from_str(manifest).unwrap();
        let created_at = value["preflight"]["created_at"].as_str().unwrap();
        assert_ne!(created_at, "2026-01-02T00:00:00Z");
        assert!(created_at >= format_utc_timestamp(before).unwrap().as_str());
        assert!(created_at <= format_utc_timestamp(after).unwrap().as_str());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn malformed_legacy_evidence_blocks_a_create_plan_before_approval() {
        let (root, _) = fixture();
        fs::create_dir_all(root.join("evidence/runs")).unwrap();
        fs::write(root.join("evidence/runs/legacy.json"), br#"{""#).unwrap();
        let snapshot = RepositorySession::open(&root).snapshot();
        let candidate = plan(
            &snapshot,
            MutationRequest::create_work_item(create_request(
                "Blocked by invalid evidence",
                "2026-01-02",
            )),
        );
        assert!(!candidate.is_applicable());
        assert!(candidate
            .diagnostics
            .iter()
            .any(|item| item.blocking && item.code == "evidence.json-invalid"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn expected_absent_target_makes_plan_stale() {
        let (root, snapshot) = fixture();
        let plan = plan(
            &snapshot,
            MutationRequest::create_work_item(create_request("A New Thing", "2026-01-02")),
        );
        fs::create_dir_all(root.join("work/active/002-a-new-thing")).unwrap();
        let result = plan.apply();
        assert!(!result.success);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "mutation.plan-stale"),
            "{result:?}"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn injected_failure_restores_complete_transition_directory() {
        let (root, snapshot) = fixture();
        let request =
            MutationRequest::transition_work_item(TransitionWorkItem::new("001", "blocked"));
        let plan = plan(&snapshot, request);
        assert!(plan.is_applicable(), "{:?}", plan.diagnostics);
        let result = plan.apply_with_options(&ApplyOptions::fail_after(0));
        assert!(!result.success);
        assert!(result.rolled_back);
        assert!(root.join("work/active/001-one/extra.md").is_file());
        assert!(!root.join("work/blocked/001-one").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn pending_completion_is_blocked_by_candidate_validation_before_apply() {
        let (root, snapshot) = fixture();
        let plan = plan(
            &snapshot,
            MutationRequest::transition_work_item(TransitionWorkItem::new("001", "completed")),
        );
        assert!(!plan.is_applicable());
        assert!(plan
            .diagnostics
            .iter()
            .any(|item| { item.code == "work.completed-pending-criterion" && item.blocking }));
        let result = plan.apply();
        assert!(!result.success);
        assert!(!result.rolled_back, "a blocked plan performs no writes");
        assert!(root.join("work/active/001-one/work-item.json").is_file());
        assert!(!root.join("work/completed/001-one").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn typed_edit_updates_only_allowed_fields_and_validates() {
        let (root, snapshot) = fixture();
        let changes = WorkItemEdits {
            title: Some("Edited title".to_owned()),
            ..WorkItemEdits::default()
        };
        let plan = plan(
            &snapshot,
            MutationRequest::edit_work_item(EditWorkItem::new("001", changes, "2026-01-03")),
        );
        let result = plan.apply();
        assert!(result.success, "{result:?}");
        let value: Value = serde_json::from_slice(
            &fs::read(root.join("work/active/001-one/work-item.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(value["title"], "Edited title");
        assert_eq!(value["updated"], "2026-01-03");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unrelated_file_not_in_read_set_does_not_stale_plan() {
        let (root, snapshot) = fixture();
        let plan = plan(
            &snapshot,
            MutationRequest::create_work_item(create_request("A New Thing", "2026-01-02")),
        );
        fs::write(root.join("unrelated.txt"), "outside the governed read set").unwrap();
        let result = plan.apply();
        assert!(result.success, "{result:?}");
        assert!(
            !repopact_validation::validate(&root).has_errors(),
            "the committed candidate must pass canonical post-apply validation"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn drift_during_apply_is_rejected_and_rolled_back() {
        let (root, snapshot) = fixture();
        let plan = plan(
            &snapshot,
            MutationRequest::create_work_item(create_request("A New Thing", "2026-01-02")),
        );
        let result = plan.apply_with_options(&ApplyOptions::external_drift(
            0,
            "work/active/002-a-new-thing/work-item.json",
        ));
        assert!(!result.success);
        assert!(result.rolled_back);
        assert!(!root.join("work/active/002-a-new-thing").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn consumed_work_record_drift_is_stale() {
        let (root, snapshot) = fixture();
        let plan = plan(
            &snapshot,
            MutationRequest::create_work_item(create_request("A New Thing", "2026-01-02")),
        );
        fs::write(
            root.join("work/active/001-one/work-item.json"),
            br#"{"id":"001","title":"Changed","status":"active","owner_scope":"governance","affected_scopes":[],"depends_on":[],"acceptance_criteria":[{"id":"AC-1","text":"prove","state":"pending","evidence":[]}],"created":"2026-01-01","updated":"2026-01-01"}"#,
        )
        .unwrap();
        let result = plan.apply();
        assert!(!result.success);
        assert!(result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "mutation.plan-stale"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn consumed_owner_and_dashboard_inputs_are_stale() {
        for relative in ["governance/owners.json", "audits/reports/dashboard.md"] {
            let (root, snapshot) = fixture();
            let plan = plan(
                &snapshot,
                MutationRequest::create_work_item(create_request("A New Thing", "2026-01-02")),
            );
            fs::write(root.join(relative), b"changed input\n").unwrap();
            let result = plan.apply();
            assert!(!result.success, "{relative}: {result:?}");
            assert!(
                result
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.code == "mutation.plan-stale"),
                "{relative}: {result:?}"
            );
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn canonical_edit_fixture_matches_exact_after_state() {
        let before: Value =
            serde_json::from_str(include_str!("../tests/fixtures/edit/before/work-item.json"))
                .unwrap();
        let record = repopact_repository::IndexedRecord {
            reference: repopact_types::RecordRef::new(
                repopact_types::RecordKind::WorkItem,
                "001",
                "work-item.json",
            ),
            path: PathBuf::from("work-item.json"),
            value: Ok(before),
            text: None,
            front_matter: Err("not Markdown".to_owned()),
        };
        let actual =
            edited_manifest(&record, &WorkItemEdits::title("Edited title"), "2026-01-03").unwrap();
        let expected = include_str!("../tests/fixtures/edit/after/work-item.json");
        assert_eq!(actual.replace("\r\n", "\n"), expected.replace("\r\n", "\n"));
    }

    #[test]
    fn canonical_transition_fixture_matches_exact_after_state() {
        let mut value: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/transition/before/work/completed/001-reopen/work-item.json"
        ))
        .unwrap();
        value["status"] = Value::String("active".to_owned());
        let actual = canonical_json(&value);
        let expected = include_str!(
            "../tests/fixtures/transition/after/work/active/001-reopen/work-item.json"
        );
        assert_eq!(actual.replace("\r\n", "\n"), expected.replace("\r\n", "\n"));
    }

    fn proposal_request(title: &str, key: &str) -> CreateWorkItem {
        create_request(title, "2026-10-27")
            .with_status("proposed")
            .with_idempotency_key(key)
    }

    #[test]
    fn proposal_reports_stale_dashboard_and_expired_review_as_warnings_then_refreshes_dashboard() {
        let (root, _) = fixture();
        fs::create_dir_all(root.join("assurance/mappings")).unwrap();
        fs::write(
            root.join("assurance/mappings/expired-review.json"),
            r#"{
                "$schema": "assurance-mapping.schema.json",
                "version": 1,
                "id": "expired-review",
                "framework": {"id": "example-framework", "source_authority": "adopter-extension"},
                "requirement": {"id": "AC-7"},
                "applicability": {"status": "unassessed"},
                "review": {"review_due_at": "2026-01-01T00:00:00Z"},
                "created": "2026-01-01",
                "updated": "2026-01-01"
            }"#,
        )
        .unwrap();
        let snapshot = RepositorySession::open(&root).snapshot();
        let full = repopact_validation::validate_snapshot(&snapshot);
        assert!(full.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "dashboard.stale" && diagnostic.severity == Severity::Error
        }));
        let plan = plan(
            &snapshot,
            MutationRequest::create_work_item(proposal_request("Scoped proposal", "retry-1")),
        );
        assert!(plan.is_applicable(), "{:?}", plan.diagnostics);
        for code in ["dashboard.stale", "assurance.review-overdue"] {
            assert!(
                plan.diagnostics
                    .iter()
                    .any(|diagnostic| { diagnostic.code == code && !diagnostic.blocking }),
                "expected non-blocking {code}: {:?}",
                plan.diagnostics
            );
        }
        let result = plan.apply();
        assert!(result.success, "{:?}", result.diagnostics);
        assert!(
            !result.diagnostics.iter().any(|diagnostic| {
                diagnostic.code == "dashboard.stale" || diagnostic.code == "dashboard.missing"
            }),
            "successful proposal should regenerate the dashboard: {:?}",
            result.diagnostics
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn proposal_rejects_candidate_scope_violation_and_preserves_global_reference_blockers() {
        let (root, snapshot) = fixture();
        let invalid_scope =
            proposal_request("Invalid scope", "scope-key").with_owner_scope("not-a-declared-scope");
        let scope_plan = plan(&snapshot, MutationRequest::create_work_item(invalid_scope));
        assert!(!scope_plan.is_applicable());
        assert!(scope_plan.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "work.unknown-owner-scope" && diagnostic.blocking
        }));

        let broken_reference = proposal_request("Broken reference", "reference-key")
            .with_dependencies(vec!["999".to_owned()]);
        let dependency_plan = plan(
            &snapshot,
            MutationRequest::create_work_item(broken_reference),
        );
        assert!(!dependency_plan.is_applicable());
        assert!(dependency_plan.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "work.unknown-dependency" && diagnostic.blocking
        }));

        let mut evidence_request = proposal_request("Broken evidence reference", "evidence-key");
        evidence_request.acceptance_criteria[0].evidence = vec!["missing-run".to_owned()];
        let evidence_plan = plan(
            &snapshot,
            MutationRequest::create_work_item(evidence_request),
        );
        assert!(!evidence_plan.is_applicable());
        assert!(evidence_plan.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "work.unknown-evidence" && diagnostic.blocking
        }));

        fs::write(
            root.join("evidence/runs/20261027-broken-reference.json"),
            r#"{
                "$schema": "../../schemas/evidence-run.schema.json",
                "id": "20261027-broken-reference",
                "timestamp": "2026-10-27T12:00:00Z",
                "work_item": "999",
                "result": "passed",
                "commands": [],
                "artifacts": [],
                "environment": {}
            }"#,
        )
        .unwrap();
        let snapshot = RepositorySession::open(&root).snapshot();
        let historical_reference_plan = plan(
            &snapshot,
            MutationRequest::create_work_item(proposal_request(
                "Broken historical evidence link",
                "historical-reference-key",
            )),
        );
        assert!(!historical_reference_plan.is_applicable());
        assert!(historical_reference_plan
            .diagnostics
            .iter()
            .any(|diagnostic| {
                diagnostic.code == "evidence.unknown-work-item" && diagnostic.blocking
            }));

        let work_path = root.join("work/active/001-one/work-item.json");
        let mut existing: Value = serde_json::from_slice(&fs::read(&work_path).unwrap()).unwrap();
        existing["depends_on"] = serde_json::json!(["002"]);
        fs::write(&work_path, canonical_json(&existing)).unwrap();
        let snapshot = RepositorySession::open(&root).snapshot();
        let cycle_request = proposal_request("Cycle candidate", "cycle-key")
            .with_dependencies(vec!["001".to_owned()]);
        let cycle_plan = plan(&snapshot, MutationRequest::create_work_item(cycle_request));
        assert!(!cycle_plan.is_applicable());
        assert!(cycle_plan.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "work.dependency-cycle" && diagnostic.blocking
        }));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_staged_proposal_leaves_no_work_item_or_receipt_and_can_be_retried() {
        let (root, snapshot) = fixture();
        let request = proposal_request("Atomic retry", "atomic-key");
        let plan = plan(
            &snapshot,
            MutationRequest::create_work_item(request.clone()),
        );
        assert!(plan.is_applicable(), "{:?}", plan.diagnostics);
        let failed = plan.apply_with_options(&ApplyOptions {
            fail_before_operation: Some(1),
            ..ApplyOptions::default()
        });
        assert!(!failed.success);
        assert!(!root.join("work/proposed/002-atomic-retry").exists());
        assert_eq!(proposal_status(&root, "atomic-key").unwrap(), None);

        let committed = propose_work_item(&root, request.clone());
        assert!(committed.success, "{:?}", committed.diagnostics);
        assert!(root
            .join("work/proposed/002-atomic-retry/proposal-receipt.json")
            .is_file());
        let replay = propose_work_item(&root, request);
        assert!(replay.success, "{:?}", replay.diagnostics);
        assert!(replay.replayed);
        assert!(replay
            .changed_paths
            .iter()
            .any(|path| path.ends_with("/work-item.json")));
        assert_eq!(
            fs::read_dir(root.join("work/proposed")).unwrap().count(),
            1,
            "same-key retry must not create a second work item"
        );
        assert!(committed
            .changed_paths
            .iter()
            .any(|path| path == "audits/reports/dashboard.md"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn proposal_idempotency_key_cannot_be_reused_for_a_different_payload() {
        let (root, _) = fixture();
        let first = propose_work_item(&root, proposal_request("First title", "bound-key"));
        assert!(first.success, "{:?}", first.diagnostics);
        let second = propose_work_item(&root, proposal_request("Other title", "bound-key"));
        assert!(!second.success);
        assert!(second
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "work.proposal-key-conflict"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn parallel_proposals_allocate_unique_ids_and_keep_receipts_resolvable() {
        let (root, _) = fixture();
        let root = std::sync::Arc::new(root);
        let workers = (0..4)
            .map(|index| {
                let root = root.clone();
                std::thread::spawn(move || {
                    let key = format!("parallel-{index}");
                    let request = proposal_request(&format!("Parallel {index}"), &key);
                    (key, propose_work_item(root.as_path(), request))
                })
            })
            .collect::<Vec<_>>();
        let mut ids = BTreeSet::new();
        for worker in workers {
            let (key, result) = worker.join().expect("proposal worker");
            assert!(result.success, "{key}: {:?}", result.diagnostics);
            let receipt = proposal_status(root.as_path(), &key)
                .unwrap()
                .expect("durable receipt");
            ids.insert(receipt["work_item_id"].as_str().unwrap().to_owned());
        }
        assert_eq!(ids.len(), 4);
        assert_eq!(ids.first().map(String::as_str), Some("002"));
        assert_eq!(ids.last().map(String::as_str), Some("005"));
        fs::remove_dir_all(root.as_path()).unwrap();
    }
}
