use git_pipeline::*;

fn main() {
    let repo_path = std::env::args().nth(1)
        .expect("Usage: runner <repo-path>");

    println!("🔍 Git-native CI/CD Pipeline Runner");
    println!("📁 Repo: {}\n", repo_path);

    let pipeline = Pipeline::standard_pipeline();
    println!("📋 Pipeline stages:");
    for stage in pipeline.stages() {
        let deps = if stage.dependencies.is_empty() {
            "none".to_string()
        } else {
            stage.dependencies.join(", ")
        };
        println!("   • {} ({}) → deps: {}", stage.name, stage.command, deps);
    }
    println!();

    let order = pipeline.execution_order().unwrap();
    println!("🔀 Execution order: {}\n", order.join(" → "));

    let mut run = PipelineRun::new(&repo_path, pipeline);
    match run.run() {
        Ok(()) => {
            println!("\n✅ Pipeline completed\n");
        }
        Err(e) => {
            println!("\n❌ Pipeline error: {}\n", e);
        }
    }

    // Generate report
    let report = PipelineReport::new(&repo_path);
    match report.generate() {
        Ok(summary) => {
            let formatted = PipelineReport::format_summary(&summary);
            println!("{}", formatted);
        }
        Err(e) => println!("Report error: {}", e),
    }
}
