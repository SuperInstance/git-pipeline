use std::fmt;

/// Errors that can occur during pipeline operations.
#[derive(Debug)]
pub enum PipelineError {
    /// A git command failed.
    GitCommandFailed { command: String, exit_code: i32, stderr: String },
    /// UTF-8 conversion failed.
    Utf8Error(std::str::Utf8Error),
    /// JSON serialization/deserialization failed.
    JsonError(serde_json::Error),
    /// A stage dependency was not found.
    DependencyNotFound(String),
    /// Cycle detected in stage dependencies.
    CycleDetected,
    /// Working directory is not a git repo.
    NotAGitRepo(String),
    /// A stage has already been executed.
    StageAlreadyRun(String),
    /// IO error.
    Io(std::io::Error),
}

impl fmt::Display for PipelineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GitCommandFailed { command, exit_code, stderr } => {
                write!(f, "git command '{}' failed (exit {}): {}", command, exit_code, stderr.trim())
            }
            Self::Utf8Error(e) => write!(f, "UTF-8 error: {}", e),
            Self::JsonError(e) => write!(f, "JSON error: {}", e),
            Self::DependencyNotFound(name) => write!(f, "dependency '{}' not found", name),
            Self::CycleDetected => write!(f, "cycle detected in stage dependencies"),
            Self::NotAGitRepo(path) => write!(f, "not a git repo: {}", path),
            Self::StageAlreadyRun(name) => write!(f, "stage '{}' already run", name),
            Self::Io(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for PipelineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Utf8Error(e) => Some(e),
            Self::JsonError(e) => Some(e),
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::str::Utf8Error> for PipelineError {
    fn from(e: std::str::Utf8Error) -> Self { Self::Utf8Error(e) }
}

impl From<std::string::FromUtf8Error> for PipelineError {
    fn from(e: std::string::FromUtf8Error) -> Self {
        Self::Utf8Error(e.utf8_error())
    }
}

impl From<serde_json::Error> for PipelineError {
    fn from(e: serde_json::Error) -> Self { Self::JsonError(e) }
}

impl From<std::io::Error> for PipelineError {
    fn from(e: std::io::Error) -> Self { Self::Io(e) }
}
