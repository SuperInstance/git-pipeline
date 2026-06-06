use std::process::Command;

use crate::error::PipelineError;
use crate::stage::StageResult;

/// Reads pipeline results from git notes and produces a status summary.
pub struct PipelineReport {
    repo_path: String,
}

/// Summary of a pipeline run.
#[derive(Debug, Clone)]
pub struct PipelineSummary {
    pub total_stages: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub results: Vec<StageResult>,
    pub overall_success: bool,
}

impl PipelineReport {
    /// Create a new report reader for the given repo.
    pub fn new(repo_path: impl Into<String>) -> Self {
        Self { repo_path: repo_path.into() }
    }

    /// Run a git command.
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

    /// Read the git note from a specific commit on the ci/ notes ref.
    pub fn read_note(&self, commit_sha: &str) -> Result<Option<StageResult>, PipelineError> {
        let note = self.git(&["notes", "--ref=ci", "show", commit_sha]);
        match note {
            Ok(json) => {
                let result: StageResult = serde_json::from_str(&json)?;
                Ok(Some(result))
            }
            Err(_) => Ok(None),
        }
    }

    /// Read all ci/ notes from a set of commit SHAs.
    pub fn read_all_notes(&self, commits: &[(String, String)]) -> Result<Vec<StageResult>, PipelineError> {
        let mut results = Vec::new();
        for (stage_name, commit_sha) in commits {
            if let Some(result) = self.read_note(commit_sha)? {
                results.push(result);
            } else {
                // No note found — treat as unknown/skipped
                results.push(StageResult {
                    stage: stage_name.clone(),
                    success: false,
                    exit_code: -1,
                    stdout: String::new(),
                    stderr: "no note found".to_string(),
                    duration_ms: 0,
                    commit_sha: commit_sha.clone(),
                });
            }
        }
        Ok(results)
    }

    /// List all ci/* branches and their HEAD commits.
    pub fn list_ci_branches(&self) -> Result<Vec<(String, String)>, PipelineError> {
        let output = self.git(&["branch", "--list", "ci/*", "--format=%(refname:short) %(objectname:short)"])?;
        let mut branches = Vec::new();
        for line in output.lines() {
            let parts: Vec<&str> = line.trim().splitn(2, ' ').collect();
            if parts.len() == 2 {
                let branch = parts[0].trim_start_matches("ci/").to_string();
                let sha = parts[1].to_string();
                branches.push((branch, sha));
            }
        }
        Ok(branches)
    }

    /// Generate a summary report from the ci branches.
    pub fn generate(&self) -> Result<PipelineSummary, PipelineError> {
        let branches = self.list_ci_branches()?;
        let results = self.read_all_notes(&branches)?;

        let passed = results.iter().filter(|r| r.success).count();
        let failed = results.iter().filter(|r| !r.success && r.exit_code != -1).count();
        let skipped = results.iter().filter(|r| !r.success && r.exit_code == -1).count();

        Ok(PipelineSummary {
            total_stages: results.len(),
            passed,
            failed,
            skipped,
            overall_success: results.iter().all(|r| r.success),
            results,
        })
    }

    /// Format a summary as a human-readable string.
    pub fn format_summary(summary: &PipelineSummary) -> String {
        let mut out = String::new();
        out.push_str(&format!("Pipeline Report: {} stages\n", summary.total_stages));
        out.push_str(&format!("  ✅ Passed: {}  ❌ Failed: {}  ⏭️  Skipped: {}\n",
            summary.passed, summary.failed, summary.skipped));
        out.push_str(&format!("  Overall: {}\n",
            if summary.overall_success { "✅ SUCCESS" } else { "❌ FAILURE" }));
        out.push_str("\nStage Results:\n");

        for r in &summary.results {
            let icon = if r.success { "✅" } else if r.exit_code == -1 { "⏭️ " } else { "❌" };
            out.push_str(&format!("  {} {} (exit {}, {}ms)\n",
                icon, r.stage, r.exit_code, r.duration_ms));
            if !r.stdout.is_empty() {
                for line in r.stdout.lines().take(5) {
                    out.push_str(&format!("     > {}\n", line));
                }
            }
            if !r.success && !r.stderr.is_empty() {
                for line in r.stderr.lines().take(5) {
                    out.push_str(&format!("     ! {}\n", line));
                }
            }
        }

        out
    }
}
