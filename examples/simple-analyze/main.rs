//! Simple example of using dx-scope's analyze functionality as a library.
//!
//! This example demonstrates how to programmatically analyze text for
//! known errors without using the CLI.

use dx_scope::analyze;
use dx_scope::{AnalyzeInput, AnalyzeOptions, AnalyzeStatus, AutoApprove, DenyAll};
use dx_scope::shared::prelude::ConfigOptions;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load configuration from the current directory
    println!("Loading scope configuration...");
    let config_opts = ConfigOptions::default();
    let config = config_opts.load_config().await?;

    println!("Found {} known errors", config.known_error.len());
    println!();

    // Create options for analysis
    let options = AnalyzeOptions::new(
        config.known_error.clone(),
        config.working_dir.clone(),
    );

    // Example 1: Analyze a string directly
    println!("=== Example 1: Analyzing text with known error ===");
    let log_text = r#"
Building project...
Compiling dependencies...
error: disk is full
Build failed!
    "#;

    let status = analyze::process_text(&options, log_text, &AutoApprove).await?;
    print_status("Direct text analysis", status);
    println!();

    // Example 2: Analyze lines from a vector (auto-approve fixes)
    println!("=== Example 2: Analyzing lines with auto-approve ===");
    let lines = vec![
        "Starting deployment...".to_string(),
        "Connecting to server...".to_string(),
        "error: connection timeout".to_string(),
        "Deployment failed".to_string(),
    ];

    let input = AnalyzeInput::from_lines(lines);
    let status = analyze::process_input(&options, input, &AutoApprove).await?;
    print_status("Lines with auto-approve", status);
    println!();

    // Example 3: Analyze with DenyAll (dry-run mode)
    println!("=== Example 3: Analyzing with DenyAll (no fixes) ===");
    let input = AnalyzeInput::from_lines(vec![
        "error: disk is full".to_string(),
    ]);

    let status = analyze::process_input(&options, input, &DenyAll).await?;
    print_status("With DenyAll", status);
    println!();

    // Example 4: Analyze a file
    println!("=== Example 4: Analyzing a file ===");
    // Create a temporary log file
    let log_content = "Building...\nerror: something went wrong\nDone.\n";
    tokio::fs::write("/tmp/test.log", log_content).await?;

    let input = AnalyzeInput::from_file("/tmp/test.log");
    let status = analyze::process_input(&options, input, &DenyAll).await?;
    print_status("File analysis", status);

    // Clean up
    tokio::fs::remove_file("/tmp/test.log").await?;

    Ok(())
}

fn print_status(label: &str, status: AnalyzeStatus) {
    println!("{}: {:?}", label, status);
    println!("  Exit code: {}", status.to_exit_code());

    let message = match status {
        AnalyzeStatus::NoKnownErrorsFound => "✓ No known errors detected",
        AnalyzeStatus::KnownErrorFoundNoFixFound => "⚠ Error found, but no fix available",
        AnalyzeStatus::KnownErrorFoundUserDenied => "⊘ Fix available but user declined",
        AnalyzeStatus::KnownErrorFoundFixFailed => "✗ Fix attempted but failed",
        AnalyzeStatus::KnownErrorFoundFixSucceeded => "✓ Error found and fixed!",
    };

    println!("  {}", message);
}
