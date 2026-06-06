use serde::{Deserialize, Serialize};

/// Result of executing a single pipeline stage.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StageResult {
    pub stage: String,
    pub success: bool,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
    pub commit_sha: String,
}

/// A single pipeline stage.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Stage {
    pub name: String,
    pub command: String,
    pub dependencies: Vec<String>,
}

impl Stage {
    /// Create a new stage with no dependencies.
    pub fn new(name: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            command: command.into(),
            dependencies: Vec::new(),
        }
    }

    /// Add a dependency on another stage.
    pub fn depends_on(mut self, dep: impl Into<String>) -> Self {
        self.dependencies.push(dep.into());
        self
    }

    /// Get the branch name for this stage (e.g., "ci/build").
    pub fn branch_name(&self) -> String {
        format!("ci/{}", self.name)
    }
}
