use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use repopact_mutation::{
    apply, plan, ApplyOptions, CreateWorkItem, MutationRequest, PathOperation,
};
use repopact_repository::Repository;

fn temp_root() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("repopact-mutation-wi046-{suffix}"));
    fs::create_dir_all(root.join("governance")).expect("governance directory");
    root
}

#[test]
fn mutation_post_validation_includes_shared_verification_contract_diagnostics() {
    let root = temp_root();
    fs::write(
        root.join("governance/verification.json"),
        r#"{
          "$schema":"../schemas/verification-profile.schema.json",
          "version":1,
          "default_profile":"missing",
          "execution_policy":{"local_primary":true,"hosted_ci_default":false,"hosted_cd_default":false},
          "profiles":{"quick":{"description":"quick","coverage":"host","steps":[{"id":"validate","argv":["{repopact}","validate"]}]}}
        }"#,
    )
    .expect("verification contract");

    let repository = Repository::open(&root);
    let snapshot = repository.session().snapshot();
    let mut plan = plan(
        &snapshot,
        MutationRequest::CreateWorkItem(
            CreateWorkItem::new("Parity probe", "2026-09-12")
                .confirm_preflight_before_work_started(),
        ),
    );
    assert_eq!(plan.file_operations.len(), 2);
    plan.file_operations.push(PathOperation::Write {
        path: "audits/reports/dashboard.md".to_owned(),
        content: "post-validation probe\n".to_owned(),
    });
    // Isolate post-apply canonical validation from the planner diagnostics:
    // the malformed verification contract above is the fact this test checks.
    plan.diagnostics.clear();

    let result = apply(&plan, &ApplyOptions::default());
    assert!(!result.success);
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("default_profile \"missing\" does not name a declared verification profile")
    }));

    fs::remove_dir_all(root).expect("cleanup");
}
