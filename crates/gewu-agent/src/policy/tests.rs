use std::path::Path;

use agent_client_protocol::v1::schema::{ToolCallLocation, ToolCallUpdateFields, ToolKind};
use serde_json::json;

use super::Policy;

fn policy() -> (tempfile::TempDir, Policy) {
    let root = tempfile::tempdir().expect("temporary project");
    std::fs::create_dir_all(root.path().join(".gewu/harness")).expect("harness directory");
    let policy = Policy::new(root.path(), Path::new("/opt/gewu/bin/gewu"));
    (root, policy)
}

#[test]
fn reads_inside_the_project_are_allowed() {
    let (root, policy) = policy();
    assert!(policy.check_read(&root.path().join("src/lib.rs")).is_ok());
    assert!(policy.check_read(Path::new("README.md")).is_ok());
}

#[test]
fn reads_that_escape_the_project_are_refused() {
    let (root, policy) = policy();
    assert!(policy.check_read(&root.path().join("../secret")).is_err());
    assert!(policy.check_read(Path::new("/etc/passwd")).is_err());
}

#[test]
fn writes_land_only_under_the_harness_directory() {
    let (root, policy) = policy();
    assert!(
        policy
            .check_write(&root.path().join(".gewu/harness/plan.json"))
            .is_ok()
    );
    assert!(
        policy
            .check_write(&root.path().join(".gewu/review-spec.json"))
            .is_err()
    );
    assert!(policy.check_write(&root.path().join("src/lib.rs")).is_err());
    assert!(
        policy
            .check_write(&root.path().join(".gewu/harness/../../src/lib.rs"))
            .is_err()
    );
}

#[test]
fn only_harness_and_read_only_review_commands_run() {
    let (_root, policy) = policy();
    let args = |words: &[&str]| words.iter().map(|w| (*w).to_owned()).collect::<Vec<_>>();
    assert!(
        policy
            .check_command("gewu", &args(&["harness", "next"]))
            .is_ok()
    );
    assert!(
        policy
            .check_command("/opt/gewu/bin/gewu", &args(&["review", "validate"]))
            .is_ok()
    );
    assert!(
        policy
            .check_command("gewu", &args(&["review", "generate"]))
            .is_err()
    );
    assert!(
        policy
            .check_command("gewu", &args(&["review", "open"]))
            .is_err()
    );
    assert!(policy.check_command("rm", &args(&["-rf", "."])).is_err());
}

#[test]
fn shell_lines_cannot_chain_other_commands() {
    let (root, policy) = policy();
    assert!(policy.check_shell("gewu harness status").is_ok());
    let cd = format!("cd {} && gewu harness next", root.path().display());
    assert!(policy.check_shell(&cd).is_ok());
    assert!(policy.check_shell("gewu harness next; rm -rf .").is_err());
    assert!(policy.check_shell("gewu harness next && rm -rf .").is_err());
    assert!(policy.check_shell("cd /tmp && gewu harness next").is_err());
    assert!(policy.check_shell("gewu harness $(rm -rf .)").is_err());
}

#[test]
fn tool_permissions_follow_the_same_rules() {
    let (root, policy) = policy();
    let edit = |path: &Path| {
        ToolCallUpdateFields::new()
            .kind(ToolKind::Edit)
            .locations(vec![ToolCallLocation::new(path)])
    };
    assert!(
        policy
            .check_tool(&edit(&root.path().join(".gewu/harness/a.json")))
            .is_ok()
    );
    assert!(policy.check_tool(&edit(&root.path().join("a.rs"))).is_err());
    let run = |line: &str| {
        ToolCallUpdateFields::new()
            .kind(ToolKind::Execute)
            .raw_input(json!({ "command": line }))
    };
    assert!(policy.check_tool(&run("gewu harness status")).is_ok());
    assert!(policy.check_tool(&run("cargo build")).is_err());
    assert!(
        policy
            .check_tool(&ToolCallUpdateFields::new().kind(ToolKind::Fetch))
            .is_err()
    );
    assert!(
        policy
            .check_tool(&ToolCallUpdateFields::new().kind(ToolKind::Edit))
            .is_err()
    );
}
