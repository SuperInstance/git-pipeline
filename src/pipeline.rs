use petgraph::graph::DiGraph;
use petgraph::algo::toposort;
use petgraph::graph::NodeIndex;
use std::collections::HashMap;

use crate::error::PipelineError;
use crate::stage::Stage;

/// A pipeline defined as a DAG of stages.
#[derive(Debug, Clone)]
pub struct Pipeline {
    stages: HashMap<String, Stage>,
}

impl Pipeline {
    /// Create an empty pipeline.
    pub fn new() -> Self {
        Self {
            stages: HashMap::new(),
        }
    }

    /// Add a stage to the pipeline.
    pub fn add_stage(&mut self, stage: Stage) {
        self.stages.insert(stage.name.clone(), stage);
    }

    /// Get a stage by name.
    pub fn get_stage(&self, name: &str) -> Option<&Stage> {
        self.stages.get(name)
    }

    /// Get all stage names.
    pub fn stage_names(&self) -> Vec<String> {
        self.stages.keys().cloned().collect()
    }

    /// Get all stages.
    pub fn stages(&self) -> Vec<&Stage> {
        self.stages.values().collect()
    }

    /// Number of stages.
    pub fn len(&self) -> usize {
        self.stages.len()
    }

    /// Whether the pipeline is empty.
    pub fn is_empty(&self) -> bool {
        self.stages.is_empty()
    }

    /// Validate the pipeline: check dependencies exist and no cycles.
    pub fn validate(&self) -> Result<(), PipelineError> {
        // Build the DAG
        let mut graph: DiGraph<String, ()> = DiGraph::new();
        let mut node_map: HashMap<String, NodeIndex> = HashMap::new();

        for name in self.stages.keys() {
            let idx = graph.add_node(name.clone());
            node_map.insert(name.clone(), idx);
        }

        for stage in self.stages.values() {
            let stage_idx = node_map[&stage.name];
            for dep in &stage.dependencies {
                let dep_idx = node_map.get(dep)
                    .ok_or_else(|| PipelineError::DependencyNotFound(dep.clone()))?;
                graph.add_edge(*dep_idx, stage_idx, ());
            }
        }

        // Check for cycles using topological sort
        match toposort(&graph, None) {
            Ok(_) => Ok(()),
            Err(_) => Err(PipelineError::CycleDetected),
        }
    }

    /// Get the execution order (topological sort) of stages.
    pub fn execution_order(&self) -> Result<Vec<String>, PipelineError> {
        self.validate()?;

        let mut graph: DiGraph<String, ()> = DiGraph::new();
        let mut node_map: HashMap<String, NodeIndex> = HashMap::new();

        for name in self.stages.keys() {
            let idx = graph.add_node(name.clone());
            node_map.insert(name.clone(), idx);
        }

        for stage in self.stages.values() {
            let stage_idx = node_map[&stage.name];
            for dep in &stage.dependencies {
                let dep_idx = node_map[dep];
                graph.add_edge(dep_idx, stage_idx, ());
            }
        }

        let order = toposort(&graph, None)
            .map_err(|_| PipelineError::CycleDetected)?;

        Ok(order.into_iter().map(|idx| graph[idx].clone()).collect())
    }

    /// Get stages that have no dependencies (root stages).
    pub fn root_stages(&self) -> Vec<&Stage> {
        self.stages.values()
            .filter(|s| s.dependencies.is_empty())
            .collect()
    }

    /// Build a standard 4-stage pipeline: fmt → check → test → publish-dry-run
    pub fn standard_pipeline() -> Self {
        let mut p = Pipeline::new();
        p.add_stage(Stage::new("fmt", "cargo fmt -- --check"));
        p.add_stage(Stage::new("check", "cargo check").depends_on("fmt"));
        p.add_stage(Stage::new("test", "cargo test").depends_on("check"));
        p.add_stage(Stage::new("publish-dry-run", "cargo publish --dry-run").depends_on("test"));
        p
    }
}

impl Default for Pipeline {
    fn default() -> Self {
        Self::new()
    }
}
