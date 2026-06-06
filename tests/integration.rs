use std::process::Command;

use tempfile::TempDir;

use git_pipeline::*;

/// Helper to create a temp git repo for testing.
fn create_test_repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    let path = dir.path();

    Command::new("git").args(["init"]).current_dir(path).output().unwrap();
    Command::new("git").args(["config", "user.email", "test@ci.local"])
        .current_dir(path).output().unwrap();
    Command::new("git").args(["config", "user.name", "CI Test"])
        .current_dir(path).output().unwrap();

    // Create an initial commit
    std::fs::write(path.join("README.md"), "# test\n").unwrap();
    Command::new("git").args(["add", "."]).current_dir(path).output().unwrap();
    Command::new("git").args(["commit", "-m", "initial"]).current_dir(path).output().unwrap();

    dir
}

// ─── Stage Tests ───

#[test]
fn test_stage_new() {
    let s = Stage::new("build", "cargo build");
    assert_eq!(s.name, "build");
    assert_eq!(s.command, "cargo build");
    assert!(s.dependencies.is_empty());
}

#[test]
fn test_stage_with_dependencies() {
    let s = Stage::new("test", "cargo test")
        .depends_on("build")
        .depends_on("lint");
    assert_eq!(s.dependencies, vec!["build", "lint"]);
}

#[test]
fn test_stage_branch_name() {
    let s = Stage::new("build", "cargo build");
    assert_eq!(s.branch_name(), "ci/build");
}

#[test]
fn test_stage_serialization() {
    let s = Stage::new("test", "cargo test").depends_on("build");
    let json = serde_json::to_string(&s).unwrap();
    let s2: Stage = serde_json::from_str(&json).unwrap();
    assert_eq!(s, s2);
}

// ─── Pipeline Tests ───

#[test]
fn test_pipeline_new() {
    let p = Pipeline::new();
    assert!(p.is_empty());
    assert_eq!(p.len(), 0);
}

#[test]
fn test_pipeline_add_stage() {
    let mut p = Pipeline::new();
    p.add_stage(Stage::new("build", "cargo build"));
    assert_eq!(p.len(), 1);
    assert!(p.get_stage("build").is_some());
    assert!(p.get_stage("test").is_none());
}

#[test]
fn test_pipeline_validation_valid() {
    let mut p = Pipeline::new();
    p.add_stage(Stage::new("fmt", "cargo fmt -- --check"));
    p.add_stage(Stage::new("check", "cargo check").depends_on("fmt"));
    assert!(p.validate().is_ok());
}

#[test]
fn test_pipeline_validation_cycle() {
    let mut p = Pipeline::new();
    p.add_stage(Stage::new("a", "echo a").depends_on("b"));
    p.add_stage(Stage::new("b", "echo b").depends_on("a"));
    assert!(matches!(p.validate().unwrap_err(), PipelineError::CycleDetected));
}

#[test]
fn test_pipeline_validation_missing_dep() {
    let mut p = Pipeline::new();
    p.add_stage(Stage::new("test", "cargo test").depends_on("nonexistent"));
    assert!(matches!(
        p.validate().unwrap_err(),
        PipelineError::DependencyNotFound(name) if name == "nonexistent"
    ));
}

#[test]
fn test_pipeline_execution_order() {
    let mut p = Pipeline::new();
    p.add_stage(Stage::new("fmt", "echo fmt"));
    p.add_stage(Stage::new("check", "echo check").depends_on("fmt"));
    p.add_stage(Stage::new("test", "echo test").depends_on("check"));
    p.add_stage(Stage::new("publish", "echo publish").depends_on("test"));

    let order = p.execution_order().unwrap();
    assert_eq!(order.len(), 4);

    let fmt_pos = order.iter().position(|n| n == "fmt").unwrap();
    let check_pos = order.iter().position(|n| n == "check").unwrap();
    let test_pos = order.iter().position(|n| n == "test").unwrap();
    let pub_pos = order.iter().position(|n| n == "publish").unwrap();

    assert!(fmt_pos < check_pos);
    assert!(check_pos < test_pos);
    assert!(test_pos < pub_pos);
}

#[test]
fn test_pipeline_root_stages() {
    let mut p = Pipeline::new();
    p.add_stage(Stage::new("fmt", "echo fmt"));
    p.add_stage(Stage::new("check", "echo check").depends_on("fmt"));

    let roots = p.root_stages();
    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0].name, "fmt");
}

#[test]
fn test_pipeline_diamond_dependency() {
    // build → test, build → lint, test → publish, lint → publish
    let mut p = Pipeline::new();
    p.add_stage(Stage::new("build", "echo build"));
    p.add_stage(Stage::new("test", "echo test").depends_on("build"));
    p.add_stage(Stage::new("lint", "echo lint").depends_on("build"));
    p.add_stage(Stage::new("publish", "echo publish").depends_on("test").depends_on("lint"));

    assert!(p.validate().is_ok());
    let order = p.execution_order().unwrap();
    assert_eq!(order.len(), 4);

    let build_pos = order.iter().position(|n| n == "build").unwrap();
    let test_pos = order.iter().position(|n| n == "test").unwrap();
    let lint_pos = order.iter().position(|n| n == "lint").unwrap();
    let pub_pos = order.iter().position(|n| n == "publish").unwrap();

    assert!(build_pos < test_pos);
    assert!(build_pos < lint_pos);
    assert!(test_pos < pub_pos);
    assert!(lint_pos < pub_pos);
}

#[test]
fn test_standard_pipeline() {
    let p = Pipeline::standard_pipeline();
    assert_eq!(p.len(), 4);
    assert!(p.validate().is_ok());
    let order = p.execution_order().unwrap();
    assert_eq!(order[0], "fmt");
    assert_eq!(order[3], "publish-dry-run");
}

// ─── PipelineRun Tests ───

#[test]
fn test_run_verify_git_repo() {
    let repo = create_test_repo();
    let p = Pipeline::new();
    let mut run = PipelineRun::new(repo.path().to_str().unwrap(), p);
    assert!(run.verify_git_repo().is_ok());
}

#[test]
fn test_run_verify_not_git_repo() {
    let dir = TempDir::new().unwrap();
    let p = Pipeline::new();
    let mut run = PipelineRun::new(dir.path().to_str().unwrap(), p);
    assert!(matches!(run.verify_git_repo().unwrap_err(), PipelineError::GitCommandFailed { .. }));
}

#[test]
fn test_run_single_success_stage() {
    let repo = create_test_repo();
    let mut p = Pipeline::new();
    p.add_stage(Stage::new("echo-stage", "echo hello-world"));

    let mut run = PipelineRun::new(repo.path().to_str().unwrap(), p);
    let result = run.run_stage("echo-stage").unwrap();

    assert!(result.success);
    assert_eq!(result.exit_code, 0);
    assert!(result.stdout.contains("hello-world"));
    assert!(!result.commit_sha.is_empty());
}

#[test]
fn test_run_single_failing_stage() {
    let repo = create_test_repo();
    let mut p = Pipeline::new();
    p.add_stage(Stage::new("fail-stage", "exit 42"));

    let mut run = PipelineRun::new(repo.path().to_str().unwrap(), p);
    let result = run.run_stage("fail-stage").unwrap();

    assert!(!result.success);
    assert_eq!(result.exit_code, 42);
}

#[test]
fn test_run_full_pipeline_success() {
    let repo = create_test_repo();
    let mut p = Pipeline::new();
    p.add_stage(Stage::new("step1", "echo step1"));
    p.add_stage(Stage::new("step2", "echo step2").depends_on("step1"));
    p.add_stage(Stage::new("step3", "echo step3").depends_on("step2"));

    let mut run = PipelineRun::new(repo.path().to_str().unwrap(), p);
    run.run().unwrap();

    let results = run.results();
    assert_eq!(results.len(), 3);
    assert!(results.values().all(|r| r.success));
}

#[test]
fn test_run_pipeline_stops_on_failure() {
    let repo = create_test_repo();
    let mut p = Pipeline::new();
    p.add_stage(Stage::new("step1", "exit 1"));
    p.add_stage(Stage::new("step2", "echo step2").depends_on("step1"));

    let mut run = PipelineRun::new(repo.path().to_str().unwrap(), p);
    run.run().unwrap();

    let r1 = run.get_result("step1").unwrap();
    assert!(!r1.success);

    let r2 = run.get_result("step2").unwrap();
    assert!(!r2.success);
    assert!(r2.stderr.contains("skipped"));
}

#[test]
fn test_run_creates_ci_branches() {
    let repo = create_test_repo();
    let mut p = Pipeline::new();
    p.add_stage(Stage::new("hello", "echo hello"));

    let mut run = PipelineRun::new(repo.path().to_str().unwrap(), p);
    run.run().unwrap();

    // Check that ci/hello branch exists
    let output = Command::new("git")
        .args(["branch", "--list", "ci/hello"])
        .current_dir(repo.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ci/hello"));
}

// ─── Report Tests ───

#[test]
fn test_report_empty_repo() {
    let repo = create_test_repo();
    let report = PipelineReport::new(repo.path().to_str().unwrap());
    let summary = report.generate().unwrap();
    assert_eq!(summary.total_stages, 0);
    assert!(summary.overall_success); // vacuously true
}

#[test]
fn test_report_after_run() {
    let repo = create_test_repo();
    let mut p = Pipeline::new();
    p.add_stage(Stage::new("step1", "echo ok"));
    p.add_stage(Stage::new("step2", "echo ok").depends_on("step1"));

    let mut run = PipelineRun::new(repo.path().to_str().unwrap(), p);
    run.run().unwrap();

    let report = PipelineReport::new(repo.path().to_str().unwrap());
    let summary = report.generate().unwrap();
    assert_eq!(summary.total_stages, 2);
    assert_eq!(summary.passed, 2);
    assert!(summary.overall_success);

    let formatted = PipelineReport::format_summary(&summary);
    assert!(formatted.contains("✅"));
    assert!(formatted.contains("step1"));
    assert!(formatted.contains("step2"));
}

// ─── ArtifactTracker Tests ───

#[test]
fn test_artifact_store_and_get() {
    let repo = create_test_repo();
    let tracker = ArtifactTracker::new(repo.path().to_str().unwrap());

    let sha = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo.path())
        .output()
        .unwrap();
    let sha_str = String::from_utf8_lossy(&sha.stdout).trim().to_string();

    let meta = ArtifactMetadata {
        name: "v1.0-binary".to_string(),
        stage: "build".to_string(),
        commit_sha: sha_str.clone(),
        created_at: "2026-06-06T00:00:00Z".to_string(),
        size_bytes: 1024,
        description: "Test binary".to_string(),
    };

    let tag = tracker.store(&meta).unwrap();
    assert_eq!(tag, "artifact/v1.0-binary");

    let retrieved = tracker.get("v1.0-binary").unwrap().unwrap();
    assert_eq!(retrieved.name, "v1.0-binary");
    assert_eq!(retrieved.stage, "build");
    assert_eq!(retrieved.size_bytes, 1024);
}

#[test]
fn test_artifact_exists() {
    let repo = create_test_repo();
    let tracker = ArtifactTracker::new(repo.path().to_str().unwrap());

    assert!(!tracker.exists("nope").unwrap());

    let sha = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo.path())
        .output()
        .unwrap();
    let sha_str = String::from_utf8_lossy(&sha.stdout).trim().to_string();

    let meta = ArtifactMetadata {
        name: "test-artifact".to_string(),
        stage: "test".to_string(),
        commit_sha: sha_str,
        created_at: "2026-06-06T00:00:00Z".to_string(),
        size_bytes: 0,
        description: "Test".to_string(),
    };

    tracker.store(&meta).unwrap();
    assert!(tracker.exists("test-artifact").unwrap());
}

#[test]
fn test_artifact_remove() {
    let repo = create_test_repo();
    let tracker = ArtifactTracker::new(repo.path().to_str().unwrap());

    let sha = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo.path())
        .output()
        .unwrap();
    let sha_str = String::from_utf8_lossy(&sha.stdout).trim().to_string();

    let meta = ArtifactMetadata {
        name: "removeme".to_string(),
        stage: "build".to_string(),
        commit_sha: sha_str,
        created_at: "2026-06-06".to_string(),
        size_bytes: 0,
        description: "".to_string(),
    };

    tracker.store(&meta).unwrap();
    assert!(tracker.exists("removeme").unwrap());

    tracker.remove("removeme").unwrap();
    assert!(!tracker.exists("removeme").unwrap());
}

#[test]
fn test_artifact_list() {
    let repo = create_test_repo();
    let tracker = ArtifactTracker::new(repo.path().to_str().unwrap());

    let sha = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo.path())
        .output()
        .unwrap();
    let sha_str = String::from_utf8_lossy(&sha.stdout).trim().to_string();

    for i in 0..3 {
        let meta = ArtifactMetadata {
            name: format!("artifact-{}", i),
            stage: "build".to_string(),
            commit_sha: sha_str.clone(),
            created_at: "2026-06-06".to_string(),
            size_bytes: i as u64 * 100,
            description: format!("Artifact {}", i),
        };
        tracker.store(&meta).unwrap();
    }

    let list = tracker.list().unwrap();
    assert_eq!(list.len(), 3);
}

// ─── Error Tests ───

#[test]
fn test_error_display() {
    let e = PipelineError::CycleDetected;
    assert!(e.to_string().contains("cycle"));

    let e = PipelineError::DependencyNotFound("build".to_string());
    assert!(e.to_string().contains("build"));
}
