use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Condvar, Mutex, OnceLock};
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

mod transaction_store;
use transaction_store::{
    Journal as OrdinaryJournal, NativeRootIdentity, OrdinaryTransactionStore,
    Phase as JournalPhase, Receipt as OrdinaryReceipt, StoredPreimages,
};

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mutation_key: Option<String>,
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

    /// Bind this plan to a caller-supplied durable mutation key. The key is
    /// independent of the deterministic plan token and is persisted with the
    /// first transaction journal.
    pub fn with_mutation_key(mut self, key: impl Into<String>) -> Self {
        self.mutation_key = Some(key.into());
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplyOptions {
    pub fail_before_operation: Option<usize>,
    pub fail_after_operation: Option<usize>,
    pub external_drift_at: Option<usize>,
    pub external_drift_path: Option<String>,
    /// Test seam for a write between the initial stale check and preimage capture.
    pub external_drift_after_initial_check: Option<String>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mutation_key: Option<String>,
    #[serde(default)]
    pub transaction_outcome: String,
    pub plan_token: String,
    pub changed_paths: Vec<String>,
    pub diagnostics: Vec<MutationDiagnostic>,
    #[serde(default)]
    pub replayed: bool,
    #[serde(default)]
    pub metrics: MutationMetrics,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutationReceiptSummary {
    pub request_key: String,
    pub plan_token: String,
    pub outcome: String,
    pub success: bool,
    pub rolled_back: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutationStatus {
    /// `clear` means no active journal was observed; it does not prove that an
    /// absent request key was never used or that an out-of-band writer is idle.
    pub state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_request_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_phase: Option<String>,
    pub receipts: Vec<MutationReceiptSummary>,
    pub unknown_artifacts: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
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
    if matches!(&request, MutationRequest::CreateWorkItem(_)) {
        // ID allocation and global work-ledger checks depend on the complete
        // lifecycle inventory, including work items added after this snapshot.
        for status in LifecycleStatus::ALL {
            let relative = format!("work/{status}");
            read_facts.push(ReadFact {
                expected: repository.path_state(&repository.root().join(&relative)),
                path: relative,
            });
        }
    }
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
    let mutation_key = if matches!(&request, MutationRequest::CreateWorkItem(item) if item.status == "proposed")
    {
        None
    } else {
        Some(new_mutation_key())
    };
    let mut plan = MutationPlan {
        repository_identity: identity.clone(),
        plan_token: read_set.token(&identity),
        mutation_key,
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

/// Look up visible receipt state without changing the repository. `None` means
/// no matching receipt is present; it cannot prove that a prior commit never
/// occurred if receipt state was later removed.
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
    let _lock = match RepositoryMutationLock::acquire(repository.root()) {
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
    let ordinary_store = match OrdinaryTransactionStore::open(repository.root()) {
        Ok(store) => store,
        Err(error) => {
            result.diagnostics.push(MutationDiagnostic::error(
                "mutation.transaction-store",
                error,
            ));
            return result;
        }
    };
    if let Err(error) = ensure_no_pending_ordinary_transaction(&ordinary_store) {
        result.diagnostics.push(MutationDiagnostic::error(
            "mutation.recovery-required",
            error,
        ));
        return result;
    }
    let cleanup_started = Instant::now();
    if let Err(error) = ensure_safe_repository_directory(repository.root(), Path::new("work")) {
        result.metrics.filesystem_staging_ms += cleanup_started.elapsed().as_millis();
        result.diagnostics.push(MutationDiagnostic::error(
            "work.proposal-stage-cleanup-failed",
            error,
        ));
        return result;
    }
    if let Err(error) =
        ensure_safe_repository_directory(repository.root(), Path::new("work/proposed"))
    {
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
        mutation_key: None,
        transaction_outcome: "not_applied".to_owned(),
        plan_token: String::new(),
        changed_paths: Vec::new(),
        diagnostics: Vec::new(),
        replayed: false,
        metrics: MutationMetrics::default(),
    }
}

fn new_mutation_key() -> String {
    static NEXT_MUTATION_KEY: AtomicU64 = AtomicU64::new(1);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!(
        "rp-{}-{nanos}-{}",
        std::process::id(),
        NEXT_MUTATION_KEY.fetch_add(1, Ordering::Relaxed)
    )
}

/// Read transaction state without creating the storage anchor, taking a
/// recovery action, or deleting a journal. A missing keyed receipt is reported
/// as `unknown`, never as proof that the request did not run.
pub fn mutation_status(root: impl AsRef<Path>, request_key: Option<&str>) -> MutationStatus {
    if let Some(key) = request_key {
        if let Err(error) = transaction_store::validate_key(key) {
            return empty_mutation_status("recovery_required", Some(error));
        }
    }
    let store = match OrdinaryTransactionStore::open_read_only(root.as_ref()) {
        Ok(Some(store)) => store,
        Ok(None) => {
            return empty_mutation_status(
                if request_key.is_some() {
                    "unknown"
                } else {
                    "clear"
                },
                request_key
                    .map(|_| "no matching durable receipt or active journal was found".to_owned()),
            )
        }
        Err(error) => return empty_mutation_status("recovery_required", Some(error)),
    };
    let unknown_artifacts = match store.has_unknown_artifacts() {
        Ok(items) => items,
        Err(error) => return empty_mutation_status("recovery_required", Some(error)),
    };
    let journal = match store.load_journal() {
        Ok(journal) => journal,
        Err(error) => {
            let mut status = empty_mutation_status("recovery_required", Some(error));
            status.unknown_artifacts = unknown_artifacts;
            return status;
        }
    };
    let receipts = match store.receipts() {
        Ok(receipts) => receipts
            .into_iter()
            .filter(|receipt| request_key.is_none_or(|key| receipt.request_key == key))
            .map(|receipt| MutationReceiptSummary {
                request_key: receipt.request_key,
                plan_token: receipt.plan_token,
                outcome: receipt.result.transaction_outcome,
                success: receipt.result.success,
                rolled_back: receipt.result.rolled_back,
            })
            .collect::<Vec<_>>(),
        Err(error) => {
            let mut status = empty_mutation_status("recovery_required", Some(error));
            status.unknown_artifacts = unknown_artifacts;
            return status;
        }
    };
    let terminal_journal = match journal.as_ref() {
        Some(journal) => match store.load_receipt(&journal.request_key) {
            Ok(Some(receipt))
                if receipt.payload_sha256 == journal.payload_sha256
                    && receipt.plan_token == journal.plan_token
                    && receipt.native_root == journal.native_root
                    && receipt.storage_identity == journal.storage_identity =>
            {
                Some(journal)
            }
            Ok(Some(_)) => {
                let mut status = empty_mutation_status(
                    "recovery_required",
                    Some("terminal receipt conflicts with the active journal".to_owned()),
                );
                status.unknown_artifacts = unknown_artifacts;
                return status;
            }
            Ok(None) => None,
            Err(error) => {
                let mut status = empty_mutation_status("recovery_required", Some(error));
                status.unknown_artifacts = unknown_artifacts;
                return status;
            }
        },
        None => None,
    };
    let (active_request_key, active_phase) = journal
        .as_ref()
        .map(|journal| {
            let phase = if terminal_journal.is_some() {
                "terminal"
            } else {
                match journal.phase {
                    JournalPhase::Prepared => "prepared",
                    JournalPhase::Applying => "applying",
                    JournalPhase::RollingBack => "rolling_back",
                }
            };
            (Some(journal.request_key.clone()), Some(phase.to_owned()))
        })
        .unwrap_or((None, None));
    let matching_receipt =
        request_key.is_some_and(|key| receipts.iter().any(|receipt| receipt.request_key == key));
    let state = if !unknown_artifacts.is_empty() {
        "recovery_required"
    } else if terminal_journal.is_some() {
        "terminal"
    } else if journal.is_some() {
        "pending"
    } else if request_key.is_some() && !matching_receipt {
        "unknown"
    } else {
        "clear"
    };
    MutationStatus {
        state: state.to_owned(),
        active_request_key,
        active_phase,
        receipts,
        unknown_artifacts,
        detail: if state == "unknown" {
            Some(
                "receipt absence is unknown and does not prove that the request did not run"
                    .to_owned(),
            )
        } else if state == "terminal" {
            Some("terminal receipt is durable; journal cleanup remains pending and status made no writes".to_owned())
        } else {
            None
        },
    }
}

fn empty_mutation_status(state: &str, detail: Option<String>) -> MutationStatus {
    MutationStatus {
        state: state.to_owned(),
        active_request_key: None,
        active_phase: None,
        receipts: Vec::new(),
        unknown_artifacts: Vec::new(),
        detail,
    }
}

/// Explicitly recover the one pending ordinary mutation, if its request key
/// matches. This operation is the only public path that rolls back a journal.
pub fn recover_mutation(root: impl AsRef<Path>, request_key: Option<&str>) -> MutationResult {
    let mut result = empty_result();
    result.transaction_outcome = "unknown".to_owned();
    if let Some(key) = request_key {
        if let Err(error) = transaction_store::validate_key(key) {
            result.transaction_outcome = "rejected".to_owned();
            result.diagnostics.push(MutationDiagnostic::error(
                "mutation.request-key-invalid",
                error,
            ));
            return result;
        }
    }
    let repository = Repository::open(root);
    let _lock = match RepositoryMutationLock::acquire(repository.root()) {
        Ok(lock) => lock,
        Err(error) => {
            result.transaction_outcome = "recovery_required".to_owned();
            result
                .diagnostics
                .push(MutationDiagnostic::error("mutation.lock-timeout", error));
            return result;
        }
    };
    let store = match OrdinaryTransactionStore::open(repository.root()) {
        Ok(store) => store,
        Err(error) => {
            result.transaction_outcome = "recovery_required".to_owned();
            result.diagnostics.push(MutationDiagnostic::error(
                "mutation.transaction-store",
                error,
            ));
            return result;
        }
    };
    if let Err(error) = reconcile_terminal_journal(&store) {
        result.transaction_outcome = "recovery_required".to_owned();
        result.diagnostics.push(MutationDiagnostic::error(
            "mutation.recovery-required",
            error,
        ));
        return result;
    }
    match store.load_journal() {
        Ok(Some(journal)) => {
            if request_key.is_some_and(|key| key != journal.request_key) {
                result.transaction_outcome = "recovery_required".to_owned();
                result.mutation_key = Some(journal.request_key);
                result.diagnostics.push(MutationDiagnostic::error(
                    "mutation.recovery-key-mismatch",
                    "the requested key does not match the active ordinary transaction",
                ));
                return result;
            }
            result.mutation_key = Some(journal.request_key.clone());
            if let Err(error) = store.has_unknown_artifacts().and_then(|items| {
                if items.is_empty() {
                    Ok(())
                } else {
                    Err(format!(
                        "unknown transaction artifacts were preserved: {}",
                        items.join(", ")
                    ))
                }
            }) {
                result.transaction_outcome = "recovery_required".to_owned();
                result.diagnostics.push(MutationDiagnostic::error(
                    "mutation.recovery-required",
                    error,
                ));
                return result;
            }
            match store.receipt_count() {
                Ok(count) if count < transaction_store::MAX_RECEIPTS => {}
                Ok(_) => {
                    result.transaction_outcome = "recovery_required".to_owned();
                    result.diagnostics.push(MutationDiagnostic::error(
                        "mutation.receipt-capacity",
                        "terminal receipt capacity is exhausted; the active journal was preserved",
                    ));
                    return result;
                }
                Err(error) => {
                    result.transaction_outcome = "recovery_required".to_owned();
                    result.diagnostics.push(MutationDiagnostic::error(
                        "mutation.transaction-store",
                        error,
                    ));
                    return result;
                }
            }
            recover_journal(&repository, &store, journal, result)
        }
        Ok(None) => {
            if let Some(key) = request_key {
                match store.load_receipt(key) {
                    Ok(Some(receipt)) => {
                        result = receipt.result;
                        result.mutation_key = Some(key.to_owned());
                        result.replayed = true;
                        result.transaction_outcome = "replayed".to_owned();
                        result
                    }
                    Ok(None) => {
                        result.transaction_outcome = "unknown".to_owned();
                        result.mutation_key = Some(key.to_owned());
                        result.diagnostics.push(MutationDiagnostic::error(
                            "mutation.outcome-unknown",
                            "no receipt is present; absence does not prove that the request did not run",
                        ));
                        result
                    }
                    Err(error) => {
                        result.transaction_outcome = "recovery_required".to_owned();
                        result.diagnostics.push(MutationDiagnostic::error(
                            "mutation.transaction-store",
                            error,
                        ));
                        result
                    }
                }
            } else {
                result.transaction_outcome = "unknown".to_owned();
                result.diagnostics.push(MutationDiagnostic::error(
                    "mutation.outcome-unknown",
                    "no active journal was found; specify a request key to inspect its terminal receipt",
                ));
                result
            }
        }
        Err(error) => {
            result.transaction_outcome = "recovery_required".to_owned();
            result.diagnostics.push(MutationDiagnostic::error(
                "mutation.recovery-required",
                error,
            ));
            result
        }
    }
}

/// Lock and pending-state guard for the one direct engine writer that shares
/// an ordinary mutation target: dashboard projection generation.
pub struct RepositoryMutationWriteGuard {
    _lock: RepositoryMutationLock,
}

pub fn acquire_repository_mutation_write_guard(
    root: impl AsRef<Path>,
) -> Result<RepositoryMutationWriteGuard, String> {
    let repository = Repository::open(root);
    let lock = RepositoryMutationLock::acquire(repository.root())?;
    let store = OrdinaryTransactionStore::open(repository.root())?;
    reconcile_terminal_journal(&store)?;
    if let Some(journal) = store.load_journal()? {
        return Err(format!(
            "ordinary mutation '{}' is pending; dashboard write is blocked until explicit recovery",
            journal.request_key
        ));
    }
    let unknown = store.has_unknown_artifacts()?;
    if !unknown.is_empty() {
        return Err(format!(
            "transaction state has unknown artifacts; dashboard write is blocked: {}",
            unknown.join(", ")
        ));
    }
    Ok(RepositoryMutationWriteGuard { _lock: lock })
}

fn mutation_payload_digest(plan: &MutationPlan) -> String {
    let payload =
        serde_json::to_vec(&(&plan.repository_identity, &plan.request)).unwrap_or_default();
    hex_digest(Sha256::digest(payload))
}

fn persist_terminal_result(
    store: &OrdinaryTransactionStore,
    repository: &Repository,
    plan: &MutationPlan,
    request_key: &str,
    payload_sha256: &str,
    mut result: MutationResult,
    outcome: &str,
) -> MutationResult {
    result.mutation_key = Some(request_key.to_owned());
    result.transaction_outcome = outcome.to_owned();
    let receipt = OrdinaryReceipt {
        version: transaction_store::STORE_VERSION,
        request_key: request_key.to_owned(),
        payload_sha256: payload_sha256.to_owned(),
        repository_identity: plan.repository_identity.clone(),
        native_root: match NativeRootIdentity::capture(repository.root()) {
            Ok(identity) => identity,
            Err(error) => {
                result.transaction_outcome = "recovery_required".to_owned();
                result.diagnostics.push(MutationDiagnostic::error(
                    "mutation.transaction-root",
                    error,
                ));
                return result;
            }
        },
        storage_identity: match NativeRootIdentity::capture(store.anchor()) {
            Ok(identity) => identity,
            Err(error) => {
                result.transaction_outcome = "recovery_required".to_owned();
                result.diagnostics.push(MutationDiagnostic::error(
                    "mutation.transaction-storage",
                    error,
                ));
                return result;
            }
        },
        plan_token: plan.plan_token.clone(),
        result: result.clone(),
    };
    match store.write_receipt(&receipt) {
        Ok(()) => result,
        Err(error) => {
            result.transaction_outcome = "unknown".to_owned();
            result
                .diagnostics
                .push(MutationDiagnostic::error("mutation.receipt-persist", error));
            result
        }
    }
}

fn reconcile_terminal_journal(store: &OrdinaryTransactionStore) -> Result<(), String> {
    let Some(journal) = store.load_journal()? else {
        return Ok(());
    };
    let Some(receipt) = store.load_receipt(&journal.request_key)? else {
        return Ok(());
    };
    if receipt.payload_sha256 != journal.payload_sha256
        || receipt.plan_token != journal.plan_token
        || receipt.native_root != journal.native_root
        || receipt.storage_identity != journal.storage_identity
    {
        return Err("terminal receipt does not match the remaining transaction journal".to_owned());
    }
    store.delete_journal()
}

fn journal_steps(
    repository: &Repository,
    journal: &OrdinaryJournal,
) -> Result<Vec<Preimages>, String> {
    let before = journal.before.to_live()?;
    let listed_roots = journal
        .roots
        .iter()
        .map(|value| {
            validate_relative_path(Path::new(value))?;
            Ok(PathBuf::from(value))
        })
        .collect::<Result<BTreeSet<_>, String>>()?;
    if listed_roots != before.roots {
        return Err("transaction journal root set does not match its complete preimage".to_owned());
    }
    if journal.operations.len() > 10_000 {
        return Err("transaction journal operation count exceeds its bound".to_owned());
    }
    for operation in &journal.operations {
        for path in operation_paths(operation) {
            validate_relative_path(&path)?;
            if path.starts_with(Path::new("work/.repopact-transactions")) {
                return Err("transaction journal targets its own state directory".to_owned());
            }
            if !listed_roots.iter().any(|root| path.starts_with(root)) {
                return Err(format!(
                    "transaction path '{}' is outside the recorded root set",
                    path_string(&path)
                ));
            }
        }
    }
    let mut steps = vec![before.clone()];
    let mut state = before;
    for operation in &journal.operations {
        state = project_operation(repository, &state, operation)?;
        steps.push(state.clone());
    }
    if journal.step_sha256.len() != steps.len() {
        return Err("transaction journal sequence digest count is inconsistent".to_owned());
    }
    for (step, expected_digest) in steps.iter().zip(&journal.step_sha256) {
        if state_digest(step)? != *expected_digest {
            return Err("transaction journal sequence integrity check failed".to_owned());
        }
    }
    let after = journal.projected_after.to_live()?;
    if steps
        .last()
        .is_none_or(|last| last.entries != after.entries || last.roots != after.roots)
    {
        return Err(
            "transaction journal projected after-state does not match its operation sequence"
                .to_owned(),
        );
    }
    Ok(steps)
}

fn state_digest(preimages: &Preimages) -> Result<String, String> {
    let stored = StoredPreimages::from_live(preimages)?;
    let encoded = serde_json::to_vec(&stored)
        .map_err(|error| format!("encode transaction state fingerprint: {error}"))?;
    Ok(hex_digest(Sha256::digest(encoded)))
}

fn recover_journal(
    repository: &Repository,
    store: &OrdinaryTransactionStore,
    mut journal: OrdinaryJournal,
    mut result: MutationResult,
) -> MutationResult {
    result.mutation_key = Some(journal.request_key.clone());
    result.plan_token = journal.plan_token.clone();
    let steps = match journal_steps(repository, &journal) {
        Ok(steps) => steps,
        Err(error) => {
            result.transaction_outcome = "recovery_required".to_owned();
            result.diagnostics.push(MutationDiagnostic::error(
                "mutation.recovery-required",
                error,
            ));
            return result;
        }
    };
    let roots = match journal
        .roots
        .iter()
        .map(|value| validate_relative_path(Path::new(value)).map(|_| PathBuf::from(value)))
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(roots) => roots,
        Err(error) => {
            result.transaction_outcome = "recovery_required".to_owned();
            result.diagnostics.push(MutationDiagnostic::error(
                "mutation.recovery-required",
                error,
            ));
            return result;
        }
    };
    let current = match capture_preimages(repository, &roots) {
        Ok(current) => current,
        Err(error) => {
            result.transaction_outcome = "recovery_required".to_owned();
            result.diagnostics.push(MutationDiagnostic::error(
                "mutation.recovery-required",
                error,
            ));
            return result;
        }
    };
    let before = &steps[0];
    let completed = if current.entries == before.entries {
        0
    } else if let Some(index) = steps
        .iter()
        .enumerate()
        .skip(1)
        .find_map(|(index, state)| (state.entries == current.entries).then_some(index))
    {
        index
    } else {
        result.transaction_outcome = "recovery_required".to_owned();
        result.diagnostics.push(MutationDiagnostic::error(
            "mutation.recovery-required",
            "live paths match neither the captured before-state nor a complete authorized operation sequence; uncertain data was preserved",
        ));
        return result;
    };
    for index in (1..=completed).rev() {
        journal.phase = JournalPhase::RollingBack;
        journal.next_operation = index - 1;
        if let Err(error) = store.write_journal(&journal, false) {
            result.transaction_outcome = "recovery_required".to_owned();
            result.diagnostics.push(MutationDiagnostic::error(
                "mutation.recovery-required",
                error,
            ));
            return result;
        }
        let undo = UndoRecord {
            operation: journal.operations[index - 1].clone(),
            before: steps[index - 1].clone(),
            after: steps[index].clone(),
        };
        if let Err(error) = restore_operation(repository, &undo) {
            result.transaction_outcome = "recovery_required".to_owned();
            result.diagnostics.push(MutationDiagnostic::error(
                "mutation.recovery-required",
                format!("owned reverse operation could not be proven safe; uncertain data was preserved: {error}"),
            ));
            return result;
        }
        pause_at_test_phase("during_rollback");
        if let Err(error) = store.write_journal(&journal, false) {
            result.transaction_outcome = "recovery_required".to_owned();
            result.diagnostics.push(MutationDiagnostic::error(
                "mutation.recovery-required",
                error,
            ));
            return result;
        }
    }
    result.success = false;
    result.rolled_back = true;
    result.transaction_outcome = "rolled_back".to_owned();
    result.diagnostics.push(MutationDiagnostic::error(
        "mutation.transaction-recovered",
        if completed == 0 {
            "the live write set was already at its captured before-state; no repository path was rewritten"
        } else {
            "the interrupted ordinary mutation was reversed from a verified operation sequence"
        },
    ));
    persist_journal_receipt(store, &journal, result)
}

fn persist_journal_receipt(
    store: &OrdinaryTransactionStore,
    journal: &OrdinaryJournal,
    mut result: MutationResult,
) -> MutationResult {
    result.mutation_key = Some(journal.request_key.clone());
    result.plan_token = journal.plan_token.clone();
    let receipt = OrdinaryReceipt {
        version: transaction_store::STORE_VERSION,
        request_key: journal.request_key.clone(),
        payload_sha256: journal.payload_sha256.clone(),
        repository_identity: journal.repository_identity.clone(),
        native_root: journal.native_root.clone(),
        storage_identity: journal.storage_identity.clone(),
        plan_token: journal.plan_token.clone(),
        result: result.clone(),
    };
    if let Err(error) = store.write_receipt(&receipt) {
        result.transaction_outcome = "recovery_required".to_owned();
        result
            .diagnostics
            .push(MutationDiagnostic::error("mutation.receipt-persist", error));
        return result;
    }
    pause_at_test_phase("after_terminal_receipt_before_journal_cleanup");
    if let Err(error) = store.delete_journal() {
        result.diagnostics.push(MutationDiagnostic::error(
            "mutation.terminal-cleanup-failed",
            format!("terminal receipt is durable; journal cleanup remains pending: {error}"),
        ));
    }
    result
}

fn durable_failure_with_rollback(
    repository: &Repository,
    mut undo: Vec<UndoRecord>,
    mut result: MutationResult,
    message: String,
    ownership_conflict: Option<String>,
    store: &OrdinaryTransactionStore,
    journal: &mut OrdinaryJournal,
) -> MutationResult {
    result
        .diagnostics
        .push(MutationDiagnostic::error("mutation.apply-failed", message));
    let changed = !undo.is_empty();
    let mut errors = ownership_conflict.into_iter().collect::<Vec<_>>();
    journal.phase = JournalPhase::RollingBack;
    journal.next_operation = undo.len();
    if let Err(error) = store.write_journal(journal, false) {
        errors.push(format!("unable to persist rollback phase: {error}"));
    }
    let mut restored = 0usize;
    while let Some(record) = undo.pop() {
        if let Err(error) = restore_operation(repository, &record) {
            errors.push(error);
            break;
        }
        restored += 1;
        if restored == 1 {
            pause_at_test_phase("during_rollback");
        }
    }
    if errors.is_empty() {
        result.rolled_back = changed;
        result.transaction_outcome = if changed { "rolled_back" } else { "rejected" }.to_owned();
        if changed {
            result.diagnostics.push(MutationDiagnostic::error(
                "mutation.transaction-rolled-back",
                "the in-process failure was reversed using captured ownership checks",
            ));
        }
        return persist_journal_receipt(store, journal, result);
    }
    result.transaction_outcome = "recovery_required".to_owned();
    for error in errors {
        result.diagnostics.push(MutationDiagnostic::error(
            "mutation.rollback-failed",
            format!("recovery required; uncertain state was preserved: {error}"),
        ));
    }
    result
}

fn validate_idempotency_key(key: &str) -> Result<(), String> {
    if key.is_empty() || key.trim() != key || key.len() > 128 {
        Err("proposal idempotency key must contain 1 to 128 non-whitespace characters".to_owned())
    } else {
        Ok(())
    }
}

fn ensure_no_pending_ordinary_transaction(store: &OrdinaryTransactionStore) -> Result<(), String> {
    reconcile_terminal_journal(store)?;
    if let Some(journal) = store.load_journal()? {
        return Err(format!(
            "ordinary mutation '{}' is pending; proposals are blocked until explicit recovery",
            journal.request_key
        ));
    }
    let unknown = store.has_unknown_artifacts()?;
    if !unknown.is_empty() {
        return Err(format!(
            "transaction state contains unknown artifacts; preserved them: {}",
            unknown.join(", ")
        ));
    }
    Ok(())
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
    result.transaction_outcome = "replayed".to_owned();
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
            let receipt_key = value
                .get("idempotency_key")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    format!(
                        "proposal receipt {} has no identifiable idempotency key",
                        path.display()
                    )
                })?;
            validate_idempotency_key(receipt_key).map_err(|error| {
                format!(
                    "proposal receipt {} has an invalid idempotency key: {error}",
                    path.display()
                )
            })?;
            if receipt_key == key {
                if value.get("format_version").and_then(Value::as_u64) != Some(1) {
                    return Err(format!(
                        "proposal receipt {} has an unsupported format version",
                        path.display()
                    ));
                }
                let digest = value
                    .get("request_digest")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        format!("proposal receipt {} has no request digest", path.display())
                    })?;
                if digest.len() != 64
                    || !digest
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                {
                    return Err(format!(
                        "proposal receipt {} has an invalid request digest",
                        path.display()
                    ));
                }
                let receipt_id = value
                    .get("work_item_id")
                    .and_then(Value::as_str)
                    .filter(|id| !id.is_empty())
                    .ok_or_else(|| {
                        format!("proposal receipt {} has no work item ID", path.display())
                    })?;
                let historical_path = value
                    .get("work_item_path")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        format!("proposal receipt {} has no work item path", path.display())
                    })?;
                let historical = Path::new(historical_path);
                let components = historical.components().collect::<Vec<_>>();
                if components.len() != 3
                    || components[0] != std::path::Component::Normal("work".as_ref())
                    || !components[1]
                        .as_os_str()
                        .to_str()
                        .is_some_and(|status| LifecycleStatus::ALL.contains(&status))
                    || components
                        .iter()
                        .any(|component| !matches!(component, std::path::Component::Normal(_)))
                {
                    return Err(format!(
                        "proposal receipt {} has an unsafe work item path",
                        path.display()
                    ));
                }
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
                let item_bytes = fs::read(&item_path).map_err(|error| {
                    format!(
                        "proposal receipt {} has an unreadable work-item record: {error}",
                        path.display()
                    )
                })?;
                let item: Value = serde_json::from_slice(&item_bytes).map_err(|error| {
                    format!(
                        "proposal receipt {} has an invalid work-item record: {error}",
                        path.display()
                    )
                })?;
                if item.get("id").and_then(Value::as_str) != Some(receipt_id)
                    || item.get("status").and_then(Value::as_str) != Some(status)
                {
                    return Err(format!(
                        "proposal receipt {} does not match its current work-item record",
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

struct RepositoryMutationLock {
    file: File,
    local_key: String,
    #[cfg(windows)]
    overlapped: windows_sys::Win32::System::IO::OVERLAPPED,
}

static LOCAL_REPOSITORY_LOCKS: OnceLock<(Mutex<BTreeSet<String>>, Condvar)> = OnceLock::new();

impl RepositoryMutationLock {
    fn acquire(root: &Path) -> Result<Self, String> {
        let store = OrdinaryTransactionStore::open(root)?;
        let native_identity = NativeRootIdentity::capture(store.root())?;
        let identity = serde_json::to_vec(&native_identity)
            .map_err(|error| format!("encode working-root lock identity: {error}"))?;
        let key = hex_digest(Sha256::digest(identity));
        let deadline = Instant::now() + Duration::from_secs(2);
        let (held_keys, changed) =
            LOCAL_REPOSITORY_LOCKS.get_or_init(|| (Mutex::new(BTreeSet::new()), Condvar::new()));
        let mut held = held_keys.lock().unwrap_or_else(|error| error.into_inner());
        while held.contains(&key) {
            let now = Instant::now();
            if now >= deadline {
                return Err("repository mutation lock did not clear within 2000 ms; inspect operation status before retrying".to_owned());
            }
            let (next, timeout) = changed
                .wait_timeout(held, deadline.saturating_duration_since(now))
                .unwrap_or_else(|error| error.into_inner());
            held = next;
            if timeout.timed_out() && held.contains(&key) {
                return Err("repository mutation lock did not clear within 2000 ms; inspect operation status before retrying".to_owned());
            }
        }
        held.insert(key.clone());
        drop(held);

        let path = store.anchor().join("repository.lock");
        let mut lock_options = OpenOptions::new();
        lock_options
            .read(true)
            .write(true)
            .create(true)
            .truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            lock_options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            lock_options.custom_flags(0x0020_0000); // FILE_FLAG_OPEN_REPARSE_POINT
        }
        let file = match lock_options.open(&path) {
            Ok(file) => file,
            Err(error) => {
                release_local_repository_lock(&key);
                return Err(format!("unable to open repository mutation lock: {error}"));
            }
        };
        let metadata = match file.metadata() {
            Ok(metadata) => metadata,
            Err(error) => {
                release_local_repository_lock(&key);
                return Err(format!(
                    "unable to inspect repository mutation lock: {error}"
                ));
            }
        };
        if !metadata.is_file() || is_symlink_or_reparse(&metadata) {
            release_local_repository_lock(&key);
            return Err("repository mutation lock is not a regular file".to_owned());
        }
        if let Err(error) = ensure_single_link(&path, &metadata) {
            release_local_repository_lock(&key);
            return Err(error);
        }
        #[cfg(unix)]
        if let Err(error) = transaction_store::check_private_file(&path, &metadata) {
            release_local_repository_lock(&key);
            return Err(error);
        }
        #[cfg(windows)]
        if let Err(error) = transaction_store::check_private_directory(store.anchor()) {
            release_local_repository_lock(&key);
            return Err(error);
        }
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
                        local_key: key,
                        #[cfg(windows)]
                        overlapped,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        release_local_repository_lock(&key);
                        return Err("repository mutation lock did not clear within 2000 ms; inspect operation status before retrying".to_owned());
                    }
                    thread::sleep(Duration::from_millis(20));
                }
                Err(error) => {
                    release_local_repository_lock(&key);
                    return Err(format!(
                        "unable to acquire repository mutation lock: {error}"
                    ));
                }
            }
        }
    }
}

impl Drop for RepositoryMutationLock {
    fn drop(&mut self) {
        unlock_file(
            &self.file,
            #[cfg(windows)]
            &mut self.overlapped,
        );
        release_local_repository_lock(&self.local_key);
    }
}

fn release_local_repository_lock(key: &str) {
    if let Some((held_keys, changed)) = LOCAL_REPOSITORY_LOCKS.get() {
        let mut held = held_keys.lock().unwrap_or_else(|error| error.into_inner());
        held.remove(key);
        changed.notify_all();
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
        mutation_key: if matches!(&request, MutationRequest::CreateWorkItem(item) if item.status == "proposed")
        {
            None
        } else {
            Some(new_mutation_key())
        },
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

// Candidate plan trees are private to each invocation. Do not reuse a path
// merely because another thread/process observed the same Windows clock tick.
static CANDIDATE_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
const CANDIDATE_TEMP_COLLISION_LIMIT: usize = 64;

fn create_candidate_temp_under(
    parent: &Path,
    stamp: u128,
    counter: &AtomicU64,
) -> Result<PathBuf, String> {
    for _ in 0..CANDIDATE_TEMP_COLLISION_LIMIT {
        let sequence = counter.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.join(format!(
            "repopact-mutation-plan-{}-{stamp}-{sequence}",
            std::process::id()
        ));
        match fs::create_dir(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!(
                    "cannot exclusively create candidate directory {candidate:?}: {error}"
                ));
            }
        }
    }
    Err(format!(
        "candidate directory allocation exhausted {CANDIDATE_TEMP_COLLISION_LIMIT} attempts"
    ))
}

fn validate_candidate(
    root: &Path,
    operations: &[PathOperation],
) -> Result<(String, Vec<MutationDiagnostic>), String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("cannot timestamp validation candidate: {error}"))?
        .as_nanos();
    let temporary =
        create_candidate_temp_under(&std::env::temp_dir(), stamp, &CANDIDATE_TEMP_COUNTER)?;
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

    // A cleanup failure is a real failure, even after successful validation.
    // Do not report success while leaving a live candidate tree behind.
    match fs::remove_dir_all(&temporary) {
        Ok(()) => result,
        Err(error) => match result {
            Ok(_) => Err(format!(
                "unable to remove validation candidate {temporary:?}: {error}"
            )),
            Err(cause) => Err(format!(
                "{cause}; unable to remove validation candidate {temporary:?}: {error}"
            )),
        },
    }
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
    let repository_root = fs::canonicalize(source).map_err(|error| error.to_string())?;
    copy_tree_inner(source, destination, true, &repository_root)
}

fn copy_tree_inner(
    source: &Path,
    destination: &Path,
    repository_root: bool,
    canonical_root: &Path,
) -> Result<(), String> {
    fs::create_dir_all(destination).map_err(|error| error.to_string())?;
    let mut entries = fs::read_dir(source)
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| path_string(&left.path()).cmp(&path_string(&right.path())));
    for entry in entries {
        let name = entry.file_name().to_string_lossy().to_string();
        let from = entry.path();
        if from == canonical_root.join("work/.repopact-transactions/ordinary-v1") {
            continue;
        }
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
            copy_tree_inner(&from, &to, false, canonical_root)?;
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
        mutation_key: plan.mutation_key.clone(),
        transaction_outcome: "unknown".to_owned(),
        plan_token: plan.plan_token.clone(),
        changed_paths: Vec::new(),
        diagnostics: plan.diagnostics.clone(),
        replayed: false,
        metrics: plan.metrics.clone(),
    };
    if matches!(&plan.request, MutationRequest::CreateWorkItem(item) if item.status == "proposed") {
        return apply_proposal(plan, options, result);
    }
    let Some(request_key) = plan.mutation_key.as_deref() else {
        result.transaction_outcome = "rejected".to_owned();
        result.diagnostics.push(MutationDiagnostic::error(
            "mutation.request-key-required",
            "ordinary mutation plan has no durable request key; re-plan before applying",
        ));
        return result;
    };
    if let Err(error) = transaction_store::validate_key(request_key) {
        result.transaction_outcome = "rejected".to_owned();
        result.diagnostics.push(MutationDiagnostic::error(
            "mutation.request-key-invalid",
            error,
        ));
        return result;
    }
    let repository = Repository::open(&plan.repository_identity.root);
    let _lock = match RepositoryMutationLock::acquire(repository.root()) {
        Ok(lock) => lock,
        Err(error) => {
            result.transaction_outcome = "recovery_required".to_owned();
            result
                .diagnostics
                .push(MutationDiagnostic::error("mutation.lock-timeout", error));
            return result;
        }
    };
    let store = match OrdinaryTransactionStore::open(repository.root()) {
        Ok(store) => store,
        Err(error) => {
            result.transaction_outcome = "recovery_required".to_owned();
            result.diagnostics.push(MutationDiagnostic::error(
                "mutation.transaction-store",
                error,
            ));
            return result;
        }
    };
    let payload_sha256 = mutation_payload_digest(plan);
    if let Err(error) = reconcile_terminal_journal(&store) {
        result.transaction_outcome = "recovery_required".to_owned();
        result.diagnostics.push(MutationDiagnostic::error(
            "mutation.recovery-required",
            error,
        ));
        return result;
    }
    match store.load_receipt(request_key) {
        Ok(Some(receipt)) if receipt.payload_sha256 == payload_sha256 => {
            let mut replay = receipt.result;
            replay.replayed = true;
            replay.mutation_key = Some(request_key.to_owned());
            replay.transaction_outcome = "replayed".to_owned();
            return replay;
        }
        Ok(Some(_)) => {
            result.transaction_outcome = "conflict".to_owned();
            result.diagnostics.push(MutationDiagnostic::error(
                "mutation.request-key-conflict",
                "this mutation request key is already bound to a different intent",
            ));
            return result;
        }
        Ok(None) => {}
        Err(error) => {
            result.transaction_outcome = "recovery_required".to_owned();
            result.diagnostics.push(MutationDiagnostic::error(
                "mutation.transaction-store",
                error,
            ));
            return result;
        }
    }
    match store.load_journal() {
        Ok(Some(journal)) => {
            result.transaction_outcome = "recovery_required".to_owned();
            result.diagnostics.push(MutationDiagnostic::error(
                "mutation.recovery-required",
                format!("request '{}' has an unresolved ordinary mutation; inspect status and explicitly recover it", journal.request_key),
            ));
            return result;
        }
        Ok(None) => {}
        Err(error) => {
            result.transaction_outcome = "recovery_required".to_owned();
            result.diagnostics.push(MutationDiagnostic::error(
                "mutation.recovery-required",
                error,
            ));
            return result;
        }
    }
    match store.has_unknown_artifacts() {
        Ok(unknown) if unknown.is_empty() => {}
        Ok(unknown) => {
            result.transaction_outcome = "recovery_required".to_owned();
            result.diagnostics.push(MutationDiagnostic::error(
                "mutation.recovery-required",
                format!(
                    "transaction storage contains an unknown artifact; preserved it: {}",
                    unknown.join(", ")
                ),
            ));
            return result;
        }
        Err(error) => {
            result.transaction_outcome = "recovery_required".to_owned();
            result.diagnostics.push(MutationDiagnostic::error(
                "mutation.transaction-store",
                error,
            ));
            return result;
        }
    }
    match store.receipts() {
        Ok(_) => {}
        Err(error) => {
            result.transaction_outcome = "recovery_required".to_owned();
            result.diagnostics.push(MutationDiagnostic::error(
                "mutation.transaction-store",
                error,
            ));
            return result;
        }
    }
    match store.receipt_count() {
        Ok(count) if count < transaction_store::MAX_RECEIPTS => {}
        Ok(_) => {
            result.transaction_outcome = "rejected".to_owned();
            result.diagnostics.push(MutationDiagnostic::error(
                "mutation.receipt-capacity",
                "terminal receipt capacity is exhausted; no history was evicted and no mutation was applied",
            ));
            return result;
        }
        Err(error) => {
            result.transaction_outcome = "recovery_required".to_owned();
            result.diagnostics.push(MutationDiagnostic::error(
                "mutation.transaction-store",
                error,
            ));
            return result;
        }
    }
    if !plan.is_applicable() {
        result.transaction_outcome = "rejected".to_owned();
        return persist_terminal_result(
            &store,
            &repository,
            plan,
            request_key,
            &payload_sha256,
            result,
            "rejected",
        );
    }
    if repository.identity() != plan.repository_identity {
        result.diagnostics.push(MutationDiagnostic::error(
            "mutation.repository-identity",
            "repository identity changed since planning",
        ));
        result.transaction_outcome = "rejected".to_owned();
        return persist_terminal_result(
            &store,
            &repository,
            plan,
            request_key,
            &payload_sha256,
            result,
            "rejected",
        );
    }
    match stale_read_fact(&repository, &plan.read_set) {
        Ok(Some(stale)) => {
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
            result.transaction_outcome = "rejected".to_owned();
            return persist_terminal_result(
                &store,
                &repository,
                plan,
                request_key,
                &payload_sha256,
                result,
                "rejected",
            );
        }
        Err(error) => {
            result.diagnostics.push(MutationDiagnostic::error(
                "mutation.plan-stale",
                format!("unable to safely verify the plan read set: {error}"),
            ));
            result.transaction_outcome = "rejected".to_owned();
            return persist_terminal_result(
                &store,
                &repository,
                plan,
                request_key,
                &payload_sha256,
                result,
                "rejected",
            );
        }
        Ok(None) => {}
    }

    if let Some(path) = options.external_drift_after_initial_check.as_deref() {
        let relative = Path::new(path);
        if let Err(error) = validate_relative_path(relative) {
            result.diagnostics.push(MutationDiagnostic::error(
                "mutation.preimage-failed",
                format!("invalid injected drift path: {error}"),
            ));
            result.transaction_outcome = "rejected".to_owned();
            return persist_terminal_result(
                &store,
                &repository,
                plan,
                request_key,
                &payload_sha256,
                result,
                "rejected",
            );
        }
        if let Err(error) = validate_path_chain(repository.root(), relative) {
            result.diagnostics.push(MutationDiagnostic::error(
                "mutation.preimage-failed",
                format!("unsafe injected drift path: {error}"),
            ));
            result.transaction_outcome = "rejected".to_owned();
            return persist_terminal_result(
                &store,
                &repository,
                plan,
                request_key,
                &payload_sha256,
                result,
                "rejected",
            );
        }
        let absolute = repository.root().join(relative);
        if let Err(error) = fs::write(
            &absolute,
            br#"{"id":"001","title":"External edit","status":"active","owner_scope":"governance","affected_scopes":[],"depends_on":[],"acceptance_criteria":[{"id":"AC-1","text":"prove","state":"pending","evidence":[]}],"created":"2026-01-01","updated":"2026-01-02"}"#,
        ) {
            result.diagnostics.push(MutationDiagnostic::error(
                "mutation.preimage-failed",
                format!("unable to inject external drift after stale check: {error}"),
            ));
            result.transaction_outcome = "rejected".to_owned();
            return persist_terminal_result(&store, &repository, plan, request_key, &payload_sha256, result, "rejected");
        }
    }

    let roots = match operation_roots(plan, &repository) {
        Ok(roots) => roots,
        Err(error) => {
            result.diagnostics.push(MutationDiagnostic::error(
                "mutation.preimage-failed",
                format!("unable to establish safe mutation paths: {error}"),
            ));
            result.transaction_outcome = "rejected".to_owned();
            return persist_terminal_result(
                &store,
                &repository,
                plan,
                request_key,
                &payload_sha256,
                result,
                "rejected",
            );
        }
    };
    let preimages = match capture_preimages(&repository, &roots) {
        Ok(preimages) => preimages,
        Err(error) => {
            result.diagnostics.push(MutationDiagnostic::error(
                "mutation.preimage-failed",
                format!("unable to capture complete mutation preimage: {error}"),
            ));
            result.transaction_outcome = "rejected".to_owned();
            return persist_terminal_result(
                &store,
                &repository,
                plan,
                request_key,
                &payload_sha256,
                result,
                "rejected",
            );
        }
    };
    if let Some(stale) = stale_read_fact_in_preimages(&repository, &plan.read_set, &preimages) {
        result.diagnostics.push(
            MutationDiagnostic::error(
                "mutation.plan-stale",
                format!(
                    "captured state for plan read fact '{}' no longer matches; re-plan before apply",
                    stale.path
                ),
            )
            .at(stale.path),
        );
        result.transaction_outcome = "rejected".to_owned();
        return persist_terminal_result(
            &store,
            &repository,
            plan,
            request_key,
            &payload_sha256,
            result,
            "rejected",
        );
    }
    match stale_read_fact(&repository, &plan.read_set) {
        Ok(Some(stale)) => {
            result.diagnostics.push(
                MutationDiagnostic::error(
                    "mutation.plan-stale",
                    format!(
                        "plan read fact for '{}' changed before the first write; re-plan before apply",
                        stale.path
                    ),
                )
                .at(stale.path),
            );
            result.transaction_outcome = "rejected".to_owned();
            return persist_terminal_result(
                &store,
                &repository,
                plan,
                request_key,
                &payload_sha256,
                result,
                "rejected",
            );
        }
        Err(error) => {
            result.diagnostics.push(MutationDiagnostic::error(
                "mutation.plan-stale",
                format!(
                    "unable to safely recheck the plan read set before the first write: {error}"
                ),
            ));
            result.transaction_outcome = "rejected".to_owned();
            return persist_terminal_result(
                &store,
                &repository,
                plan,
                request_key,
                &payload_sha256,
                result,
                "rejected",
            );
        }
        Ok(None) => {}
    }
    let mut projected_steps = vec![preimages.clone()];
    for operation in &plan.file_operations {
        let Some(previous) = projected_steps.last() else {
            unreachable!("the before-state is always the first projected step");
        };
        let projected = match project_operation(&repository, previous, operation) {
            Ok(projected) => projected,
            Err(error) => {
                result.transaction_outcome = "rejected".to_owned();
                result.diagnostics.push(MutationDiagnostic::error(
                    "mutation.projection-failed",
                    error,
                ));
                return persist_terminal_result(
                    &store,
                    &repository,
                    plan,
                    request_key,
                    &payload_sha256,
                    result,
                    "rejected",
                );
            }
        };
        projected_steps.push(projected);
    }
    let before = match StoredPreimages::from_live(&preimages) {
        Ok(before) => before,
        Err(error) => {
            result.transaction_outcome = "rejected".to_owned();
            result
                .diagnostics
                .push(MutationDiagnostic::error("mutation.preimage-limit", error));
            return persist_terminal_result(
                &store,
                &repository,
                plan,
                request_key,
                &payload_sha256,
                result,
                "rejected",
            );
        }
    };
    let projected_after = match projected_steps
        .last()
        .and_then(|state| StoredPreimages::from_live(state).ok())
    {
        Some(after) => after,
        None => {
            result.transaction_outcome = "rejected".to_owned();
            result.diagnostics.push(MutationDiagnostic::error(
                "mutation.projection-limit",
                "projected transaction state could not be bounded for journaling",
            ));
            return persist_terminal_result(
                &store,
                &repository,
                plan,
                request_key,
                &payload_sha256,
                result,
                "rejected",
            );
        }
    };
    let step_sha256 = match projected_steps
        .iter()
        .map(state_digest)
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(digests) => digests,
        Err(error) => {
            result.transaction_outcome = "rejected".to_owned();
            result.diagnostics.push(MutationDiagnostic::error(
                "mutation.projection-limit",
                error,
            ));
            return persist_terminal_result(
                &store,
                &repository,
                plan,
                request_key,
                &payload_sha256,
                result,
                "rejected",
            );
        }
    };
    let native_root = match NativeRootIdentity::capture(repository.root()) {
        Ok(identity) => identity,
        Err(error) => {
            result.transaction_outcome = "rejected".to_owned();
            result.diagnostics.push(MutationDiagnostic::error(
                "mutation.transaction-root",
                error,
            ));
            return persist_terminal_result(
                &store,
                &repository,
                plan,
                request_key,
                &payload_sha256,
                result,
                "rejected",
            );
        }
    };
    let mut journal = OrdinaryJournal {
        version: transaction_store::STORE_VERSION,
        request_key: request_key.to_owned(),
        payload_sha256: payload_sha256.clone(),
        repository_identity: plan.repository_identity.clone(),
        native_root,
        storage_identity: match NativeRootIdentity::capture(store.anchor()) {
            Ok(identity) => identity,
            Err(error) => {
                result.transaction_outcome = "rejected".to_owned();
                result.diagnostics.push(MutationDiagnostic::error(
                    "mutation.transaction-storage",
                    error,
                ));
                return persist_terminal_result(
                    &store,
                    &repository,
                    plan,
                    request_key,
                    &payload_sha256,
                    result,
                    "rejected",
                );
            }
        },
        plan_token: plan.plan_token.clone(),
        request: plan.request.clone(),
        operations: plan.file_operations.clone(),
        roots: before.roots.clone(),
        before,
        projected_after,
        step_sha256,
        phase: JournalPhase::Prepared,
        next_operation: 0,
    };
    pause_at_test_phase("before_prepare");
    if let Err(error) = store.write_journal(&journal, true) {
        result.transaction_outcome = "unknown".to_owned();
        result.diagnostics.push(MutationDiagnostic::error(
            "mutation.transaction-prepare",
            error,
        ));
        return result;
    }
    pause_at_test_phase("after_prepare_before_write");
    let mut expected = plan
        .file_operations
        .iter()
        .flat_map(operation_paths)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|path| {
            let state = preimages
                .state_for(&repository, &path)
                .expect("every planned operation path is covered by its preimage roots");
            (path, state)
        })
        .collect::<BTreeMap<_, _>>();
    let mut undo = Vec::<UndoRecord>::new();
    for (index, operation) in plan.file_operations.iter().enumerate() {
        journal.phase = JournalPhase::Applying;
        journal.next_operation = index;
        if let Err(error) = store.write_journal(&journal, false) {
            return durable_failure_with_rollback(
                &repository,
                undo,
                result,
                format!("unable to persist operation {index} checkpoint: {error}"),
                None,
                &store,
                &mut journal,
            );
        }
        if index > 0 {
            pause_at_test_phase("between_writes");
        }
        if options.external_drift_at == Some(index) {
            if let Some(path) = options.external_drift_path.as_deref() {
                let absolute = repository.root().join(path);
                if let Some(parent) = absolute.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                if let Err(error) = fs::write(&absolute, b"external drift injected during apply") {
                    return durable_failure_with_rollback(
                        &repository,
                        undo,
                        result,
                        format!("unable to inject external drift: {error}"),
                        None,
                        &store,
                        &mut journal,
                    );
                }
            }
        }
        let operation_roots = match checked_operation_roots(operation, &repository) {
            Ok(roots) => roots,
            Err(error) => {
                return durable_failure_with_rollback(
                    &repository,
                    undo,
                    result,
                    format!("unable to recheck operation paths before operation {index}: {error}"),
                    None,
                    &store,
                    &mut journal,
                );
            }
        };
        let before = match capture_preimages(&repository, &operation_roots) {
            Ok(before) => before,
            Err(error) => {
                return durable_failure_with_rollback(
                    &repository,
                    undo,
                    result,
                    format!("unable to recheck preimage before operation {index}: {error}"),
                    None,
                    &store,
                    &mut journal,
                );
            }
        };
        if let Some(stale_path) = operation_drift(operation, &repository, &before, &expected) {
            return durable_failure_with_rollback(
                &repository,
                undo,
                result,
                format!("target '{stale_path}' changed during apply; refusing to merge external content"),
                None,
                &store,
                &mut journal,
            );
        }
        if options.fail_before_operation == Some(index) {
            return durable_failure_with_rollback(
                &repository,
                undo,
                result,
                format!("failure injected before operation {index}"),
                None,
                &store,
                &mut journal,
            );
        }
        let expected_after = match project_operation(&repository, &before, operation) {
            Ok(after) => after,
            Err(error) => {
                return durable_failure_with_rollback(
                    &repository,
                    undo,
                    result,
                    format!("operation {index} cannot be applied safely: {error}"),
                    None,
                    &store,
                    &mut journal,
                );
            }
        };
        let apply_result = apply_operation(repository.root(), operation);
        let after = match capture_preimages(&repository, &operation_roots) {
            Ok(after) => after,
            Err(error) => {
                return durable_failure_with_rollback(
                    &repository,
                    undo,
                    result,
                    format!("unable to verify operation {index} after apply: {error}"),
                    Some(format!(
                        "operation {index} after-state could not be inspected; recovery is required and its paths were left untouched"
                    )),
                    &store,
                    &mut journal,
                );
            }
        };
        if after.entries != expected_after.entries {
            let unchanged = after.entries == before.entries;
            let apply_message = match apply_result {
                Err(error) => format!("apply failed: {error}"),
                Ok(()) => format!("operation {index} produced an unexpected after-state"),
            };
            return durable_failure_with_rollback(
                &repository,
                undo,
                result,
                apply_message,
                (!unchanged).then(|| {
                    format!(
                        "operation {index} after-state does not match the transaction's projected state; recovery is required and the uncertain paths were left untouched"
                    )
                }),
                &store,
                &mut journal,
            );
        }
        let changed = after.entries != before.entries;
        if changed {
            undo.push(UndoRecord {
                operation: operation.clone(),
                before,
                after: expected_after,
            });
        }
        if let Err(error) = apply_result {
            return durable_failure_with_rollback(
                &repository,
                undo,
                result,
                format!("apply failed: {error}"),
                None,
                &store,
                &mut journal,
            );
        }
        for (path, state) in &mut expected {
            if let Some(observed) = after.state_for(&repository, path) {
                *state = observed;
            }
        }
        journal.next_operation = index + 1;
        if let Err(error) = store.write_journal(&journal, false) {
            return durable_failure_with_rollback(
                &repository,
                undo,
                result,
                format!("unable to persist completion checkpoint for operation {index}: {error}"),
                None,
                &store,
                &mut journal,
            );
        }
        if index + 1 == plan.file_operations.len() {
            pause_at_test_phase("after_last_write_before_validation");
        }
        if options.fail_after_operation == Some(index) {
            return durable_failure_with_rollback(
                &repository,
                undo,
                result,
                format!("failure injected after operation {index}"),
                None,
                &store,
                &mut journal,
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
        return durable_failure_with_rollback(
            &repository,
            undo,
            result,
            format!("post-state validation failed: {messages}"),
            None,
            &store,
            &mut journal,
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
    result.transaction_outcome = "committed".to_owned();
    pause_at_test_phase("after_validation_before_terminal_receipt");
    let terminal = persist_journal_receipt(&store, &journal, result);
    pause_at_test_phase("after_terminal_receipt_before_response");
    terminal
}

#[cfg(test)]
fn pause_at_test_phase(phase: &str) {
    use std::io::{Read, Write};
    if std::env::var("REPOPACT_MUTATION_PAUSE_AT").ok().as_deref() != Some(phase) {
        return;
    }
    let mut stdout = std::io::stdout().lock();
    let _ = writeln!(stdout, "REPOPACT_MUTATION_PHASE:{phase}");
    let _ = stdout.flush();
    let mut release = [0u8; 1];
    let _ = std::io::stdin().lock().read(&mut release);
}

#[cfg(not(test))]
fn pause_at_test_phase(_phase: &str) {}

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
    let _lock = match RepositoryMutationLock::acquire(repository.root()) {
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
    let ordinary_store = match OrdinaryTransactionStore::open(repository.root()) {
        Ok(store) => store,
        Err(error) => {
            result.diagnostics.push(MutationDiagnostic::error(
                "mutation.transaction-store",
                error,
            ));
            return result;
        }
    };
    if let Err(error) = ensure_no_pending_ordinary_transaction(&ordinary_store) {
        result.diagnostics.push(MutationDiagnostic::error(
            "mutation.recovery-required",
            error,
        ));
        return result;
    }
    let cleanup_started = Instant::now();
    if let Err(error) = ensure_safe_repository_directory(repository.root(), Path::new("work")) {
        result.metrics.filesystem_staging_ms += cleanup_started.elapsed().as_millis();
        result.diagnostics.push(MutationDiagnostic::error(
            "work.proposal-stage-cleanup-failed",
            error,
        ));
        return result;
    }
    if let Err(error) =
        ensure_safe_repository_directory(repository.root(), Path::new("work/proposed"))
    {
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
    let stale_plan = match stale_read_fact(&repository, &plan.read_set) {
        Ok(stale) => stale.is_some(),
        Err(error) => {
            result.diagnostics.push(MutationDiagnostic::error(
                "mutation.plan-stale",
                format!("unable to safely verify proposal read facts: {error}"),
            ));
            return result;
        }
    };
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
    result: MutationResult,
) -> MutationResult {
    commit_proposal_locked_with_hook(repository, plan, snapshot, options, result, || {})
}

fn commit_proposal_locked_with_hook<F>(
    repository: &Repository,
    plan: &MutationPlan,
    snapshot: &RepositorySnapshot,
    options: &ApplyOptions,
    mut result: MutationResult,
    before_publish: F,
) -> MutationResult
where
    F: FnOnce(),
{
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
            if options.external_drift_at == Some(index) {
                if let Some(path) = options.external_drift_path.as_deref() {
                    let absolute = repository.root().join(path);
                    if let Some(parent) = absolute.parent() {
                        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
                    }
                    fs::write(
                        &absolute,
                        b"external drift injected during proposal staging",
                    )
                    .map_err(|error| error.to_string())?;
                }
            }
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
        before_publish();
        match stale_read_fact(repository, &plan.read_set) {
            Ok(Some(stale)) => {
                return Err(format!(
                    "proposal read fact '{}' changed after validation",
                    stale.path
                ));
            }
            Err(error) => {
                return Err(format!(
                    "cannot safely recheck proposal read facts: {error}"
                ));
            }
            Ok(None) => {}
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
    result.transaction_outcome = "committed".to_owned();
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
            Ok(metadata) if metadata.file_type().is_dir() && !is_symlink_or_reparse(&metadata) => {}
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
                        if !metadata.file_type().is_dir() || is_symlink_or_reparse(&metadata) {
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

fn stale_read_fact(
    repository: &Repository,
    read_set: &ReadSet,
) -> Result<Option<ReadFact>, String> {
    for fact in &read_set.facts {
        let actual = checked_read_fact_state(repository, Path::new(&fact.path))?;
        if actual != fact.expected {
            return Ok(Some(fact.clone()));
        }
    }
    Ok(None)
}

fn stale_read_fact_in_preimages(
    repository: &Repository,
    read_set: &ReadSet,
    preimages: &Preimages,
) -> Option<ReadFact> {
    read_set.facts.iter().find_map(|fact| {
        preimages
            .state_for(repository, Path::new(&fact.path))
            .filter(|actual| actual != &fact.expected)
            .map(|_| fact.clone())
    })
}

fn checked_read_fact_state(repository: &Repository, relative: &Path) -> Result<PathState, String> {
    let mut read_file = |path: &Path| fs::read(path);
    let mut read_directory = |path: &Path| fs::read_dir(path);
    observe_path_state(
        repository.root(),
        relative,
        &mut read_file,
        &mut read_directory,
        false,
    )
}

fn operation_roots(plan: &MutationPlan, repository: &Repository) -> Result<Vec<PathBuf>, String> {
    let mut roots = BTreeSet::new();
    for operation in &plan.file_operations {
        for path in checked_operation_roots(operation, repository)? {
            roots.insert(path);
        }
    }
    Ok(prune_roots(roots.into_iter().collect()))
}

fn checked_operation_roots(
    operation: &PathOperation,
    repository: &Repository,
) -> Result<Vec<PathBuf>, String> {
    let mut roots = BTreeSet::new();
    let (paths, create_parent_for): (Vec<PathBuf>, Vec<PathBuf>) = match operation {
        PathOperation::Write { path, .. } => {
            let path = PathBuf::from(path);
            (vec![path.clone()], vec![path])
        }
        PathOperation::Move { from, to } => {
            let from = PathBuf::from(from);
            let to = PathBuf::from(to);
            (vec![from, to.clone()], vec![to])
        }
    };
    for path in &paths {
        validate_path_chain(repository.root(), path)?;
        roots.insert(path.clone());
    }
    for path in create_parent_for {
        let mut parent = path.parent();
        while let Some(relative) = parent.filter(|value| !value.as_os_str().is_empty()) {
            validate_relative_path(relative)?;
            let absolute = repository.root().join(relative);
            match fs::symlink_metadata(&absolute) {
                Ok(metadata) if is_symlink_or_reparse(&metadata) => {
                    return Err(format!(
                        "parent '{}' is a symlink or reparse point",
                        path_string(relative)
                    ));
                }
                Ok(metadata) if metadata.file_type().is_dir() => break,
                Ok(_) => {
                    return Err(format!(
                        "parent '{}' is not a regular directory",
                        path_string(relative)
                    ));
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    roots.insert(relative.to_path_buf());
                }
                Err(error) => {
                    return Err(format!(
                        "cannot inspect parent '{}': {error}",
                        path_string(relative)
                    ));
                }
            }
            parent = relative.parent();
        }
    }
    Ok(prune_roots(roots.into_iter().collect()))
}

fn prune_roots(mut roots: Vec<PathBuf>) -> Vec<PathBuf> {
    roots.sort_by_key(|path| (path.components().count(), path.clone()));
    let mut retained = Vec::<PathBuf>::new();
    for path in roots {
        if !retained.iter().any(|parent| path.starts_with(parent)) {
            retained.push(path);
        }
    }
    retained
}

fn validate_relative_path(path: &Path) -> Result<(), String> {
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err(format!(
            "'{}' is not a repository-relative path",
            path_string(path)
        ));
    }
    if path
        .components()
        .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(format!(
            "'{}' contains an ambiguous or escaping path component",
            path_string(path)
        ));
    }
    Ok(())
}

fn validate_path_chain(root: &Path, relative: &Path) -> Result<(), String> {
    validate_relative_path(relative)?;
    let components = relative.components().collect::<Vec<_>>();
    let mut current = root.to_path_buf();
    for (index, component) in components.iter().enumerate() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                let kind = metadata.file_type();
                if is_symlink_or_reparse(&metadata) {
                    return Err(format!(
                        "'{}' contains a symlink or reparse point",
                        path_string(&current)
                    ));
                }
                let is_last = index + 1 == components.len();
                if (!is_last && !kind.is_dir()) || (is_last && !kind.is_dir() && !kind.is_file()) {
                    return Err(format!(
                        "'{}' has an unsupported path type",
                        path_string(&current)
                    ));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "cannot inspect '{}': {error}",
                    path_string(&current)
                ));
            }
        }
    }
    Ok(())
}

fn is_symlink_or_reparse(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
        return metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
    }
    #[cfg(not(windows))]
    {
        false
    }
}

fn ensure_single_link(path: &Path, metadata: &fs::Metadata) -> Result<(), String> {
    #[cfg(unix)]
    let link_count = {
        use std::os::unix::fs::MetadataExt;
        metadata.nlink()
    };
    #[cfg(windows)]
    let link_count = {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{
            GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_DIRECTORY,
            FILE_ATTRIBUTE_REPARSE_POINT,
        };
        let _ = metadata;
        let file = File::open(path).map_err(|error| {
            format!(
                "cannot inspect file links for '{}': {error}",
                path_string(path)
            )
        })?;
        let mut information = BY_HANDLE_FILE_INFORMATION::default();
        let succeeded =
            unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut information) };
        if succeeded == 0 {
            return Err(format!(
                "cannot obtain file link information for '{}': {}",
                path_string(path),
                std::io::Error::last_os_error()
            ));
        }
        if information.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0
            || information.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
        {
            return Err(format!(
                "'{}' changed type while its link count was inspected",
                path_string(path)
            ));
        }
        information.nNumberOfLinks as u64
    };
    #[cfg(not(any(unix, windows)))]
    let link_count = {
        let _ = (path, metadata);
        return Err("regular-file link count is unsupported on this platform".to_owned());
    };

    if link_count != 1 {
        return Err(format!(
            "'{}' has {link_count} hard links; refusing an external alias",
            path_string(path)
        ));
    }
    Ok(())
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
    current: &Preimages,
    expected: &BTreeMap<PathBuf, PathState>,
) -> Option<String> {
    operation_paths(operation)
        .into_iter()
        .find(|path| current.state_for(repository, path) != expected.get(path).cloned())
        .map(|path| path_string(&path))
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PreimageEntry {
    File(Vec<u8>),
    Directory,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Preimages {
    roots: BTreeSet<PathBuf>,
    entries: BTreeMap<PathBuf, PreimageEntry>,
}

impl Preimages {
    fn covers(&self, path: &Path) -> bool {
        self.roots.iter().any(|root| path.starts_with(root))
    }

    fn state_for(&self, repository: &Repository, path: &Path) -> Option<PathState> {
        self.state_for_root(repository.root(), path)
    }

    fn state_for_root(&self, repository_root: &Path, path: &Path) -> Option<PathState> {
        if !self.covers(path) {
            return None;
        }
        match self.entries.get(path) {
            Some(PreimageEntry::File(bytes)) => Some(PathState::Present {
                digest: hex_digest(Sha256::digest(bytes)),
                kind: "file".to_owned(),
            }),
            Some(PreimageEntry::Directory) => {
                let mut files = self
                    .entries
                    .iter()
                    .filter_map(|(candidate, entry)| match entry {
                        PreimageEntry::File(bytes) if candidate.starts_with(path) => {
                            Some((path_string(&repository_root.join(candidate)), bytes))
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                files.sort_by(|left, right| left.0.cmp(&right.0));
                let mut hasher = Sha256::new();
                for (name, bytes) in files {
                    hasher.update(name.as_bytes());
                    hasher.update(Sha256::digest(bytes));
                }
                Some(PathState::Present {
                    digest: hex_digest(hasher.finalize()),
                    kind: "directory".to_owned(),
                })
            }
            None => Some(PathState::Absent),
        }
    }
}

#[derive(Debug, Clone)]
struct UndoRecord {
    operation: PathOperation,
    before: Preimages,
    after: Preimages,
}

fn capture_preimages(repository: &Repository, roots: &[PathBuf]) -> Result<Preimages, String> {
    let mut read_file = |path: &Path| fs::read(path);
    let mut read_directory = |path: &Path| fs::read_dir(path);
    capture_preimages_with(repository, roots, &mut read_file, &mut read_directory)
}

fn capture_preimages_with(
    repository: &Repository,
    roots: &[PathBuf],
    read_file: &mut dyn FnMut(&Path) -> std::io::Result<Vec<u8>>,
    read_directory: &mut dyn FnMut(&Path) -> std::io::Result<fs::ReadDir>,
) -> Result<Preimages, String> {
    let roots = prune_roots(roots.to_vec());
    let before =
        capture_preimages_once(repository.root(), &roots, read_file, read_directory, true)?;
    let preimages =
        capture_preimages_once(repository.root(), &roots, read_file, read_directory, true)?;
    let after = capture_preimages_once(repository.root(), &roots, read_file, read_directory, true)?;
    if before != preimages || preimages != after {
        return Err("target state changed while its complete preimage was captured".to_owned());
    }
    Ok(preimages)
}

fn capture_preimages_once(
    repository_root: &Path,
    roots: &[PathBuf],
    read_file: &mut dyn FnMut(&Path) -> std::io::Result<Vec<u8>>,
    read_directory: &mut dyn FnMut(&Path) -> std::io::Result<fs::ReadDir>,
    check_link_count: bool,
) -> Result<Preimages, String> {
    let roots = prune_roots(roots.to_vec());
    let mut preimages = Preimages {
        roots: roots.iter().cloned().collect(),
        entries: BTreeMap::new(),
    };
    for path in &roots {
        validate_path_chain(repository_root, path)?;
        capture_preimage_path(
            repository_root,
            path,
            &mut preimages,
            read_file,
            read_directory,
            check_link_count,
        )?;
    }
    Ok(preimages)
}

fn observe_path_state(
    repository_root: &Path,
    relative: &Path,
    read_file: &mut dyn FnMut(&Path) -> std::io::Result<Vec<u8>>,
    read_directory: &mut dyn FnMut(&Path) -> std::io::Result<fs::ReadDir>,
    check_link_count: bool,
) -> Result<PathState, String> {
    let observation = capture_preimages_once(
        repository_root,
        &[relative.to_path_buf()],
        read_file,
        read_directory,
        check_link_count,
    )?;
    observation
        .state_for_root(repository_root, relative)
        .ok_or_else(|| format!("cannot observe path state for '{}'", path_string(relative)))
}

fn capture_preimage_path(
    repository_root: &Path,
    relative: &Path,
    preimages: &mut Preimages,
    read_file: &mut dyn FnMut(&Path) -> std::io::Result<Vec<u8>>,
    read_directory: &mut dyn FnMut(&Path) -> std::io::Result<fs::ReadDir>,
    check_link_count: bool,
) -> Result<Option<u8>, String> {
    validate_relative_path(relative)?;
    let absolute = repository_root.join(relative);
    let metadata = match fs::symlink_metadata(&absolute) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "cannot inspect '{}': {error}",
                path_string(&absolute)
            ));
        }
    };
    let kind = metadata.file_type();
    if is_symlink_or_reparse(&metadata) {
        return Err(format!(
            "'{}' is a symlink or reparse point",
            path_string(&absolute)
        ));
    }
    if kind.is_file() {
        if check_link_count {
            ensure_single_link(&absolute, &metadata)?;
        }
        let first = read_file(&absolute).map_err(|error| {
            format!("cannot read preimage '{}': {error}", path_string(&absolute))
        })?;
        let middle_metadata = fs::symlink_metadata(&absolute).map_err(|error| {
            format!(
                "cannot recheck preimage '{}': {error}",
                path_string(&absolute)
            )
        })?;
        if !middle_metadata.file_type().is_file() || is_symlink_or_reparse(&middle_metadata) {
            return Err(format!(
                "preimage '{}' changed type while it was read",
                path_string(&absolute)
            ));
        }
        if check_link_count {
            ensure_single_link(&absolute, &middle_metadata)?;
        }
        let second = read_file(&absolute).map_err(|error| {
            format!(
                "cannot verify preimage '{}': {error}",
                path_string(&absolute)
            )
        })?;
        let final_metadata = fs::symlink_metadata(&absolute).map_err(|error| {
            format!(
                "cannot recheck preimage '{}': {error}",
                path_string(&absolute)
            )
        })?;
        if first != second
            || first.len() as u64 != metadata.len()
            || second.len() as u64 != middle_metadata.len()
            || final_metadata.len() != middle_metadata.len()
            || !final_metadata.file_type().is_file()
            || is_symlink_or_reparse(&final_metadata)
        {
            return Err(format!(
                "preimage '{}' changed while it was read",
                path_string(&absolute)
            ));
        }
        if check_link_count {
            ensure_single_link(&absolute, &final_metadata)?;
        }
        insert_preimage(
            preimages,
            relative.to_path_buf(),
            PreimageEntry::File(first),
        )?;
        return Ok(Some(2));
    }
    if !kind.is_dir() {
        return Err(format!(
            "'{}' has an unsupported filesystem type",
            path_string(&absolute)
        ));
    }
    insert_preimage(preimages, relative.to_path_buf(), PreimageEntry::Directory)?;
    let before = directory_signature(&absolute, read_directory)?;
    for (name, expected_kind) in &before {
        let child = relative.join(name);
        let Some(actual_kind) = capture_preimage_path(
            repository_root,
            &child,
            preimages,
            read_file,
            read_directory,
            check_link_count,
        )?
        else {
            return Err(format!(
                "entry '{}' disappeared during preimage capture",
                path_string(&repository_root.join(&child))
            ));
        };
        if actual_kind != *expected_kind {
            return Err(format!(
                "entry '{}' changed type during preimage capture",
                path_string(&repository_root.join(&child))
            ));
        }
    }
    let after = directory_signature(&absolute, read_directory)?;
    if before != after {
        return Err(format!(
            "directory '{}' changed during preimage capture",
            path_string(&absolute)
        ));
    }
    Ok(Some(1))
}

fn directory_signature(
    path: &Path,
    read_directory: &mut dyn FnMut(&Path) -> std::io::Result<fs::ReadDir>,
) -> Result<Vec<(std::ffi::OsString, u8)>, String> {
    let entries = read_directory(path)
        .map_err(|error| format!("cannot enumerate '{}': {error}", path_string(path)))?;
    let mut signature = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "cannot read directory entry in '{}': {error}",
                path_string(path)
            )
        })?;
        let entry_path = entry.path();
        let metadata = fs::symlink_metadata(&entry_path).map_err(|error| {
            format!(
                "cannot inspect directory entry '{}': {error}",
                path_string(&entry_path)
            )
        })?;
        if is_symlink_or_reparse(&metadata) {
            return Err(format!(
                "'{}' is a symlink or reparse point",
                path_string(&entry_path)
            ));
        }
        let kind = metadata.file_type();
        let kind = if kind.is_dir() {
            1
        } else if kind.is_file() {
            2
        } else {
            return Err(format!(
                "'{}' has an unsupported filesystem type",
                path_string(&entry_path)
            ));
        };
        signature.push((entry.file_name(), kind));
    }
    signature.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(signature)
}

fn insert_preimage(
    preimages: &mut Preimages,
    path: PathBuf,
    entry: PreimageEntry,
) -> Result<(), String> {
    if let Some(existing) = preimages.entries.insert(path.clone(), entry.clone()) {
        if existing != entry {
            return Err(format!(
                "conflicting preimage observations for '{}'",
                path_string(&path)
            ));
        }
    }
    Ok(())
}

fn project_operation(
    repository: &Repository,
    before: &Preimages,
    operation: &PathOperation,
) -> Result<Preimages, String> {
    let mut after = before.clone();
    match operation {
        PathOperation::Write { path, content } => {
            let path = PathBuf::from(path);
            match before.state_for(repository, &path) {
                Some(PathState::Absent) => {}
                Some(PathState::Present { kind, .. }) if kind == "file" => {}
                Some(PathState::Present { kind, .. }) => {
                    return Err(format!("write target '{}' is a {kind}", path_string(&path)));
                }
                None => {
                    return Err(format!(
                        "write target '{}' was not captured",
                        path_string(&path)
                    ))
                }
            }
            add_created_parent_directories(repository, before, &mut after, &path);
            after
                .entries
                .insert(path, PreimageEntry::File(content.as_bytes().to_vec()));
        }
        PathOperation::Move { from, to } => {
            let from = PathBuf::from(from);
            let to = PathBuf::from(to);
            if from == to || from.starts_with(&to) || to.starts_with(&from) {
                return Err("move source and destination overlap".to_owned());
            }
            if !matches!(
                before.state_for(repository, &from),
                Some(PathState::Present { .. })
            ) {
                return Err(format!("move source '{}' is absent", path_string(&from)));
            }
            if before.state_for(repository, &to) != Some(PathState::Absent) {
                return Err(format!(
                    "move destination '{}' already exists",
                    path_string(&to)
                ));
            }
            add_created_parent_directories(repository, before, &mut after, &to);
            let moved = before
                .entries
                .iter()
                .filter(|(path, _)| path.starts_with(&from))
                .map(|(path, entry)| {
                    let suffix = path.strip_prefix(&from).expect("prefix was checked");
                    (to.join(suffix), entry.clone())
                })
                .collect::<Vec<_>>();
            if moved.is_empty() {
                return Err(format!(
                    "move source '{}' has no captured entries",
                    path_string(&from)
                ));
            }
            after
                .entries
                .retain(|path, _| !path.starts_with(&from) && !path.starts_with(&to));
            for (path, entry) in moved {
                after.entries.insert(path, entry);
            }
        }
    }
    Ok(after)
}

fn add_created_parent_directories(
    repository: &Repository,
    before: &Preimages,
    after: &mut Preimages,
    target: &Path,
) {
    let mut parents = target
        .parent()
        .into_iter()
        .flat_map(Path::ancestors)
        .filter(|path| !path.as_os_str().is_empty())
        .collect::<Vec<_>>();
    parents.reverse();
    for parent in parents {
        if before.state_for(repository, parent) == Some(PathState::Absent) {
            after
                .entries
                .insert(parent.to_path_buf(), PreimageEntry::Directory);
        }
    }
}

fn restore_operation(repository: &Repository, record: &UndoRecord) -> Result<(), String> {
    let current = capture_preimages(
        repository,
        &record.after.roots.iter().cloned().collect::<Vec<_>>(),
    )?;
    if current.entries != record.after.entries {
        return Err(format!(
            "operation after-state for '{}' no longer matches its owned result",
            operation_paths(&record.operation)
                .first()
                .map(|path| path_string(path))
                .unwrap_or_else(|| "unknown path".to_owned())
        ));
    }
    match &record.operation {
        PathOperation::Write { path, .. } => {
            let path = PathBuf::from(path);
            let absolute = repository.root().join(&path);
            match record.before.entries.get(&path) {
                Some(PreimageEntry::File(bytes)) => {
                    fs::write(&absolute, bytes).map_err(|error| {
                        format!("cannot restore file '{}': {error}", path_string(&absolute))
                    })?;
                }
                Some(PreimageEntry::Directory) => {
                    return Err(format!(
                        "write target '{}' was a directory",
                        path_string(&path)
                    ));
                }
                None if record.before.state_for(repository, &path) == Some(PathState::Absent) => {
                    fs::remove_file(&absolute).map_err(|error| {
                        format!(
                            "cannot remove transaction-created file '{}': {error}",
                            path_string(&absolute)
                        )
                    })?;
                }
                None => {
                    return Err(format!(
                        "preimage for '{}' is incomplete",
                        path_string(&path)
                    ))
                }
            }
        }
        PathOperation::Move { from, to } => {
            let from = repository.root().join(from);
            let to = repository.root().join(to);
            if fs::symlink_metadata(&from).is_ok() {
                return Err(format!(
                    "move source '{}' is occupied during rollback",
                    path_string(&from)
                ));
            }
            fs::rename(&to, &from)
                .map_err(|error| format!("cannot reverse move '{}': {error}", path_string(&to)))?;
        }
    }
    remove_created_empty_directories(repository, &record.before, &record.after)?;
    let restored = capture_preimages(
        repository,
        &record.before.roots.iter().cloned().collect::<Vec<_>>(),
    )?;
    if restored != record.before {
        return Err(format!(
            "restored preimage for '{}' does not exactly match the captured before-state",
            operation_paths(&record.operation)
                .first()
                .map(|path| path_string(path))
                .unwrap_or_else(|| "unknown path".to_owned())
        ));
    }
    Ok(())
}

fn remove_created_empty_directories(
    repository: &Repository,
    before: &Preimages,
    after: &Preimages,
) -> Result<(), String> {
    let mut created = after
        .entries
        .iter()
        .filter_map(|(path, entry)| {
            if *entry == PreimageEntry::Directory
                && before.state_for(repository, path) == Some(PathState::Absent)
            {
                Some(path.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    created.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    for relative in created {
        let absolute = repository.root().join(&relative);
        match fs::symlink_metadata(&absolute) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(format!(
                    "cannot inspect created directory '{}': {error}",
                    path_string(&absolute)
                ));
            }
            Ok(metadata) if is_symlink_or_reparse(&metadata) || !metadata.file_type().is_dir() => {
                return Err(format!(
                    "created directory '{}' changed type during rollback",
                    path_string(&absolute)
                ));
            }
            Ok(_) => fs::remove_dir(&absolute).map_err(|error| {
                format!(
                    "created directory '{}' is no longer empty or cannot be removed: {error}",
                    path_string(&absolute)
                )
            })?,
        }
    }
    Ok(())
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
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    #[cfg(windows)]
    use std::os::windows::fs::symlink_dir;
    use std::path::PathBuf;
    use std::process::{Command, Stdio};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::mpsc;
    use std::time::Duration;

    static TEST_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn candidate_copy_includes_only_governed_semantic_build_inputs() {
        let root = create_fixture_root_under(
            &std::env::temp_dir(),
            "repopact-build-input-copy",
            &TEST_ROOT_COUNTER,
        )
        .expect("create a fresh, exclusive candidate-copy test root");
        let destination = root.join("candidate");
        let source = root.join("source");
        let build = source.join("build");
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

    const FIXTURE_ROOT_COLLISION_LIMIT: usize = 64;

    fn create_fixture_root_under(
        parent: &Path,
        prefix: &str,
        counter: &AtomicU64,
    ) -> std::io::Result<PathBuf> {
        let mut last_collision = None;
        for _ in 0..FIXTURE_ROOT_COLLISION_LIMIT {
            let sequence = counter.fetch_add(1, Ordering::Relaxed);
            let candidate = parent.join(format!("{prefix}-{}-{sequence}", std::process::id()));
            match fs::create_dir(&candidate) {
                Ok(()) => return Ok(candidate),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    last_collision = Some(candidate);
                }
                Err(error) => {
                    return Err(std::io::Error::new(
                        error.kind(),
                        format!(
                            "cannot exclusively create fixture directory {candidate:?}: {error}"
                        ),
                    ));
                }
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!(
                "fixture directory allocation exhausted {FIXTURE_ROOT_COLLISION_LIMIT} collision attempts; last existing path: {last_collision:?}"
            ),
        ))
    }

    fn fixture() -> (PathBuf, RepositorySnapshot) {
        let root = create_fixture_root_under(
            &std::env::temp_dir(),
            "repopact-mutation-test",
            &TEST_ROOT_COUNTER,
        )
        .expect("create a fresh, exclusive mutation test fixture root");
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

    #[test]
    fn candidate_temp_allocation_is_exclusive_and_preserves_collisions() {
        let scratch_counter = AtomicU64::new(0);
        let scratch = create_fixture_root_under(
            &std::env::temp_dir(),
            "repopact-candidate-temp-regression",
            &scratch_counter,
        )
        .unwrap();
        let sequence = AtomicU64::new(0);
        let stamp = 17_u128;
        let collision = scratch.join(format!(
            "repopact-mutation-plan-{}-{stamp}-0",
            std::process::id()
        ));
        fs::create_dir(&collision).unwrap();
        let marker = collision.join("preexisting-marker");
        fs::write(&marker, b"must not be deleted or reused").unwrap();

        let first = create_candidate_temp_under(&scratch, stamp, &sequence).unwrap();
        let second = create_candidate_temp_under(&scratch, stamp, &sequence).unwrap();
        assert_ne!(first, second, "concurrent same-tick candidates must differ");
        assert_ne!(first, collision, "existing directory cannot be reused");
        assert_ne!(second, collision, "existing directory cannot be reused");
        assert!(marker.is_file(), "collision contents must be untouched");
        assert_eq!(fs::read_dir(&first).unwrap().count(), 0);
        assert_eq!(fs::read_dir(&second).unwrap().count(), 0);

        let start = sequence.load(Ordering::Relaxed);
        for candidate in start..start + CANDIDATE_TEMP_COLLISION_LIMIT as u64 {
            fs::create_dir(scratch.join(format!(
                "repopact-mutation-plan-{}-{stamp}-{candidate}",
                std::process::id()
            )))
            .unwrap();
        }
        let exhausted = create_candidate_temp_under(&scratch, stamp, &sequence).unwrap_err();
        assert!(exhausted.contains("exhausted 64 attempts"), "{exhausted}");
        assert!(
            marker.is_file(),
            "allocation exhaustion cannot destroy prior evidence"
        );
        fs::remove_dir_all(&scratch).unwrap();
    }

    #[test]
    fn fixture_root_collision_skips_existing_nonempty_directory() {
        let counter = AtomicU64::new(0);
        let scratch = create_fixture_root_under(
            &std::env::temp_dir(),
            "repopact-mutation-collision-test",
            &counter,
        )
        .unwrap();
        let first_fixture_sequence = counter.load(Ordering::Relaxed);
        let collision = scratch.join(format!(
            "repopact-mutation-test-{}-{first_fixture_sequence}",
            std::process::id()
        ));
        fs::create_dir(&collision).unwrap();
        let marker = collision.join("preexisting-collision-marker.txt");
        fs::write(&marker, "owned by fixture collision regression\n").unwrap();

        let root = create_fixture_root_under(&scratch, "repopact-mutation-test", &counter).unwrap();
        let marker_preserved = marker.is_file();
        let allocated_root_is_empty = fs::read_dir(&root).unwrap().next().is_none();
        let root_is_distinct = root != collision;
        let exhaustion_start = counter.load(Ordering::Relaxed);
        for sequence in exhaustion_start..exhaustion_start + FIXTURE_ROOT_COLLISION_LIMIT as u64 {
            fs::create_dir(scratch.join(format!(
                "repopact-mutation-test-{}-{sequence}",
                std::process::id()
            )))
            .unwrap();
        }
        let exhaustion =
            create_fixture_root_under(&scratch, "repopact-mutation-test", &counter).unwrap_err();
        let attempts = counter.load(Ordering::Relaxed) - exhaustion_start;
        let exhausted_as_collision = exhaustion.kind() == std::io::ErrorKind::AlreadyExists;
        let exhaustion_diagnostic = exhaustion.to_string().contains("64 collision attempts");
        fs::remove_dir_all(&scratch).unwrap();

        assert!(
            root_is_distinct,
            "must never reuse an existing fixture path"
        );
        assert!(marker_preserved, "must preserve unknown collision contents");
        assert!(allocated_root_is_empty, "fresh fixture root must be empty");
        assert_eq!(attempts, FIXTURE_ROOT_COLLISION_LIMIT as u64);
        assert!(
            exhausted_as_collision,
            "exhaustion must return AlreadyExists"
        );
        assert!(exhaustion_diagnostic, "exhaustion must report its bound");
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

    fn terminate_mutation_child_at(root: &Path, key: &str, phase: &str, fail_after: Option<&str>) {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .arg("--exact")
            .arg("tests::mutation_process_termination_child_entry")
            .arg("--nocapture")
            .env("REPOPACT_MUTATION_PAUSE_AT", phase)
            .env("REPOPACT_MUTATION_CHILD_ROOT", root)
            .env("REPOPACT_MUTATION_CHILD_KEY", key)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        if let Some(index) = fail_after {
            command.env("REPOPACT_MUTATION_CHILD_FAIL_AFTER", index);
        }
        let mut child = command.spawn().expect("spawn isolated mutation test child");
        let stdout = child.stdout.take().unwrap();
        let (line_tx, line_rx) = mpsc::channel();
        std::thread::spawn(move || {
            use std::io::BufRead;
            for line in std::io::BufReader::new(stdout).lines() {
                if line_tx.send(line).is_err() {
                    break;
                }
            }
        });
        let expected_signal = format!("REPOPACT_MUTATION_PHASE:{phase}");
        let mut observed = false;
        let mut output = Vec::new();
        while let Ok(line) = line_rx.recv_timeout(Duration::from_secs(20)) {
            let line = line.expect("read child test output");
            if line == expected_signal {
                observed = true;
                break;
            }
            output.push(line);
        }
        if !observed {
            let _ = child.kill();
            let _ = child.wait();
            panic!("child did not reach synchronized phase {phase}; output: {output:?}");
        }
        child
            .kill()
            .expect("terminate only the synchronized child process");
        let exit = child.wait().expect("wait for terminated child process");
        assert!(
            !exit.success(),
            "synchronized child termination must interrupt the child"
        );
    }

    #[test]
    fn mutation_process_termination_child_entry() {
        let Ok(root) = std::env::var("REPOPACT_MUTATION_CHILD_ROOT") else {
            return;
        };
        let root = PathBuf::from(root);
        let key = std::env::var("REPOPACT_MUTATION_CHILD_KEY").unwrap();
        let snapshot = RepositorySession::open(&root).snapshot();
        let plan = plan(
            &snapshot,
            MutationRequest::create_work_item(create_request(
                "Process Recovery Probe",
                "2026-11-27",
            )),
        )
        .with_mutation_key(key);
        let options = match std::env::var("REPOPACT_MUTATION_CHILD_FAIL_AFTER") {
            Ok(value) => ApplyOptions {
                fail_after_operation: Some(value.parse().expect("numeric child failure index")),
                ..ApplyOptions::default()
            },
            Err(_) => ApplyOptions::default(),
        };
        let _result = plan.apply_with_options(&options);
    }

    #[test]
    fn process_termination_recovery_uses_synchronized_child_kills() {
        let cases = [
            ("before_prepare", None),
            ("after_prepare_before_write", None),
            ("between_writes", None),
            ("after_last_write_before_validation", None),
            ("after_validation_before_terminal_receipt", None),
            ("after_terminal_receipt_before_journal_cleanup", None),
            ("after_terminal_receipt_before_response", None),
            ("during_rollback", Some("0")),
        ];
        for (phase, fail_after) in cases {
            let (root, snapshot) = fixture();
            let key = format!("recovery-{phase}-{}", std::process::id());
            let plan = plan(
                &snapshot,
                MutationRequest::create_work_item(create_request(
                    "Process Recovery Probe",
                    "2026-11-27",
                )),
            )
            .with_mutation_key(key.clone());
            let repository = Repository::open(&root);
            let roots = operation_roots(&plan, &repository).unwrap();
            let before = capture_preimages(&repository, &roots).unwrap();
            assert!(
                plan.file_operations.len() >= 2,
                "the synchronized between-write case requires two operations"
            );

            terminate_mutation_child_at(&root, &key, phase, fail_after);

            let status = mutation_status(&root, Some(&key));
            if phase == "before_prepare" {
                assert_eq!(status.state, "unknown", "pre-prepare absence is unknown");
                assert_eq!(capture_preimages(&repository, &roots).unwrap(), before);
                let committed = plan.apply();
                assert!(
                    committed.success,
                    "known pre-prepare retry should commit: {:?}",
                    committed.diagnostics
                );
                let replay = plan.apply();
                assert!(replay.replayed, "the same durable key replays its receipt");
                assert!(replay.success);
            } else if phase == "after_terminal_receipt_before_journal_cleanup"
                || phase == "after_terminal_receipt_before_response"
            {
                assert_eq!(
                    status.state,
                    if phase == "after_terminal_receipt_before_journal_cleanup" {
                        "terminal"
                    } else {
                        "clear"
                    }
                );
                assert!(status
                    .receipts
                    .iter()
                    .any(|receipt| receipt.request_key == key));
                let replay = recover_mutation(&root, Some(&key));
                assert!(replay.success);
                assert!(replay.replayed);
                assert_eq!(replay.transaction_outcome, "replayed");
            } else {
                assert_eq!(
                    status.state, "pending",
                    "phase {phase} must leave a recoverable journal; status={status:?}"
                );
                assert_eq!(status.active_request_key.as_deref(), Some(key.as_str()));
                let recovered = recover_mutation(&root, Some(&key));
                assert_eq!(
                    recovered.transaction_outcome, "rolled_back",
                    "phase {phase}: {:?}",
                    recovered.diagnostics
                );
                assert!(!recovered.success);
                assert!(recovered.rolled_back);
                assert_eq!(capture_preimages(&repository, &roots).unwrap(), before);
                let replay = recover_mutation(&root, Some(&key));
                assert!(replay.replayed, "recovery receipt must be idempotent");
                assert_eq!(replay.transaction_outcome, "replayed");
            }
            let _ = fs::remove_dir_all(root);
        }
    }

    #[test]
    fn durable_mutation_key_replays_once_and_rejects_changed_intent() {
        let (root, snapshot) = fixture();
        let key = "same-request-key";
        let first = plan(
            &snapshot,
            MutationRequest::create_work_item(create_request("First Durable Item", "2026-11-27")),
        )
        .with_mutation_key(key);
        let committed = first.apply();
        assert!(committed.success, "{:?}", committed.diagnostics);
        assert_eq!(committed.transaction_outcome, "committed");
        let replay = first.apply();
        assert!(replay.success);
        assert!(replay.replayed);
        assert_eq!(replay.transaction_outcome, "replayed");
        assert_eq!(replay.changed_paths, committed.changed_paths);

        let changed = plan(
            &RepositorySession::open(&root).snapshot(),
            MutationRequest::create_work_item(create_request("Different Intent", "2026-11-27")),
        )
        .with_mutation_key(key);
        let conflict = changed.apply();
        assert!(!conflict.success);
        assert_eq!(conflict.transaction_outcome, "conflict");
        assert!(conflict
            .diagnostics
            .iter()
            .any(|item| item.code == "mutation.request-key-conflict"));
        assert_eq!(fs::read_dir(root.join("work/active")).unwrap().count(), 2);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn recovery_preserves_external_edits_and_leaves_the_journal_pending() {
        let (root, snapshot) = fixture();
        let key = format!("external-edit-{}", std::process::id());
        let plan = plan(
            &snapshot,
            MutationRequest::create_work_item(create_request(
                "Process Recovery Probe",
                "2026-11-27",
            )),
        )
        .with_mutation_key(key.clone());
        let operation_path = operation_paths(&plan.file_operations[0])[0].clone();
        terminate_mutation_child_at(&root, &key, "between_writes", None);

        let absolute = root.join(&operation_path);
        let external = b"external user edit preserved after interrupted mutation\n";
        fs::write(&absolute, external).unwrap();
        let journal_path = root.join("work/.repopact-transactions/ordinary-v1/journal.json");
        let before_status = fs::read(&journal_path).unwrap();
        assert_eq!(mutation_status(&root, Some(&key)).state, "pending");
        assert_eq!(
            fs::read(&journal_path).unwrap(),
            before_status,
            "status must be read-only"
        );
        let recovered = recover_mutation(&root, Some(&key));
        assert_eq!(recovered.transaction_outcome, "recovery_required");
        assert_eq!(fs::read(&absolute).unwrap(), external);
        assert!(
            journal_path.is_file(),
            "ambiguous journal remains for inspection"
        );
        assert!(root.join("work/active/001-one/extra.md").is_file());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn copied_and_corrupt_journals_are_reported_without_rewriting_them() {
        let (root, _) = fixture();
        let key = format!("copied-root-{}", std::process::id());
        terminate_mutation_child_at(&root, &key, "after_prepare_before_write", None);
        let source_journal = root.join("work/.repopact-transactions/ordinary-v1/journal.json");
        let original = fs::read(&source_journal).unwrap();

        let clone = create_fixture_root_under(
            &std::env::temp_dir(),
            "repopact-mutation-cloned-root",
            &TEST_ROOT_COUNTER,
        )
        .unwrap();
        copy_tree(&root, &clone).unwrap();
        let clone_store = transaction_store::OrdinaryTransactionStore::open(&clone).unwrap();
        let copied_journal = clone_store.anchor().join("journal.json");
        fs::write(&copied_journal, &original).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&copied_journal, fs::Permissions::from_mode(0o600)).unwrap();
        }
        let copied_status = mutation_status(&clone, Some(&key));
        assert_eq!(
            copied_status.state, "recovery_required",
            "copied root status: {copied_status:?}"
        );
        assert!(
            copied_status
                .detail
                .as_deref()
                .unwrap_or_default()
                .contains("identity"),
            "copied root detail: {copied_status:?}"
        );
        assert_eq!(fs::read(&copied_journal).unwrap(), original);

        let corrupt = b"{corrupt journal evidence; preserve exactly";
        fs::write(&source_journal, corrupt).unwrap();
        let corrupt_status = mutation_status(&root, Some(&key));
        assert_eq!(corrupt_status.state, "recovery_required");
        assert_eq!(fs::read(&source_journal).unwrap(), corrupt);
        let recovery = recover_mutation(&root, Some(&key));
        assert_eq!(recovery.transaction_outcome, "recovery_required");
        assert_eq!(fs::read(&source_journal).unwrap(), corrupt);
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(clone);
    }

    #[test]
    fn transaction_journal_hard_link_is_rejected_without_touching_its_external_name() {
        let (root, _) = fixture();
        let store = transaction_store::OrdinaryTransactionStore::open(&root).unwrap();
        let external = root.join("external-user-data.txt");
        let bytes = b"user data linked into transaction storage must survive";
        fs::write(&external, bytes).unwrap();
        let journal = store.anchor().join("journal.json");
        fs::hard_link(&external, &journal).unwrap();

        let status = mutation_status(&root, Some("link-probe"));
        assert_eq!(status.state, "recovery_required");
        assert_eq!(fs::read(&external).unwrap(), bytes);
        assert_eq!(fs::read(&journal).unwrap(), bytes);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn receipt_capacity_failure_precedes_any_governed_write() {
        let (root, snapshot) = fixture();
        let repository = Repository::open(&root);
        let store = transaction_store::OrdinaryTransactionStore::open(&root).unwrap();
        let native_root = NativeRootIdentity::capture(&root).unwrap();
        let storage_identity = NativeRootIdentity::capture(store.anchor()).unwrap();
        let capacity_token = "00".repeat(32);
        for index in 0..transaction_store::MAX_RECEIPTS {
            let key = format!("capacity-{index}");
            let receipt = OrdinaryReceipt {
                version: transaction_store::STORE_VERSION,
                request_key: key.clone(),
                payload_sha256: capacity_token.clone(),
                repository_identity: repository.identity().clone(),
                native_root: native_root.clone(),
                storage_identity: storage_identity.clone(),
                plan_token: capacity_token.clone(),
                result: MutationResult {
                    success: false,
                    rolled_back: false,
                    mutation_key: Some(key.clone()),
                    transaction_outcome: "rejected".to_owned(),
                    plan_token: capacity_token.clone(),
                    changed_paths: Vec::new(),
                    diagnostics: Vec::new(),
                    replayed: false,
                    metrics: MutationMetrics::default(),
                },
            };
            let payload = serde_json::to_vec(&receipt).unwrap();
            let envelope = transaction_store::Envelope {
                payload: receipt,
                sha256: transaction_store::digest_hex(Sha256::digest(&payload)),
            };
            let name = format!(
                "{}.json",
                transaction_store::digest_hex(Sha256::digest(key.as_bytes()))
            );
            let path = store.anchor().join("receipts").join(name);
            fs::write(&path, serde_json::to_vec(&envelope).unwrap()).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
            }
        }
        assert_eq!(
            store.receipts().unwrap().len(),
            transaction_store::MAX_RECEIPTS
        );
        let key = "capacity-overflow";
        let pending = plan(
            &snapshot,
            MutationRequest::create_work_item(create_request("Must Not Be Written", "2026-11-27")),
        )
        .with_mutation_key(key);
        let repository = Repository::open(&root);
        let roots = operation_roots(&pending, &repository).unwrap();
        let before = capture_preimages(&repository, &roots).unwrap();
        let result = pending.apply();
        assert!(!result.success);
        assert_eq!(
            result.transaction_outcome, "rejected",
            "diagnostics={:?}",
            result.diagnostics
        );
        assert!(result
            .diagnostics
            .iter()
            .any(|item| item.code == "mutation.receipt-capacity"));
        assert_eq!(capture_preimages(&repository, &roots).unwrap(), before);
        assert_eq!(
            store.receipt_count().unwrap(),
            transaction_store::MAX_RECEIPTS
        );
        let _ = fs::remove_dir_all(root);
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
    fn injected_failure_after_file_write_restores_exact_preimage() {
        let (root, snapshot) = fixture();
        let path = root.join("work/active/001-one/work-item.json");
        let before = fs::read(&path).unwrap();
        let plan = plan(
            &snapshot,
            MutationRequest::edit_work_item(EditWorkItem::new(
                "001",
                WorkItemEdits {
                    title: Some("Temporary write".to_owned()),
                    ..WorkItemEdits::default()
                },
                "2026-01-03",
            )),
        );
        let result = plan.apply_with_options(&ApplyOptions::fail_after(0));

        assert!(!result.success, "{result:?}");
        assert!(
            result.rolled_back,
            "owned file write should restore exactly"
        );
        assert_eq!(fs::read(&path).unwrap(), before);
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
    fn external_drift_before_first_operation_is_preserved() {
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
        assert!(!result.rolled_back, "no transaction write occurred");
        let external = root.join("work/active/002-a-new-thing/work-item.json");
        assert_eq!(
            fs::read(external).unwrap(),
            b"external drift injected during apply",
            "rollback must not delete a target the transaction never wrote"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn external_edit_to_previously_written_target_is_preserved_as_recovery_conflict() {
        let (root, snapshot) = fixture();
        let plan = plan(
            &snapshot,
            MutationRequest::create_work_item(create_request("A New Thing", "2026-01-02")),
        );
        assert!(
            plan.file_operations.len() >= 2,
            "test requires a later failure point"
        );
        let first_path = match &plan.file_operations[0] {
            PathOperation::Write { path, .. } => path.clone(),
            PathOperation::Move { .. } => panic!("create plan should begin with a write"),
        };
        let external = b"external drift injected during apply";
        let result = plan.apply_with_options(&ApplyOptions {
            fail_before_operation: Some(1),
            external_drift_at: Some(1),
            external_drift_path: Some(first_path.clone()),
            ..ApplyOptions::default()
        });

        assert!(!result.success, "{result:?}");
        assert!(
            !result.rolled_back,
            "the transaction no longer owns the changed target"
        );
        assert!(
            result
                .diagnostics
                .iter()
                .any(|item| item.code == "mutation.rollback-failed"
                    && item.message.contains("recovery required")),
            "{result:?}"
        );
        assert_eq!(fs::read(root.join(first_path)).unwrap(), external);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_preimage_enumeration_is_not_treated_as_an_absent_directory() {
        let (root, _) = fixture();
        let repository = Repository::open(&root);
        let protected = root.join("work/active/001-one");
        let mut read_file = |path: &Path| fs::read(path);
        let mut fail_enumeration = true;
        let mut read_directory = |_path: &Path| {
            if fail_enumeration {
                fail_enumeration = false;
                Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "injected enumeration failure",
                ))
            } else {
                fs::read_dir(_path)
            }
        };
        let error = capture_preimages_with(
            &repository,
            &[PathBuf::from("work/active/001-one")],
            &mut read_file,
            &mut read_directory,
        )
        .unwrap_err();

        assert!(error.contains("cannot enumerate"), "{error}");
        assert!(protected.join("work-item.json").is_file());
        assert_eq!(fs::read(protected.join("extra.md")).unwrap(), b"history\n");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_preimage_file_read_is_not_treated_as_an_absent_file() {
        let (root, _) = fixture();
        let protected = root.join("work/active/001-one/work-item.json");
        let bytes_before = fs::read(&protected).unwrap();
        let repository = Repository::open(&root);
        let mut read_file = |_path: &Path| {
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "injected file read failure",
            ))
        };
        let mut read_directory = |path: &Path| fs::read_dir(path);
        let error = capture_preimages_with(
            &repository,
            &[PathBuf::from("work/active/001-one/work-item.json")],
            &mut read_file,
            &mut read_directory,
        )
        .unwrap_err();

        assert!(error.contains("injected file read failure"), "{error}");
        assert_eq!(fs::read(&protected).unwrap(), bytes_before);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn preimage_capture_rejects_a_a_b_b_a_a_file_observations_before_mutation() {
        let (root, _) = fixture();
        let target = root.join("captured-file");
        let sentinel = root.join("outside-sentinel");
        fs::write(&target, b"AAAA").unwrap();
        fs::write(&sentinel, b"keep external bytes").unwrap();
        let repository = Repository::open(&root);
        let mut read_count = 0;
        let mut read_file = |_path: &Path| {
            read_count += 1;
            let bytes = match read_count {
                1 | 2 | 5 | 6 => b"AAAA".as_slice(),
                3 | 4 => b"BBBB".as_slice(),
                _ => panic!("capture made an unexpected number of file reads"),
            };
            Ok(bytes.to_vec())
        };
        let mut read_directory = |path: &Path| fs::read_dir(path);

        let result = capture_preimages_with(
            &repository,
            &[PathBuf::from("captured-file")],
            &mut read_file,
            &mut read_directory,
        );

        let error = result.expect_err("middle B/B preimage must disagree with A/A boundaries");
        assert!(error.contains("target state changed"), "{error}");
        assert_eq!(read_count, 6);
        assert_eq!(fs::read(&target).unwrap(), b"AAAA");
        assert_eq!(fs::read(&sentinel).unwrap(), b"keep external bytes");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn preimage_capture_accepts_stable_file_observations() {
        let (root, _) = fixture();
        let target = root.join("captured-file");
        fs::write(&target, b"stable bytes").unwrap();
        let repository = Repository::open(&root);
        let mut read_file = |path: &Path| fs::read(path);
        let mut read_directory = |path: &Path| fs::read_dir(path);

        let preimages = capture_preimages_with(
            &repository,
            &[PathBuf::from("captured-file")],
            &mut read_file,
            &mut read_directory,
        )
        .unwrap();

        assert_eq!(
            preimages.entries.get(Path::new("captured-file")),
            Some(&PreimageEntry::File(b"stable bytes".to_vec()))
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn preimage_capture_rejects_empty_directory_added_only_to_middle_snapshot() {
        let (root, _) = fixture();
        let relative = PathBuf::from("capture-root");
        let target = root.join(&relative);
        let transient_empty = target.join("empty-during-capture");
        let sentinel = root.join("outside-sentinel");
        fs::create_dir(&target).unwrap();
        fs::write(&sentinel, b"keep external bytes").unwrap();
        let repository = Repository::open(&root);
        let mut read_file = |path: &Path| fs::read(path);
        let mut root_reads = 0;
        let mut read_directory = |path: &Path| {
            if path.ends_with("capture-root") {
                root_reads += 1;
                match root_reads {
                    3 => fs::create_dir(&transient_empty)?,
                    5 => fs::remove_dir(&transient_empty)?,
                    _ => {}
                }
                match root_reads {
                    1 | 2 | 5 | 6 => assert!(!transient_empty.exists()),
                    3 | 4 => assert!(transient_empty.is_dir()),
                    _ => panic!("capture made an unexpected number of root observations"),
                }
            }
            fs::read_dir(path)
        };

        let result = capture_preimages_with(
            &repository,
            &[relative],
            &mut read_file,
            &mut read_directory,
        );

        assert_eq!(
            root_reads, 6,
            "unexpected root observation count for {target:?}"
        );
        let error = result.expect_err("middle empty-directory entry must disagree with boundaries");
        assert!(error.contains("target state changed"), "{error}");
        assert!(!transient_empty.exists());
        assert_eq!(fs::read(&sentinel).unwrap(), b"keep external bytes");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn directory_move_with_symlink_is_rejected_before_mutation() {
        let (root, _) = fixture();
        let outside = root.with_extension("outside-sentinel");
        fs::create_dir_all(&outside).unwrap();
        let sentinel = outside.join("keep.txt");
        fs::write(&sentinel, b"outside data must survive").unwrap();
        let link = root.join("work/active/001-one/outside-link");
        #[cfg(unix)]
        symlink(&outside, &link).unwrap();
        #[cfg(windows)]
        symlink_dir(&outside, &link).unwrap();

        let snapshot = RepositorySession::open(&root).snapshot();
        let plan = plan(
            &snapshot,
            MutationRequest::transition_work_item(TransitionWorkItem::new("001", "blocked")),
        );
        assert!(plan.is_applicable(), "{:?}", plan.diagnostics);
        let result = plan.apply_with_options(&ApplyOptions::fail_after(0));

        assert!(!result.success, "unsafe preimage capture must fail closed");
        assert!(
            !result.rolled_back,
            "no operation may start before capture completes"
        );
        assert!(root.join("work/active/001-one/outside-link").exists());
        assert_eq!(fs::read(&sentinel).unwrap(), b"outside data must survive");
        assert!(
            result.diagnostics.iter().any(|item| {
                item.code == "mutation.preimage-failed" && item.message.contains("symlink")
            }),
            "{result:?}"
        );

        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(outside).unwrap();
    }

    #[test]
    fn directory_move_rejects_descendant_hard_link_alias() {
        let (root, snapshot) = fixture();
        let descendant = root.join("work/active/001-one/extra.md");
        let outside = root.with_extension("outside-descendant-hard-link");
        let before = fs::read(&descendant).unwrap();
        let plan = plan(
            &snapshot,
            MutationRequest::transition_work_item(TransitionWorkItem::new("001", "blocked")),
        );
        assert!(plan.is_applicable(), "{:?}", plan.diagnostics);
        fs::hard_link(&descendant, &outside).expect("create same-volume descendant alias");

        let result = plan.apply();
        let sentinel_after = fs::read(&outside).unwrap();
        let descendant_after = fs::read(&descendant).unwrap();
        fs::remove_file(&outside).unwrap();
        fs::remove_dir_all(&root).unwrap();

        assert!(
            !result.success,
            "captured hard-linked descendants must fail closed: {result:?}"
        );
        assert!(
            !result.rolled_back,
            "no transaction write may start: {result:?}"
        );
        assert_eq!(sentinel_after, before);
        assert_eq!(descendant_after, before);
        assert!(
            result.diagnostics.iter().any(|diagnostic| {
                diagnostic.code == "mutation.preimage-failed"
                    && diagnostic.message.contains("hard link")
            }),
            "{result:?}"
        );
    }

    #[cfg(windows)]
    #[test]
    fn directory_move_with_junction_is_rejected_before_mutation() {
        let (root, snapshot) = fixture();
        let outside = root.with_extension("outside-junction-sentinel");
        fs::create_dir_all(&outside).unwrap();
        let sentinel = outside.join("keep.txt");
        fs::write(&sentinel, b"outside data must survive").unwrap();
        let junction = root.join("work/active/001-one/outside-junction");
        let junction_literal = format!("'{}'", junction.to_string_lossy().replace('\'', "''"));
        let outside_literal = format!("'{}'", outside.to_string_lossy().replace('\'', "''"));
        let created = Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command"])
            .arg(format!(
                "New-Item -ItemType Junction -Path {junction_literal} -Target {outside_literal} -ErrorAction Stop | Out-Null"
            ))
            .output()
            .expect("run the Windows junction helper");
        if !created.status.success() {
            eprintln!(
                "junction regression skipped because this Windows host could not create the junction: {}",
                String::from_utf8_lossy(&created.stderr).trim()
            );
            fs::remove_dir_all(&root).unwrap();
            fs::remove_dir_all(&outside).unwrap();
            return;
        }
        eprintln!("junction regression exercised a real Windows junction");

        let plan = plan(
            &snapshot,
            MutationRequest::transition_work_item(TransitionWorkItem::new("001", "blocked")),
        );
        assert!(plan.is_applicable(), "{:?}", plan.diagnostics);
        let result = plan.apply();
        let sentinel_after = fs::read(&sentinel).unwrap();
        fs::remove_dir(&junction).unwrap();
        fs::remove_dir_all(&root).unwrap();
        fs::remove_dir_all(&outside).unwrap();

        assert!(!result.success, "junction must fail closed: {result:?}");
        assert!(!result.rolled_back, "no operation may start: {result:?}");
        assert_eq!(sentinel_after, b"outside data must survive");
        assert!(
            result.diagnostics.iter().any(|item| {
                item.code == "mutation.preimage-failed"
                    && (item.message.contains("reparse") || item.message.contains("symlink"))
            }),
            "{result:?}"
        );
    }

    #[test]
    fn edit_rejects_hard_link_alias_before_overwriting_external_sentinel() {
        let (root, snapshot) = fixture();
        let work_record = root.join("work/active/001-one/work-item.json");
        let outside = root.with_extension("outside-hard-link-sentinel");
        let plan = plan(
            &snapshot,
            MutationRequest::edit_work_item(EditWorkItem::new(
                "001",
                WorkItemEdits::title("Edited title"),
                "2026-01-03",
            )),
        );
        assert!(plan.is_applicable(), "{:?}", plan.diagnostics);
        fs::hard_link(&work_record, &outside).expect("create same-volume hard-link sentinel");
        let sentinel_before = fs::read(&outside).unwrap();

        let result = plan.apply();
        let work_record_after = fs::read(&work_record).unwrap();
        let sentinel_after = fs::read(&outside).unwrap();
        fs::remove_file(&outside).unwrap();
        fs::remove_dir_all(&root).unwrap();

        assert!(
            !result.success,
            "hard-linked mutation target must fail closed: {result:?}"
        );
        assert!(
            !result.rolled_back,
            "no transaction write may start: {result:?}"
        );
        assert_eq!(work_record_after, sentinel_before);
        assert_eq!(sentinel_after, sentinel_before);
        assert!(
            result.diagnostics.iter().any(|diagnostic| {
                diagnostic.code == "mutation.preimage-failed"
                    && diagnostic.message.contains("hard link")
            }),
            "{result:?}"
        );
    }

    #[test]
    fn rollback_recheck_preserves_a_new_hard_linked_after_state() {
        let (root, _) = fixture();
        let repository = Repository::open(&root);
        let relative = "work/active/001-one/work-item.json";
        let path = root.join(relative);
        let outside = root.with_extension("outside-rollback-hard-link");
        let before = capture_preimages(&repository, &[PathBuf::from(relative)]).unwrap();
        let operation = PathOperation::Write {
            path: relative.to_owned(),
            content: "transaction after-state\n".to_owned(),
        };
        let after = project_operation(&repository, &before, &operation).unwrap();
        apply_operation(&root, &operation).unwrap();
        let owned_after = fs::read(&path).unwrap();
        fs::hard_link(&path, &outside).expect("create after-state alias before rollback recheck");
        let record = UndoRecord {
            operation,
            before,
            after,
        };

        let error = restore_operation(&repository, &record).unwrap_err();
        let target_after = fs::read(&path).unwrap();
        let sentinel_after = fs::read(&outside).unwrap();
        fs::remove_file(&outside).unwrap();
        fs::remove_dir_all(&root).unwrap();

        assert!(error.contains("hard link"), "{error}");
        assert_eq!(target_after, owned_after);
        assert_eq!(sentinel_after, owned_after);
    }

    #[test]
    fn edit_rejects_valid_external_change_between_stale_check_and_capture() {
        let (root, snapshot) = fixture();
        let relative = "work/active/001-one/work-item.json";
        let path = root.join(relative);
        let plan = plan(
            &snapshot,
            MutationRequest::edit_work_item(EditWorkItem::new(
                "001",
                WorkItemEdits::title("Planned edit"),
                "2026-01-03",
            )),
        );
        assert!(plan.is_applicable(), "{:?}", plan.diagnostics);
        let result = plan.apply_with_options(&ApplyOptions {
            external_drift_after_initial_check: Some(relative.to_owned()),
            ..ApplyOptions::default()
        });
        let external_after = fs::read(&path).unwrap();
        let value: Value = serde_json::from_slice(&external_after).unwrap();
        fs::remove_dir_all(root).unwrap();

        assert!(!result.success, "captured plan input changed: {result:?}");
        assert!(
            !result.rolled_back,
            "no transaction write may start: {result:?}"
        );
        assert_eq!(value["title"], "External edit");
        assert_eq!(value["updated"], "2026-01-02");
        assert!(
            result.diagnostics.iter().any(|diagnostic| {
                diagnostic.code == "mutation.plan-stale"
                    && diagnostic.message.contains("captured state")
            }),
            "{result:?}"
        );
    }

    #[test]
    fn consumed_work_record_drift_is_stale() {
        const WORK_RECORD: &str = "work/active/001-one/work-item.json";
        let (root, snapshot) = fixture();
        let plan = plan(
            &snapshot,
            MutationRequest::create_work_item(create_request("A New Thing", "2026-01-02")),
        );
        if !plan.is_applicable() {
            // Failure-only forensics. Never print an inherited environment value.
            let repository = Repository::open(&root);
            let observed = repository.path_state(&root.join(WORK_RECORD));
            let read_fact = plan
                .read_set
                .facts
                .iter()
                .find(|fact| fact.path == WORK_RECORD);
            panic!(
                "baseline plan must be applicable before injected drift; fixture root={root:?}; pid={}; thread={:?}; planned_identity={:?}; live_identity={:?}; read_fact_count={}; work_record_read_fact={read_fact:?}; work_record_before={observed:?}; diagnostics={:?}; cleanup=fixture retained after panic for inspection",
                std::process::id(),
                std::thread::current().id(),
                plan.repository_identity,
                repository.identity(),
                plan.read_set.facts.len(),
                plan.diagnostics
            );
        }
        let repository = Repository::open(&root);
        let work_record = root.join(WORK_RECORD);
        let before = repository.path_state(&work_record);
        fs::write(
            &work_record,
            br#"{"id":"001","title":"Changed","status":"active","owner_scope":"governance","affected_scopes":[],"depends_on":[],"acceptance_criteria":[{"id":"AC-1","text":"prove","state":"pending","evidence":[]}],"created":"2026-01-01","updated":"2026-01-01"}"#,
        )
        .unwrap();
        let after_write = repository.path_state(&work_record);
        let result = plan.apply();
        let stale = result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "mutation.plan-stale");
        if result.success || !stale {
            // Includes the exact read fact used by the plan and three live states.
            // Hashes are PathState digests, not raw work-record contents.
            let read_fact = plan
                .read_set
                .facts
                .iter()
                .find(|fact| fact.path == WORK_RECORD);
            let after_apply = repository.path_state(&work_record);
            panic!(
                "expected failed apply with mutation.plan-stale after consumed record drift; fixture root={root:?}; pid={}; thread={:?}; planned_identity={:?}; live_identity={:?}; read_fact_count={}; work_record_read_fact={read_fact:?}; before={before:?}; after_write={after_write:?}; after_apply={after_apply:?}; result={result:?}; cleanup=fixture retained after panic for inspection",
                std::process::id(),
                std::thread::current().id(),
                plan.repository_identity,
                repository.identity(),
                plan.read_set.facts.len()
            );
        }
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
    fn proposal_rejects_authority_drift_after_validation_and_during_staging() {
        let (root, snapshot) = fixture();
        let request = proposal_request("Authority race", "authority-race-key");
        let plan = plan(&snapshot, MutationRequest::create_work_item(request));
        let result =
            plan.apply_with_options(&ApplyOptions::external_drift(1, "governance/owners.json"));
        assert!(
            !result.success,
            "stale authorization must not commit: {result:?}"
        );
        assert!(result.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "work.proposal-commit-failed"
                && diagnostic.message.contains("governance/owners.json")
        }));
        assert!(!root.join("work/proposed/002-authority-race").exists());
        assert_eq!(proposal_status(&root, "authority-race-key").unwrap(), None);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn post_publish_dashboard_failure_does_not_misreport_or_undo_the_commit() {
        let (root, _) = fixture();
        let dashboard = root.join("audits/reports/dashboard.md");
        fs::remove_file(&dashboard).unwrap();
        fs::create_dir(&dashboard).unwrap();

        let result = propose_work_item(&root, proposal_request("Published", "published-key"));
        assert!(result.success, "{:?}", result.diagnostics);
        assert!(result.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "mutation.dashboard-regeneration-failed" && !diagnostic.blocking
        }));
        assert!(root
            .join("work/proposed/002-published/work-item.json")
            .is_file());
        assert!(root.join("work/proposed/002-published/README.md").is_file());
        assert!(root
            .join("work/proposed/002-published/proposal-receipt.json")
            .is_file());
        assert!(proposal_status(&root, "published-key").unwrap().is_some());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn nonproposal_mutation_waits_for_proposal_publish_then_rejects_its_stale_id_plan() {
        let (root, _) = fixture();
        ensure_safe_repository_directory(&root, Path::new("work/proposed")).unwrap();
        let snapshot = RepositorySession::open(&root).snapshot();
        let proposal = plan(
            &snapshot,
            MutationRequest::create_work_item(proposal_request(
                "Concurrent proposal",
                "concurrent-proposal-key",
            )),
        );
        let competing = plan(
            &snapshot,
            MutationRequest::create_work_item(create_request(
                "Concurrent active item",
                "2026-10-27",
            )),
        );
        assert!(proposal.is_applicable(), "{:?}", proposal.diagnostics);
        assert!(competing.is_applicable(), "{:?}", competing.diagnostics);

        let repository = Repository::open(&root);
        let lock = RepositoryMutationLock::acquire(repository.root()).unwrap();
        let (started_tx, started_rx) = mpsc::channel();
        let (result_tx, result_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            result_tx.send(competing.apply()).unwrap();
        });
        started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("competing mutation starts while the proposal owns the lock");

        let mut initial = empty_result();
        initial.plan_token = proposal.plan_token.clone();
        initial.diagnostics = proposal.diagnostics.clone();
        let committed = commit_proposal_locked_with_hook(
            &repository,
            &proposal,
            &snapshot,
            &ApplyOptions::default(),
            initial,
            || match result_rx.recv_timeout(Duration::from_millis(150)) {
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Ok(result) => {
                    panic!("non-proposal mutation completed before proposal publish: {result:?}")
                }
                Err(error) => panic!("competing mutation channel failed: {error}"),
            },
        );
        assert!(committed.success, "{:?}", committed.diagnostics);
        drop(lock);

        let competing_result = result_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("competing mutation finishes after proposal releases the lock");
        worker.join().unwrap();
        assert!(!competing_result.success, "{competing_result:?}");
        assert!(competing_result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "mutation.plan-stale"));
        assert!(root.join("work/proposed/002-concurrent-proposal").is_dir());
        assert!(!root.join("work/active/002-concurrent-active-item").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn proposal_receipt_lookup_follows_the_item_through_lifecycle_transitions() {
        let (root, _) = fixture();
        let request = proposal_request("Lifecycle receipt", "lifecycle-receipt-key");
        let created = propose_work_item(&root, request.clone());
        assert!(created.success, "{:?}", created.diagnostics);

        let snapshot = RepositorySession::open(&root).snapshot();
        let transition = plan(
            &snapshot,
            MutationRequest::transition_work_item(TransitionWorkItem::new("002", "active")),
        );
        assert!(transition.is_applicable(), "{:?}", transition.diagnostics);
        let moved = transition.apply();
        assert!(moved.success, "{:?}", moved.diagnostics);

        let receipt = proposal_status(&root, "lifecycle-receipt-key")
            .unwrap()
            .expect("receipt remains visible after lifecycle move");
        assert_eq!(
            receipt["current_work_item_path"],
            "work/active/002-lifecycle-receipt"
        );
        let replay = propose_work_item(&root, request);
        assert!(replay.success, "{:?}", replay.diagnostics);
        assert!(replay.replayed);
        assert_eq!(
            fs::read_dir(root.join("work/active")).unwrap().count(),
            2,
            "same-key replay after transition must not create another item"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn matching_corrupt_receipt_fails_closed_but_valid_unrelated_receipt_does_not_couple_lookup() {
        let (root, _) = fixture();
        assert!(propose_work_item(&root, proposal_request("Original", "original-key")).success);
        let receipt_path = root.join("work/proposed/002-original/proposal-receipt.json");
        let mut receipt: Value = serde_json::from_slice(&fs::read(&receipt_path).unwrap()).unwrap();
        receipt["work_item_id"] = Value::String("999".to_owned());
        fs::write(&receipt_path, canonical_json(&receipt)).unwrap();
        assert!(proposal_status(&root, "original-key").is_err());

        receipt["work_item_id"] = Value::String("002".to_owned());
        receipt["request_digest"] = Value::String("broken".to_owned());
        fs::write(&receipt_path, canonical_json(&receipt)).unwrap();

        assert!(proposal_status(&root, "original-key").is_err());
        assert_eq!(proposal_status(&root, "different-key").unwrap(), None);
        let unrelated = propose_work_item(&root, proposal_request("Unrelated", "different-key"));
        assert!(unrelated.success, "{:?}", unrelated.diagnostics);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn malformed_historical_receipt_fails_closed_when_its_key_cannot_be_identified() {
        let (root, _) = fixture();
        let corrupt = root.join("work/proposed/900-corrupt/proposal-receipt.json");
        fs::create_dir_all(corrupt.parent().unwrap()).unwrap();
        fs::write(&corrupt, b"not-json\n").unwrap();

        assert!(proposal_status(&root, "unrelated-key").is_err());
        fs::write(&corrupt, b"{}\n").unwrap();
        assert!(proposal_status(&root, "unrelated-key").is_err());
        let result = propose_work_item(&root, proposal_request("Candidate", "unrelated-key"));
        assert!(!result.success);
        assert!(result
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == "work.proposal-receipt-read-failed" }));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_receipt_is_absent_even_when_a_historical_item_remains() {
        let (root, _) = fixture();
        assert!(
            propose_work_item(&root, proposal_request("Receipt removed", "removed-key")).success
        );
        fs::remove_file(root.join("work/proposed/002-receipt-removed/proposal-receipt.json"))
            .unwrap();

        assert_eq!(proposal_status(&root, "removed-key").unwrap(), None);
        assert!(root
            .join("work/proposed/002-receipt-removed/work-item.json")
            .is_file());
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
