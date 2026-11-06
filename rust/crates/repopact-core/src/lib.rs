use std::path::Path;

use repopact_analysis::{analyze, AnalysisQuery, AnalysisReport};
use repopact_graph::{build, RepositoryGraph};
use repopact_mutation::{
    apply, plan, ApplyOptions, MutationDiagnostic, MutationPlan, MutationRequest, MutationResult,
};
use repopact_repository::{Repository, RepositorySession, RepositorySnapshot};
use repopact_types::{RepositoryIdentity, ValidationReport};
use serde_json::Value;

/// Reusable, non-Tauri RepoPact façade. Graph, analysis, mutation, and
/// validation all start from an immutable snapshot produced by the same
/// repository session.
pub struct RepoPactCore {
    session: RepositorySession,
}

impl RepoPactCore {
    pub fn open(root: impl AsRef<Path>) -> Self {
        Self::open_repository(Repository::open(root))
    }

    pub fn open_repository(repository: Repository) -> Self {
        Self {
            session: repository.session(),
        }
    }

    pub fn repository(&self) -> &Repository {
        self.session.repository()
    }

    pub fn identity(&self) -> RepositoryIdentity {
        self.session.repository().identity()
    }

    /// Build one fresh immutable generation. Desktop/session consumers should
    /// retain this snapshot and use the `*_snapshot` projections below.
    pub fn snapshot(&self) -> RepositorySnapshot {
        self.session.snapshot()
    }

    /// Fresh-snapshot convenience for isolated callers.
    pub fn validate(&self) -> ValidationReport {
        let snapshot = self.snapshot();
        self.validate_snapshot(&snapshot)
    }

    pub fn validate_snapshot(&self, snapshot: &RepositorySnapshot) -> ValidationReport {
        repopact_validation::validate_snapshot(snapshot)
    }

    /// Fresh-snapshot convenience for isolated callers.
    pub fn assurance_snapshot(&self, mapping_id: &str) -> Result<serde_json::Value, String> {
        let snapshot = self.snapshot();
        self.assurance_snapshot_snapshot(&snapshot, mapping_id)
    }

    pub fn assurance_snapshot_snapshot(
        &self,
        snapshot: &RepositorySnapshot,
        mapping_id: &str,
    ) -> Result<serde_json::Value, String> {
        repopact_validation::compute_review_snapshot(snapshot, mapping_id)
    }

    /// Fresh-snapshot convenience for a documentation freshness mapping.
    pub fn documentation_snapshot(&self, mapping_id: &str) -> Result<serde_json::Value, String> {
        let snapshot = self.snapshot();
        self.documentation_snapshot_snapshot(&snapshot, mapping_id)
    }

    pub fn documentation_snapshot_snapshot(
        &self,
        snapshot: &RepositorySnapshot,
        mapping_id: &str,
    ) -> Result<serde_json::Value, String> {
        repopact_validation::compute_documentation_snapshot(snapshot, mapping_id)
    }

    /// Fresh-snapshot convenience for isolated callers.
    pub fn graph(&self) -> RepositoryGraph {
        self.graph_snapshot(&self.snapshot())
    }

    pub fn graph_snapshot(&self, snapshot: &RepositorySnapshot) -> RepositoryGraph {
        build(snapshot)
    }

    /// Fresh-snapshot convenience for isolated callers.
    pub fn analyze(&self, query: &AnalysisQuery) -> AnalysisReport {
        let snapshot = self.snapshot();
        self.analyze_snapshot(&snapshot, query)
    }

    pub fn analyze_snapshot(
        &self,
        snapshot: &RepositorySnapshot,
        query: &AnalysisQuery,
    ) -> AnalysisReport {
        analyze(snapshot, query)
    }

    /// Fresh-snapshot convenience for isolated callers.
    pub fn plan_mutation(&self, request: MutationRequest) -> MutationPlan {
        let snapshot = self.snapshot();
        self.plan_mutation_snapshot(&snapshot, request)
    }

    pub fn plan_mutation_snapshot(
        &self,
        snapshot: &RepositorySnapshot,
        request: MutationRequest,
    ) -> MutationPlan {
        plan(snapshot, request)
    }

    pub fn apply_mutation(&self, mutation: &MutationPlan) -> MutationResult {
        apply(mutation, &ApplyOptions::default())
    }

    pub fn apply_mutation_with_options(
        &self,
        mutation: &MutationPlan,
        options: &ApplyOptions,
    ) -> MutationResult {
        apply(mutation, options)
    }

    /// Plan and apply under the shared repository lock with a durable request
    /// receipt. Proposal admission remains on its operation-scoped path.
    pub fn plan_and_apply_idempotently(
        &self,
        request_key: &str,
        operation: &str,
        params: &Value,
        request: MutationRequest,
    ) -> MutationResult {
        repopact_mutation::plan_and_apply_idempotently(
            self.repository().root(),
            request_key,
            operation,
            params,
            request,
        )
    }

    pub fn preview_mutation_transactionally(
        &self,
        request: MutationRequest,
    ) -> Result<MutationPlan, MutationDiagnostic> {
        repopact_mutation::preview_mutation_transactionally(self.repository().root(), request)
    }

    pub fn request_receipt(
        &self,
        request_key: &str,
    ) -> Result<Option<MutationResult>, MutationDiagnostic> {
        repopact_mutation::read_request_receipt(self.repository().root(), request_key)
    }
}

pub fn validate(root: impl AsRef<Path>) -> ValidationReport {
    RepoPactCore::open(root).validate()
}
