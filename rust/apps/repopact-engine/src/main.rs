use std::fs;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use repopact_analysis::{AnalysisQuery, AnalysisReport};
use repopact_core::RepoPactCore;
use repopact_mutation::{
    CreateWorkItem, EditWorkItem, MutationRequest, MutationResult, WorkItemEdits,
};
use repopact_protocol::{
    capabilities, EngineBuildInfo, EngineRequest, EngineResponse, ProtocolDiagnostic, PROTOCOL,
    PROTOCOL_VERSION,
};
use repopact_types::{AcceptanceCriterion, Diagnostic, Severity, WorkItem};
use serde::Deserialize;
use serde_json::{json, Value};

mod query_ops;

pub(crate) const ENGINE_VERSION: &str = env!("REPOPACT_ENGINE_VERSION");
const ENGINE_SEMANTIC_IDENTITY: &str = env!("REPOPACT_ENGINE_SEMANTIC_IDENTITY");

fn engine_build_info() -> EngineBuildInfo {
    EngineBuildInfo {
        source_revision: env!("REPOPACT_ENGINE_SOURCE_REVISION").to_owned(),
        rustc_version: env!("REPOPACT_ENGINE_RUSTC_VERSION").to_owned(),
        target: env!("REPOPACT_ENGINE_TARGET").to_owned(),
        profile: env!("REPOPACT_ENGINE_PROFILE").to_owned(),
    }
}

#[derive(Default)]
struct EngineSession {
    handshaken: bool,
    admitted: bool,
}

#[derive(Debug, Deserialize)]
struct CreateParams {
    title: String,
    date: String,
    #[serde(default = "default_active")]
    status: String,
    #[serde(default)]
    preflight_confirmed_before_work_started: bool,
    #[serde(default)]
    acceptance_criteria: Vec<AcceptanceCriterion>,
    #[serde(default)]
    idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ProposalStatusParams {
    idempotency_key: String,
}

#[derive(Debug, Deserialize)]
struct AmendProposalParams {
    id: String,
    title: String,
    date: String,
}

fn default_active() -> String {
    "active".to_owned()
}

fn main() -> ExitCode {
    let stdin = io::stdin();
    let mut session = EngineSession::default();
    let mut stdout = io::stdout().lock();
    for line in stdin.lock().lines() {
        let response = match line {
            Ok(input) => match serde_json::from_str::<EngineRequest>(&input) {
                Ok(request) => handle(request, &mut session),
                Err(error) => EngineResponse::failure(
                    "",
                    ENGINE_VERSION,
                    "protocol.malformed-request",
                    format!("request must be one JSON object per line: {error}"),
                ),
            },
            Err(error) => EngineResponse::failure(
                "",
                ENGINE_VERSION,
                "protocol.stdin",
                format!("unable to read request: {error}"),
            ),
        };
        if emit(&mut stdout, response).is_err() {
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}

fn emit(stdout: &mut impl Write, mut response: EngineResponse) -> io::Result<()> {
    response.semantic_identity = Some(ENGINE_SEMANTIC_IDENTITY.to_owned());
    response.build_info = Some(engine_build_info());
    match serde_json::to_string(&response) {
        Ok(encoded) => {
            writeln!(stdout, "{encoded}")?;
            stdout.flush()
        }
        Err(error) => {
            eprintln!("unable to encode engine response: {error}");
            Err(io::Error::other(error))
        }
    }
}

fn handle(request: EngineRequest, session: &mut EngineSession) -> EngineResponse {
    if request.protocol != PROTOCOL {
        return EngineResponse::failure(
            request.request_id,
            ENGINE_VERSION,
            "protocol.name-mismatch",
            format!(
                "unsupported protocol '{}'; expected '{PROTOCOL}'",
                request.protocol
            ),
        );
    }
    if request.protocol_version != PROTOCOL_VERSION {
        return EngineResponse::failure(
            request.request_id,
            ENGINE_VERSION,
            "protocol.version-mismatch",
            format!(
                "unsupported protocol major {}; expected {}",
                request.protocol_version, PROTOCOL_VERSION
            ),
        );
    }
    let operation = request.operation.clone();
    match operation.as_str() {
        "handshake" | "capabilities" => {
            session.handshaken = true;
            handshake(&request)
        }
        "admit" => admit(&request, session),
        operation if !session.admitted => EngineResponse::failure(
            request.request_id.clone(),
            ENGINE_VERSION,
            "engine.not-admitted",
            format!("operation '{operation}' was refused before semantic admission"),
        ),
        operation => dispatch(request, operation),
    }
}

fn dispatch(request: EngineRequest, operation: &str) -> EngineResponse {
    match operation {
        "validate" => validate(&request),
        "dashboard.write" => dashboard_write(&request),
        "work.create" | "work.propose" => create_work(&request),
        "work.proposal-plan" => proposal_plan(&request),
        "work.proposal-status" => proposal_status(&request),
        "work.amend_proposal" => amend_proposal(&request),
        "graph" => graph(&request),
        "graph.status" => graph_status(&request),
        "graph.build" => graph_build(&request),
        "graph.verify" => graph_verify(&request),
        "graph.update" => graph_update(&request),
        "graph.disable" => graph_disable(&request),
        "graph.resolve" => query_ops::graph_resolve(&request),
        "graph.search" => query_ops::graph_search(&request),
        "graph.reconcile-merge" => graph_reconcile_merge(&request),
        "graph.context" => query_ops::graph_context(&request),
        "graph.neighbors" => query_ops::graph_neighbors(&request),
        "graph.path" => query_ops::graph_path(&request),
        "graph.dependencies" => query_ops::graph_dependencies(&request),
        "graph.dependents" => query_ops::graph_dependents(&request),
        "graph.tests" => query_ops::graph_tests(&request),
        "graph.governance" => query_ops::graph_governance(&request),
        "graph.impact" => query_ops::graph_impact(&request),
        "graph.orient" => query_ops::graph_orient(&request),
        "analyze" => analyze(&request),
        "assurance.snapshot" => assurance_snapshot(&request),
        "documentation.snapshot" => documentation_snapshot(&request),
        operation => EngineResponse::failure(
            request.request_id.clone(),
            ENGINE_VERSION,
            "operation.unsupported",
            format!("unsupported engine operation '{operation}'"),
        ),
    }
}

fn admit(request: &EngineRequest, session: &mut EngineSession) -> EngineResponse {
    if !session.handshaken {
        return EngineResponse::failure(
            request.request_id.clone(),
            ENGINE_VERSION,
            "engine.admission-order",
            "semantic admission requires a successful handshake in this process",
        );
    }
    let requested_identity = request
        .params
        .get("semantic_identity")
        .and_then(Value::as_str);
    if requested_identity != Some(ENGINE_SEMANTIC_IDENTITY) {
        session.admitted = false;
        return EngineResponse::failure(
            request.request_id.clone(),
            ENGINE_VERSION,
            "engine.semantic-identity-mismatch",
            "client and engine semantic-contract identities do not match",
        );
    }
    session.admitted = true;
    let mut response = EngineResponse::success(
        request.request_id.clone(),
        ENGINE_VERSION,
        json!({
            "status": "admitted",
            "semantic_identity": ENGINE_SEMANTIC_IDENTITY,
        }),
    );
    response.semantic_identity = Some(ENGINE_SEMANTIC_IDENTITY.to_owned());
    response
}

fn handshake(request: &EngineRequest) -> EngineResponse {
    let mut response = EngineResponse::success(
        request.request_id.clone(),
        ENGINE_VERSION,
        json!({"status": "ready"}),
    );
    response.capabilities = Some(capabilities());
    response
}

pub(crate) fn require_root(request: &EngineRequest) -> Result<PathBuf, EngineResponse> {
    let Some(root) = request.root.as_deref() else {
        return Err(EngineResponse::failure(
            request.request_id.clone(),
            ENGINE_VERSION,
            "repository.root-required",
            "this engine operation requires a repository root",
        ));
    };
    let root = PathBuf::from(root);
    if !root.is_dir() {
        return Err(EngineResponse::failure(
            request.request_id.clone(),
            ENGINE_VERSION,
            "repository.invalid-root",
            format!("repository root is not a directory: {}", root.display()),
        ));
    }
    Ok(root)
}

fn validate(request: &EngineRequest) -> EngineResponse {
    let root = match require_root(request) {
        Ok(root) => root,
        Err(response) => return response,
    };
    let report = RepoPactCore::open(root).validate();
    validation_response(request, report)
}

fn validation_response(
    request: &EngineRequest,
    report: repopact_types::ValidationReport,
) -> EngineResponse {
    let diagnostics = report
        .diagnostics
        .iter()
        .map(protocol_diagnostic)
        .collect::<Vec<_>>();
    let error_count = report
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .count();
    let warning_count = report
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Warning)
        .count();
    let mut response = EngineResponse::success(
        request.request_id.clone(),
        ENGINE_VERSION,
        json!({
            "valid": error_count == 0,
            "error_count": error_count,
            "warning_count": warning_count,
            "diagnostics": diagnostics,
        }),
    );
    response.diagnostics = diagnostics;
    response
}

fn dashboard_write(request: &EngineRequest) -> EngineResponse {
    let root = match require_root(request) {
        Ok(root) => root,
        Err(response) => return response,
    };
    let core = RepoPactCore::open(&root);
    let snapshot = core.snapshot();
    let content = match repopact_validation::render_dashboard_snapshot(&snapshot) {
        Ok(content) => content,
        Err(error) => return semantic_failure(request, "dashboard.render", error),
    };
    let path = root.join("audits/reports/dashboard.md");
    if let Some(parent) = path.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            return EngineResponse::failure(
                request.request_id.clone(),
                ENGINE_VERSION,
                "dashboard.write-failed",
                format!("unable to create dashboard directory: {error}"),
            );
        }
    }
    if let Err(error) = fs::write(&path, content) {
        return EngineResponse::failure(
            request.request_id.clone(),
            ENGINE_VERSION,
            "dashboard.write-failed",
            format!("unable to write dashboard: {error}"),
        );
    }
    EngineResponse::success(
        request.request_id.clone(),
        ENGINE_VERSION,
        json!({"path": "audits/reports/dashboard.md"}),
    )
}

fn create_work(request: &EngineRequest) -> EngineResponse {
    let root = match require_root(request) {
        Ok(root) => root,
        Err(response) => return response,
    };
    let params = match serde_json::from_value::<CreateParams>(request.params.clone()) {
        Ok(params) => params,
        Err(error) => {
            return semantic_failure(request, "work.invalid-parameters", error.to_string())
        }
    };
    if request.operation == "work.propose" {
        let operation_started = std::time::Instant::now();
        let mut intent = CreateWorkItem::new(params.title, params.date).with_status("proposed");
        if let Some(key) = params.idempotency_key {
            intent = intent.with_idempotency_key(key);
        }
        if params.preflight_confirmed_before_work_started {
            intent = intent.confirm_preflight_before_work_started();
        }
        if !params.acceptance_criteria.is_empty() {
            intent = intent.with_acceptance_criteria(params.acceptance_criteria);
        }
        let mut result = repopact_mutation::propose_work_item(&root, intent);
        result.metrics.engine_execution_ms = operation_started.elapsed().as_millis();
        return mutation_response(request, result);
    }
    if params.status == "proposed" {
        return semantic_failure(
            request,
            "work.proposal-operation-required",
            "proposed work must use the idempotent work.propose operation",
        );
    }
    let status = params.status;
    let core = RepoPactCore::open(&root);
    let snapshot_started = std::time::Instant::now();
    let snapshot = core.snapshot();
    let snapshot_ms = snapshot_started.elapsed().as_millis();
    let mut intent = CreateWorkItem::new(params.title, params.date)
        .with_status(status)
        .with_acceptance_criteria(params.acceptance_criteria);
    if params.preflight_confirmed_before_work_started {
        intent = intent.confirm_preflight_before_work_started();
    }
    let planning_started = std::time::Instant::now();
    let plan = core.plan_mutation_snapshot(&snapshot, MutationRequest::create_work_item(intent));
    let planning_ms = planning_started.elapsed().as_millis();
    let operation_started = std::time::Instant::now();
    let result = plan.apply();
    let mut result = result;
    result.metrics.snapshot_ms = snapshot_ms;
    result.metrics.planning_ms = planning_ms;
    result.metrics.engine_execution_ms =
        operation_started.elapsed().as_millis() + snapshot_ms + planning_ms;
    mutation_response(request, result)
}

fn proposal_plan(request: &EngineRequest) -> EngineResponse {
    let root = match require_root(request) {
        Ok(root) => root,
        Err(response) => return response,
    };
    let params = match serde_json::from_value::<CreateParams>(request.params.clone()) {
        Ok(params) => params,
        Err(error) => {
            return semantic_failure(request, "work.invalid-parameters", error.to_string())
        }
    };
    let Some(key) = params.idempotency_key.clone() else {
        return semantic_failure(
            request,
            "work.proposal-key-required",
            "work.proposal-plan requires an idempotency key",
        );
    };
    let operation_started = std::time::Instant::now();
    let core = RepoPactCore::open(&root);
    let snapshot_started = std::time::Instant::now();
    let snapshot = core.snapshot();
    let snapshot_ms = snapshot_started.elapsed().as_millis();
    let mut intent = CreateWorkItem::new(params.title, params.date)
        .with_status("proposed")
        .with_idempotency_key(key);
    if params.preflight_confirmed_before_work_started {
        intent = intent.confirm_preflight_before_work_started();
    }
    if !params.acceptance_criteria.is_empty() {
        intent = intent.with_acceptance_criteria(params.acceptance_criteria);
    }
    let planning_started = std::time::Instant::now();
    let plan = core.plan_mutation_snapshot(&snapshot, MutationRequest::create_work_item(intent));
    let planning_ms = planning_started.elapsed().as_millis();
    let mut metrics = plan.metrics.clone();
    metrics.snapshot_ms = snapshot_ms;
    metrics.planning_ms = planning_ms;
    metrics.engine_execution_ms = operation_started.elapsed().as_millis();
    let diagnostics = plan
        .diagnostics
        .iter()
        .map(mutation_diagnostic)
        .collect::<Vec<_>>();
    let result = json!({
        "applicable": plan.is_applicable(),
        "plan_token": plan.plan_token,
        "preview": plan.preview,
        "diagnostics": plan.diagnostics,
        "metrics": metrics,
    });
    let mut response = EngineResponse::success(request.request_id.clone(), ENGINE_VERSION, result);
    response.diagnostics = diagnostics;
    response
}

fn proposal_status(request: &EngineRequest) -> EngineResponse {
    let root = match require_root(request) {
        Ok(root) => root,
        Err(response) => return response,
    };
    let params = match serde_json::from_value::<ProposalStatusParams>(request.params.clone()) {
        Ok(params) => params,
        Err(error) => {
            return semantic_failure(request, "work.invalid-parameters", error.to_string())
        }
    };
    match repopact_mutation::proposal_status(&root, &params.idempotency_key) {
        Ok(Some(mut receipt)) => {
            if let Some(object) = receipt.as_object_mut() {
                object.insert("state".to_owned(), Value::String("committed".to_owned()));
            }
            EngineResponse::success(request.request_id.clone(), ENGINE_VERSION, receipt)
        }
        Ok(None) => EngineResponse::success(
            request.request_id.clone(),
            ENGINE_VERSION,
            json!({"state": "absent", "idempotency_key": params.idempotency_key}),
        ),
        Err(error) => semantic_failure(request, "work.proposal-status-failed", error),
    }
}

fn amend_proposal(request: &EngineRequest) -> EngineResponse {
    let root = match require_root(request) {
        Ok(root) => root,
        Err(response) => return response,
    };
    let params = match serde_json::from_value::<AmendProposalParams>(request.params.clone()) {
        Ok(params) => params,
        Err(error) => {
            return semantic_failure(request, "work.invalid-parameters", error.to_string())
        }
    };
    let core = RepoPactCore::open(&root);
    let snapshot = core.snapshot();
    let Some(record) = snapshot.index().work_item(&params.id) else {
        return semantic_failure(
            request,
            "mutation.work-not-found",
            format!(
                "unknown work item '{}'; only proposed work may be amended",
                params.id
            ),
        );
    };
    let is_proposed = record
        .value
        .as_ref()
        .ok()
        .and_then(|value| serde_json::from_value::<WorkItem>(value.clone()).ok())
        .is_some_and(|item| item.status == "proposed");
    if !is_proposed {
        return semantic_failure(
            request,
            "mutation.proposal-only",
            "Only proposed work may be amended".to_owned(),
        );
    }
    let intent = EditWorkItem::new(params.id, WorkItemEdits::title(params.title), params.date);
    let plan = core.plan_mutation_snapshot(&snapshot, MutationRequest::edit_work_item(intent));
    let result = plan.apply();
    mutation_response(request, result)
}

fn graph(request: &EngineRequest) -> EngineResponse {
    let root = match require_root(request) {
        Ok(root) => root,
        Err(response) => return response,
    };
    let core = RepoPactCore::open(root);
    let snapshot = core.snapshot();
    EngineResponse::success(
        request.request_id.clone(),
        ENGINE_VERSION,
        serde_json::to_value(core.graph_snapshot(&snapshot)).unwrap_or(Value::Null),
    )
}

/// WI063 foundation graph operations. These never write outside `rog/`
/// (`graph.build`) or write at all (`graph.status`/`graph.verify`); they
/// never call out to a remote provider, and a repository with no durable
/// graph is reported as `absent`, not as an error (Decision 0044 section
/// 9). `graph.verify` recomputes the source-projection fingerprint (a full
/// walk over the current source), so it is not free, but it performs no
/// Git invocations beyond what `RepositorySession::snapshot()`/
/// `Repository::topology()` already cost.
fn graph_status(request: &EngineRequest) -> EngineResponse {
    let root = match require_root(request) {
        Ok(root) => root,
        Err(response) => return response,
    };
    let core = RepoPactCore::open(root);
    let status = repopact_graph::status::status(core.repository());
    EngineResponse::success(
        request.request_id.clone(),
        ENGINE_VERSION,
        serde_json::to_value(status).unwrap_or(Value::Null),
    )
}

fn graph_build(request: &EngineRequest) -> EngineResponse {
    let root = match require_root(request) {
        Ok(root) => root,
        Err(response) => return response,
    };
    let core = RepoPactCore::open(root);
    let snapshot = core.snapshot();
    match repopact_graph::build_and_write(&snapshot) {
        Ok(manifest) => EngineResponse::success(
            request.request_id.clone(),
            ENGINE_VERSION,
            serde_json::to_value(manifest).unwrap_or(Value::Null),
        ),
        Err(error) => semantic_failure(request, error.code, error.message),
    }
}

/// WI063 adoption/backfill/clean-clone checkpoint (Decision 0051
/// section 4): explicit disable. Removes the derived `rog/` directory
/// and persists `capabilities.rog = disabled` -- idempotent, never
/// touches source/governance.
fn graph_disable(request: &EngineRequest) -> EngineResponse {
    let root = match require_root(request) {
        Ok(root) => root,
        Err(response) => return response,
    };
    match repopact_graph::disable_graph(&root) {
        Ok(()) => EngineResponse::success(
            request.request_id.clone(),
            ENGINE_VERSION,
            json!({"disabled": true}),
        ),
        Err(error) => semantic_failure(request, error.code, error.message),
    }
}

/// ROG-029, Decision 0052 section 4: the explicit, never-automatic
/// derived-graph merge repair command. Refuses closed on any unresolved
/// authoritative source/configuration path (including
/// `governance/rog-capability.json` itself); regenerates and stages
/// `rog/**` only when every unresolved path is confined to it.
fn graph_reconcile_merge(request: &EngineRequest) -> EngineResponse {
    let root = match require_root(request) {
        Ok(root) => root,
        Err(response) => return response,
    };
    let core = RepoPactCore::open(root);
    let snapshot = core.snapshot();
    match repopact_graph::merge_reconcile::reconcile_merge(core.repository(), &snapshot) {
        Ok(repopact_graph::merge_reconcile::ReconcileOutcome::NothingToReconcile) => {
            EngineResponse::success(
                request.request_id.clone(),
                ENGINE_VERSION,
                json!({"outcome": "nothing_to_reconcile"}),
            )
        }
        Ok(repopact_graph::merge_reconcile::ReconcileOutcome::Repaired {
            staged_paths,
            manifest,
        }) => EngineResponse::success(
            request.request_id.clone(),
            ENGINE_VERSION,
            json!({
                "outcome": "repaired",
                "staged_paths": staged_paths,
                "manifest": manifest,
            }),
        ),
        Err(error) => {
            let code = match &error {
                repopact_graph::merge_reconcile::ReconcileError::AuthoritativeConflict {
                    ..
                } => "graph.reconcile-merge.authoritative-conflict",
                repopact_graph::merge_reconcile::ReconcileError::CapabilityConflict => {
                    "graph.reconcile-merge.capability-conflict"
                }
                repopact_graph::merge_reconcile::ReconcileError::GitQueryFailed(_) => {
                    "graph.reconcile-merge.git-query-failed"
                }
                repopact_graph::merge_reconcile::ReconcileError::CapabilityUnreadable(_) => {
                    "graph.reconcile-merge.capability-unreadable"
                }
                repopact_graph::merge_reconcile::ReconcileError::RebuildFailed(_) => {
                    "graph.reconcile-merge.rebuild-failed"
                }
                repopact_graph::merge_reconcile::ReconcileError::StageFailed(_) => {
                    "graph.reconcile-merge.stage-failed"
                }
            };
            semantic_failure(request, code.to_owned(), error.to_string())
        }
    }
}

fn graph_verify(request: &EngineRequest) -> EngineResponse {
    let root = match require_root(request) {
        Ok(root) => root,
        Err(response) => return response,
    };
    let core = RepoPactCore::open(root);
    let status = repopact_graph::status::status(core.repository());
    EngineResponse::success(
        request.request_id.clone(),
        ENGINE_VERSION,
        serde_json::to_value(status).unwrap_or(Value::Null),
    )
}

/// WI063 incremental-equivalence checkpoint (ROG-012, Decision 0046).
/// Reuses unchanged semantic contributions and always converges to the
/// same canonical output as `graph.build`; never hides a full rebuild
/// behind a claimed "incremental" mode (see
/// `repopact_graph::incremental::UpdateMode`).
fn graph_update(request: &EngineRequest) -> EngineResponse {
    let root = match require_root(request) {
        Ok(root) => root,
        Err(response) => return response,
    };
    let core = RepoPactCore::open(root);
    let snapshot = core.snapshot();
    match repopact_graph::incremental::update(&snapshot) {
        Ok(result) => EngineResponse::success(
            request.request_id.clone(),
            ENGINE_VERSION,
            serde_json::to_value(result).unwrap_or(Value::Null),
        ),
        Err(error) => semantic_failure(request, error.code, error.message),
    }
}

fn analyze(request: &EngineRequest) -> EngineResponse {
    let root = match require_root(request) {
        Ok(root) => root,
        Err(response) => return response,
    };
    let query = match serde_json::from_value::<AnalysisQuery>(request.params.clone()) {
        Ok(query) => query,
        Err(error) => {
            return semantic_failure(request, "analysis.invalid-parameters", error.to_string())
        }
    };
    let core = RepoPactCore::open(root);
    let snapshot = core.snapshot();
    let report: AnalysisReport = core.analyze_snapshot(&snapshot, &query);
    EngineResponse::success(
        request.request_id.clone(),
        ENGINE_VERSION,
        serde_json::to_value(report).unwrap_or(Value::Null),
    )
}

fn assurance_snapshot(request: &EngineRequest) -> EngineResponse {
    let root = match require_root(request) {
        Ok(root) => root,
        Err(response) => return response,
    };
    let Some(mapping_id) = request.params.get("mapping_id").and_then(Value::as_str) else {
        return semantic_failure(
            request,
            "assurance.invalid-parameters",
            "missing required 'mapping_id' parameter",
        );
    };
    let core = RepoPactCore::open(root);
    match core.assurance_snapshot(mapping_id) {
        Ok(snapshot) => {
            EngineResponse::success(request.request_id.clone(), ENGINE_VERSION, snapshot)
        }
        Err(error) => semantic_failure(request, "assurance.snapshot-failed", error),
    }
}

fn documentation_snapshot(request: &EngineRequest) -> EngineResponse {
    let root = match require_root(request) {
        Ok(root) => root,
        Err(response) => return response,
    };
    let Some(mapping_id) = request.params.get("mapping_id").and_then(Value::as_str) else {
        return semantic_failure(
            request,
            "documentation.invalid-parameters",
            "missing required 'mapping_id' parameter",
        );
    };
    let core = RepoPactCore::open(root);
    match core.documentation_snapshot(mapping_id) {
        Ok(snapshot) => {
            EngineResponse::success(request.request_id.clone(), ENGINE_VERSION, snapshot)
        }
        Err(error) => semantic_failure(request, "documentation.snapshot-failed", error),
    }
}

fn mutation_response(request: &EngineRequest, result: MutationResult) -> EngineResponse {
    let diagnostics = result
        .diagnostics
        .iter()
        .map(mutation_diagnostic)
        .collect::<Vec<_>>();
    let value = json!({
        "success": result.success,
        "rolled_back": result.rolled_back,
        "plan_token": result.plan_token,
        "changed_paths": result.changed_paths,
        "diagnostics": result.diagnostics,
        "replayed": result.replayed,
        "metrics": result.metrics,
    });
    let mut response = EngineResponse::success(request.request_id.clone(), ENGINE_VERSION, value);
    response.diagnostics = diagnostics;
    response
}

fn semantic_failure(
    request: &EngineRequest,
    code: impl Into<String>,
    message: impl Into<String>,
) -> EngineResponse {
    let diagnostic = ProtocolDiagnostic {
        code: code.into(),
        severity: "error".to_owned(),
        message: message.into(),
        path: None,
        record: None,
        field: None,
        related_records: Vec::new(),
        suggested_actions: Vec::new(),
    };
    let mut response = EngineResponse::success(
        request.request_id.clone(),
        ENGINE_VERSION,
        json!({"success": false, "diagnostics": [diagnostic.clone()]}),
    );
    response.diagnostics.push(diagnostic);
    response
}

/// A genuine fail-closed protocol error (`ok: false`) -- distinct from
/// [`semantic_failure`], which reports a domain-level failure inside a
/// successful envelope. Used by the query surface (Decision 0050 section
/// 7) for the two cases that must never be presented as ordinary data:
/// an unsupported durable schema major, and a durable graph that failed
/// structural validation.
pub(crate) fn semantic_failure_closed(
    request: &EngineRequest,
    code: impl Into<String>,
    message: impl Into<String>,
) -> EngineResponse {
    EngineResponse::failure(request.request_id.clone(), ENGINE_VERSION, code, message)
}

fn protocol_diagnostic(value: &Diagnostic) -> ProtocolDiagnostic {
    ProtocolDiagnostic {
        code: value.code.clone(),
        severity: match value.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        }
        .to_owned(),
        message: value.message.clone(),
        path: value.path.clone(),
        record: value.record.clone(),
        field: value.field.clone(),
        related_records: value.related_records.clone().unwrap_or_default(),
        suggested_actions: value.suggested_actions.clone().unwrap_or_default(),
    }
}

fn mutation_diagnostic(value: &repopact_mutation::MutationDiagnostic) -> ProtocolDiagnostic {
    ProtocolDiagnostic {
        code: value.code.clone(),
        severity: if value.blocking { "error" } else { "warning" }.to_owned(),
        message: value.message.clone(),
        path: value.path.clone(),
        record: None,
        field: None,
        related_records: value.related_records.clone(),
        suggested_actions: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEST_ROOT: AtomicU64 = AtomicU64::new(0);

    fn request(operation: &str) -> EngineRequest {
        EngineRequest {
            protocol: PROTOCOL.to_owned(),
            protocol_version: PROTOCOL_VERSION,
            request_id: format!("test-{}", NEXT_TEST_ROOT.fetch_add(1, Ordering::Relaxed)),
            operation: operation.to_owned(),
            root: None,
            params: Value::Object(Default::default()),
        }
    }

    fn admit_session(session: &mut EngineSession) {
        let handshake_response = handle(request("handshake"), session);
        assert!(handshake_response.ok);
        let mut admission = request("admit");
        admission.params = json!({"semantic_identity": ENGINE_SEMANTIC_IDENTITY});
        assert!(handle(admission, session).ok);
    }

    fn admitted_response(request: EngineRequest) -> EngineResponse {
        let mut session = EngineSession::default();
        admit_session(&mut session);
        handle(request, &mut session)
    }

    #[test]
    fn handshake_is_structured_and_echoes_request_identity() {
        let request = request("handshake");
        let request_id = request.request_id.clone();
        let mut session = EngineSession::default();
        let response = handle(request, &mut session);
        let mut emitted = Vec::new();
        emit(&mut emitted, response).unwrap();
        let response: EngineResponse = serde_json::from_slice(&emitted).unwrap();
        assert!(response.ok);
        assert_eq!(response.request_id, request_id);
        assert_eq!(response.engine_version, ENGINE_VERSION);
        assert_eq!(
            response.semantic_identity.as_deref(),
            Some(ENGINE_SEMANTIC_IDENTITY)
        );
        assert!(response
            .capabilities
            .unwrap()
            .operations
            .contains(&"validate".to_owned()));
    }

    #[test]
    fn protocol_major_mismatch_is_transport_failure() {
        let mut request = request("handshake");
        request.protocol_version += 1;
        let mut session = EngineSession::default();
        let response = handle(request, &mut session);
        assert!(!response.ok);
        assert_eq!(response.error.unwrap().code, "protocol.version-mismatch");
    }

    #[test]
    fn unsupported_operation_is_explicit() {
        let response = admitted_response(request("filesystem.write"));
        assert!(!response.ok);
        assert_eq!(response.error.unwrap().code, "operation.unsupported");
    }

    #[test]
    fn operations_are_refused_before_same_process_admission() {
        let root = std::env::temp_dir().join(format!(
            "repopact-engine-unadmitted-{}-{}",
            std::process::id(),
            NEXT_TEST_ROOT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let mut operation = request("dashboard.write");
        operation.root = Some(root.to_string_lossy().into_owned());
        let mut session = EngineSession::default();
        let response = handle(operation, &mut session);
        assert!(!response.ok);
        assert_eq!(response.error.unwrap().code, "engine.not-admitted");
        assert!(!root.join("audits/reports/dashboard.md").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn mismatched_admission_keeps_all_operations_before_dispatch() {
        let root = std::env::temp_dir().join(format!(
            "repopact-engine-mismatch-{}-{}",
            std::process::id(),
            NEXT_TEST_ROOT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let mut session = EngineSession::default();
        assert!(handle(request("handshake"), &mut session).ok);
        let mut admission = request("admit");
        admission.params = json!({
            "semantic_identity": "repopact-semantic-v1:sha256:0000000000000000000000000000000000000000000000000000000000000000"
        });
        let response = handle(admission, &mut session);
        assert!(!response.ok);
        assert_eq!(
            response.error.unwrap().code,
            "engine.semantic-identity-mismatch"
        );
        let mut operation = request("dashboard.write");
        operation.root = Some(root.to_string_lossy().into_owned());
        let response = handle(operation, &mut session);
        assert!(!response.ok);
        assert_eq!(response.error.unwrap().code, "engine.not-admitted");
        assert!(!root.join("audits/reports/dashboard.md").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn malformed_request_is_rejected_before_semantic_dispatch() {
        let error = serde_json::from_str::<EngineRequest>("not-json").unwrap_err();
        assert!(error.is_syntax());
    }

    #[test]
    fn invalid_repository_is_a_semantic_result() {
        let root = std::env::temp_dir().join(format!(
            "repopact-engine-test-{}-{}",
            std::process::id(),
            NEXT_TEST_ROOT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let mut request = request("validate");
        request.root = Some(root.to_string_lossy().into_owned());
        let response = admitted_response(request);
        assert!(response.ok);
        assert_eq!(response.result.unwrap()["valid"], false);
        fs::remove_dir_all(root).unwrap();
    }
}
