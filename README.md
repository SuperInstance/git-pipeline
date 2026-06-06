# git-pipeline

A CI/CD pipeline engine that uses **only git primitives** — branches, commits, notes, and tags — to track stage execution, results, and artifacts.

## Hypothesis

> A git-only pipeline is sufficient for small projects.

This library tests that hypothesis by implementing a complete CI/CD pipeline using nothing but git operations: stages produce commits on `ci/*` branches, results are stored as git notes, and artifacts become annotated tags.

## Architecture

```
Pipeline (DAG) ──► PipelineRun ──► ci/* branches + git notes
                  │
                  ├── PipelineReport ──► reads notes → summary
                  └── ArtifactTracker ──► annotated tags with metadata
```

### Core Types

| Type | Description |
|------|-------------|
| `Stage` | A pipeline stage: name, shell command, dependencies |
| `Pipeline` | DAG of stages (uses `petgraph` for topological sort + cycle detection) |
| `PipelineRun` | Executes stages in DAG order, creates `ci/<stage>` branches |
| `PipelineReport` | Reads git notes to produce a status summary |
| `ArtifactTracker` | Stores build artifacts as annotated git tags with JSON metadata |

### Git Primitives Used

- **Branches**: Each stage creates a `ci/<stage>` branch from HEAD
- **Commits**: Stage results are committed to the ci branch
- **Notes**: JSON payload (`ref=ci`) attached to stage commits with full result data
- **Tags**: Annotated tags (`artifact/<name>`) store artifact metadata

### DAG Dependencies

```
fmt → check → test → publish-dry-run
```

If a stage fails, all dependent stages are skipped with a descriptive message.

## Real Results: entropy-gpu-rs

Ran the standard 4-stage pipeline against [entropy-gpu-rs](https://github.com/SuperInstance/entropy-gpu-rs):

```
Pipeline Report: 1 stages
  ✅ Passed: 0  ❌ Failed: 1  ⏭️  Skipped: 0
  Overall: ❌ FAILURE

Stage Results:
  ❌ fmt (exit 1, 45ms)
     > Diff in src/lib.rs:33: formatting issues detected
     > (6 files with style violations)
```

The `fmt` stage caught real formatting violations — `cargo fmt -- --check` found 6 locations where entropy-gpu-rs diverged from `rustfmt` defaults. Since `fmt` failed, downstream stages (`check`, `test`, `publish-dry-run`) were correctly skipped.

The git notes on the `ci/fmt` branch contain the full diff output, recoverable at any time via `git notes --ref=ci show <sha>`.

**Conclusion**: The hypothesis holds — for a small Rust project, git primitives are sufficient to track CI results, store rich metadata, and report pipeline status. The overhead is minimal (branches + notes + tags) and the data is portable, diffable, and lives alongside the code.

## Usage

```rust
use git_pipeline::*;

// Define a pipeline
let mut p = Pipeline::new();
p.add_stage(Stage::new("fmt", "cargo fmt -- --check"));
p.add_stage(Stage::new("check", "cargo check").depends_on("fmt"));
p.add_stage(Stage::new("test", "cargo test").depends_on("check"));

// Run it against a repo
let mut run = PipelineRun::new("/path/to/repo", p);
run.run().unwrap();

// Generate a report
let report = PipelineReport::new("/path/to/repo");
let summary = report.generate().unwrap();
println!("{}", PipelineReport::format_summary(&summary));

// Store an artifact
let tracker = ArtifactTracker::new("/path/to/repo");
tracker.store(&ArtifactMetadata {
    name: "v1.0-binary".into(),
    stage: "build".into(),
    commit_sha: "abc123".into(),
    created_at: "2026-06-06".into(),
    size_bytes: 2048,
    description: "Release binary".into(),
}).unwrap();
```

### Quick Start (CLI example)

```bash
cargo run --example runner -- /path/to/your/repo
```

## Tests

27 tests covering all core functionality:

```
running 27 tests
test test_stage_new ... ok
test test_stage_with_dependencies ... ok
test test_stage_branch_name ... ok
test test_stage_serialization ... ok
test test_pipeline_new ... ok
test test_pipeline_add_stage ... ok
test test_pipeline_validation_valid ... ok
test test_pipeline_validation_cycle ... ok
test test_pipeline_validation_missing_dep ... ok
test test_pipeline_execution_order ... ok
test test_pipeline_root_stages ... ok
test test_pipeline_diamond_dependency ... ok
test test_standard_pipeline ... ok
test test_run_verify_git_repo ... ok
test test_run_verify_not_git_repo ... ok
test test_run_single_success_stage ... ok
test test_run_single_failing_stage ... ok
test test_run_full_pipeline_success ... ok
test test_run_pipeline_stops_on_failure ... ok
test test_run_creates_ci_branches ... ok
test test_report_empty_repo ... ok
test test_report_after_run ... ok
test test_artifact_store_and_get ... ok
test test_artifact_exists ... ok
test test_artifact_remove ... ok
test test_artifact_list ... ok
test test_error_display ... ok
```

## License

MIT
