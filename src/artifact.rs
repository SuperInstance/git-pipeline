use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::error::PipelineError;

/// Metadata stored with an artifact tag.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArtifactMetadata {
    pub name: String,
    pub stage: String,
    pub commit_sha: String,
    pub created_at: String,
    pub size_bytes: u64,
    pub description: String,
}

/// Tracks build artifacts as git tags with metadata.
pub struct ArtifactTracker {
    repo_path: String,
}

impl ArtifactTracker {
    /// Create a new artifact tracker for the given repo.
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

    /// Store an artifact as a git tag with metadata.
    pub fn store(&self, metadata: &ArtifactMetadata) -> Result<String, PipelineError> {
        let tag_name = format!("artifact/{}", metadata.name);
        let message = serde_json::to_string_pretty(metadata)?;

        // Create an annotated tag with the metadata as the message
        self.git(&[
            "tag", "-a", &tag_name,
            &metadata.commit_sha,
            "-m", &message,
        ])?;

        Ok(tag_name)
    }

    /// Retrieve artifact metadata from a tag.
    pub fn get(&self, name: &str) -> Result<Option<ArtifactMetadata>, PipelineError> {
        let tag_name = format!("artifact/{}", name);
        let message = self.git(&["tag", "-l", &tag_name, "--format=%(contents)"]);

        match message {
            Ok(json) if !json.is_empty() => {
                let meta: ArtifactMetadata = serde_json::from_str(&json)?;
                Ok(Some(meta))
            }
            _ => Ok(None),
        }
    }

    /// List all artifacts.
    pub fn list(&self) -> Result<Vec<ArtifactMetadata>, PipelineError> {
        let output = self.git(&[
            "tag", "-l", "artifact/*", "--format=%(contents)"
        ])?;

        let mut artifacts = Vec::new();
        for chunk in output.split("\n\n") {
            if let Ok(meta) = serde_json::from_str::<ArtifactMetadata>(chunk.trim()) {
                artifacts.push(meta);
            }
        }

        Ok(artifacts)
    }

    /// Remove an artifact tag.
    pub fn remove(&self, name: &str) -> Result<(), PipelineError> {
        let tag_name = format!("artifact/{}", name);
        self.git(&["tag", "-d", &tag_name])?;
        Ok(())
    }

    /// Check if an artifact exists.
    pub fn exists(&self, name: &str) -> Result<bool, PipelineError> {
        let tag_name = format!("artifact/{}", name);
        let output = self.git(&["tag", "-l", &tag_name])?;
        Ok(!output.is_empty())
    }
}
