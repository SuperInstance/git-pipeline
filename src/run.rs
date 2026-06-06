use std::collections::HashMap;
use std::process::Command;
use std::time::Instant;

use crate::error::PipelineError;
use crate::pipeline::Pipeline;
use crate::stage::{Stage, StageResult};

/// Executes a pipeline against a git repository.
pub struct PipelineRun {
    repo_path: String,
    pipeline: Pipeline,
    results: HashMap<String, StageResult>,
}

impl PipelineRun {
    /// Create a new pipeline run for the given repo.
    pub fn new(repo_path: impl Into<String>, pipeline: Pipeline) -> Self {
        Self {
            repo_path: repo_path.into(),
            pipeline,
            results: HashMap::new(),
        }
    }

    /// Get the results of the run.
    pub fn results(&self) -> &HashMap<String, StageResult> {
        &self.results
    }

    /// Get a specific stage result.
    pub fn get_result(&self, stage: &str) -> Option<&StageResult> {
        self.results.get(stage)
    }

    /// Run a git command in the repo directory.
    fn git(&self, args: &[&str]) -> Result<String, PipelineError> {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| PipelineError::Io(e))?;

        if !output.status.success() {
            let code = output.status.code().unwrap_or(1);
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(PipelineError::GitCommandFailed {
                command: args.join(" "),
                exit_code: code,
                stderr,
            });
        }

        Ok(String::from_utf8(output.stdout)?.trim().to_string())
    }

    /// Verify the path is a git repo.
    pub fn verify_git_repo(&self) -> Result<(), PipelineError> {
        self.git(&["rev-parse", "--git-dir"])?;
        Ok(())
    }

    /// Get the current HEAD commit SHA.
    fn head_sha(&self) -> Result<String, PipelineError> {
        self.git(&["rev-parse", "HEAD"])
    }

    /// Execute a single stage and record its result as a git commit on a branch.
    fn execute_stage(&mut self, stage: &Stage) -> Result<StageResult, PipelineError> {
        let branch = stage.branch_name();

        // Create the ci/ branch from HEAD
        let head = self.head_sha()?;
        self.git(&["branch", "-f", &branch, &head])?;

        // Switch to the ci branch
        self.git(&["checkout", &branch])?;

        let start = Instant::now();
        let output = Command::new("sh")
            .arg("-c")
            .arg(&stage.command)
            .current_dir(&self.repo_path)
            .env("CI_STAGE", &stage.name)
            .env("CI_PIPELINE", "git-pipeline")
            .output()
            .map_err(|e| PipelineError::Io(e))?;
        let duration_ms = start.elapsed().as_millis() as u64;

        let success = output.status.success();
        let exit_code = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        // Commit the result as a note-like file commit
        let result = StageResult {
            stage: stage.name.clone(),
            success,
            exit_code,
            stdout: if stdout.len() > 10000 { stdout[..10000].to_string() } else { stdout },
            stderr: if stderr.len() > 10000 { stderr[..10000].to_string() } else { stderr },
            duration_ms,
            commit_sha: head.clone(),
        };

        // Create a commit on the branch noting the result
        // Write a small result file and commit it
        let result_file = format!(".ci-result-{}", stage.name);
        let result_json = serde_json::to_string_pretty(&result)?;
        std::fs::write(
            format!("{}/{}", self.repo_path, result_file),
            &result_json,
        )?;

        self.git(&["add", &result_file])?;
        let commit_msg = format!("ci: {} {} (exit {})",
            stage.name,
            if success { "PASSED" } else { "FAILED" },
            exit_code,
        );
        // git commit may fail if nothing changed; that's ok
        let _ = self.git(&["commit", "-m", &commit_msg]);

        // Get the new commit SHA
        let branch_sha = self.git(&["rev-parse", "HEAD"])?;

        // Add git note with the result JSON
        let _ = self.git(&["notes", "--ref=ci", "add", "-f", "-m", &result_json, &branch_sha]);

        // Switch back to the original branch
        self.git(&["checkout", "-"])?;

        let mut result = result;
        result.commit_sha = branch_sha;

        // Clean up the result file
        let _ = std::fs::remove_file(format!("{}/{}", self.repo_path, result_file));

        Ok(result)
    }

    /// Run the entire pipeline, respecting the DAG order.
    pub fn run(&mut self) -> Result<(), PipelineError> {
        self.verify_git_repo()?;
        self.pipeline.validate()?;

        let order = self.pipeline.execution_order()?;

        // Save current branch to restore later
        let original_branch = self.git(&["rev-parse", "--abbrev-ref", "HEAD"])
            .unwrap_or_else(|_| "main".to_string());

        for stage_name in &order {
            let stage = self.pipeline.get_stage(stage_name)
                .ok_or_else(|| PipelineError::DependencyNotFound(stage_name.clone()))?
                .clone();

            // Check dependencies succeeded
            for dep in &stage.dependencies {
                let dep_result = self.results.get(dep)
                    .ok_or_else(|| PipelineError::DependencyNotFound(dep.clone()))?;
                if !dep_result.success {
                    // Dependency failed — skip this stage
                    self.results.insert(stage_name.clone(), StageResult {
                        stage: stage_name.clone(),
                        success: false,
                        exit_code: -1,
                        stdout: String::new(),
                        stderr: format!("skipped: dependency '{}' failed", dep),
                        duration_ms: 0,
                        commit_sha: String::new(),
                    });
                    break;
                }
            }

            // If a dependency failed, we already inserted a skipped result
            if self.results.contains_key(stage_name) {
                continue;
            }

            let result = self.execute_stage(&stage)?;
            self.results.insert(stage_name.clone(), result);
        }

        // Restore original branch
        let _ = self.git(&["checkout", &original_branch]);

        Ok(())
    }

    /// Run a single stage by name (without full pipeline execution).
    pub fn run_stage(&mut self, stage_name: &str) -> Result<StageResult, PipelineError> {
        self.verify_git_repo()?;

        let stage = self.pipeline.get_stage(stage_name)
            .ok_or_else(|| PipelineError::DependencyNotFound(stage_name.to_string()))?
            .clone();

        let original_branch = self.git(&["rev-parse", "--abbrev-ref", "HEAD"])
            .unwrap_or_else(|_| "main".to_string());

        let result = self.execute_stage(&stage)?;
        self.results.insert(stage_name.to_string(), result.clone());

        let _ = self.git(&["checkout", &original_branch]);

        Ok(result)
    }
}
