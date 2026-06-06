//! # git-pipeline
//!
//! A CI/CD pipeline engine that uses **only git primitives** — branches, commits,
//! notes, and tags — to track stage execution, results, and artifacts.
//!
//! ## Hypothesis
//!
//! > A git-only pipeline is sufficient for small projects.
//!
//! Every stage result becomes a commit on a `ci/<stage>` branch. Status is
//! tracked via git notes (JSON payload) on HEAD. Artifacts are git tags with
//! metadata.

mod stage;
mod pipeline;
mod run;
mod report;
mod artifact;
mod error;

pub use stage::Stage;
pub use pipeline::Pipeline;
pub use run::PipelineRun;
pub use report::PipelineReport;
pub use artifact::{ArtifactTracker, ArtifactMetadata};
pub use error::PipelineError;
