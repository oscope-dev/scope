# Scope Architecture: Current State & Library-First Refactoring

## Executive Summary

This document is driven by a single goal: enable Scope’s analyze and fix capabilities to be used programmatically from other Rust programs. We outline how to expose clear, stable public functions in the `analyze` and `doctor` modules, move CLI-only code into `cli`, and keep binaries as thin wrappers. The result is a library-first crate that external tools can depend on, while `scope` remains a CLI built on those same APIs.

---

## Current Architecture

### Overview

Scope is currently structured with a **hybrid approach** where:
- The library (`src/lib.rs`) provides module organization and a prelude pattern
- **Business logic and CLI handling are tightly coupled** within the binary and submodules
- The binary (`src/bin/scope.rs`) contains significant command routing and execution logic
- Domain logic lives in feature modules but with CLI dependencies

### Current Structure

```
src/
├── lib.rs                          # Library root - minimal, mainly exports modules
├── bin/
│   ├── scope.rs                    # Main binary - contains CLI parsing, command routing, and main entry
│   └── scope-intercept.rs          # Secondary binary - separate intercept tool
├── doctor/                         # Doctor feature domain
│   ├── mod.rs                      # Module exports
│   ├── cli.rs                      # CLI argument definitions AND routing logic
│   ├── commands/                   # Command implementations
│   └── runner.rs, check.rs, etc.   # Domain logic
├── analyze/                        # Analyze feature domain
│   ├── mod.rs
│   ├── cli.rs                      # CLI arguments AND routing
│   └── error.rs
├── report/                         # Report feature domain
│   ├── mod.rs
│   ├── cli.rs                      # CLI arguments AND command logic
│   └── ...
├── lint/                           # Lint feature domain
│   ├── mod.rs                      # Contains both CLI and commands
│   └── ...
├── shared/                         # Shared utilities (mix of public/internal)
│   ├── capture.rs                  # Output capturing (should be public)
│   ├── config_load.rs              # Configuration loading (should be public)
│   ├── logging.rs                  # Logging setup (CLI-only, should move)
│   ├── report.rs                   # Report builders (should be public)
│   └── analyze/                    # Shared analyze logic
└── models/                         # Data models
    ├── core.rs
    └── v1alpha/                    # Versioned schemas
```

### Key Components

#### 1. **Library (`src/lib.rs`)**
**Current State:**
- Exports modules (`doctor`, `analyze`, `report`, `lint`, `shared`, `models`)
- Provides a `prelude` that re-exports common types from each module
- `shared` contains a mix of public utilities and CLI-only code (not well separated)
- Defines utility macros like `report_stdout!`
- **Does NOT** expose high-level API functions directly

**Limitations:**
- Cannot use the library independently without importing from preludes
- No clear entry points for programmatic use
- Library users must know internal module structure

#### 2. **Main Binary (`src/bin/scope.rs`)**
**Current State:**
- 232 lines containing:
  - CLI definition with `clap` (`Cli`, `Command` enum, argument structs)
  - `main()` function with setup (panic handler, env loading, logger configuration)
  - `run_subcommand()` - command routing logic
  - `handle_commands()` - command dispatch to feature modules
  - `exec_sub_command()` - external subcommand execution
  - `show_config()`, `print_commands()`, `print_version()` - utility commands
  
**Issues:**
- Business logic mixed with CLI orchestration
- Cannot reuse main application flow without running the binary
- Hard to test command routing independently
- Version printing, config display logic in binary

#### 3. **Feature Modules (doctor/analyze/report/lint)**
**Current State:**
Each module follows a similar pattern:
- `cli.rs`: Contains both `Args` structs AND routing functions (e.g., `doctor_root()`, `analyze_root()`)
- `commands/` or inline: Actual implementation logic
- Exports both CLI types and routing functions via `prelude`

**Issues:**
- CLI argument types coupled to domain logic
- `*_root()` functions take `&FoundConfig` and `Args` - CLI-specific types
- Cannot call domain logic without constructing CLI args
- Business logic buried in CLI routing layer

Example from `doctor/cli.rs`:
```rust
pub async fn doctor_root(found_config: &FoundConfig, args: &DoctorArgs) -> Result<i32> {
    match &args.command {
        DoctorCommands::List(args) => doctor_list(found_config, args).await.map(|_| 0),
        DoctorCommands::Run(args) => doctor_run(found_config, args).await,
        DoctorCommands::Init(args) => doctor_init(found_config, args).await.map(|_| 0),
    }
}
```

#### 4. **Intercept Binary (`src/bin/scope-intercept.rs`)**
**Current State:**
- 145 lines - separate executable for command interception
- Contains its own CLI parsing, config loading, and execution logic
- Duplicates some patterns from main binary

#### 5. **Shared Module**
**Current State:**
- Contains reusable utilities: config loading, output capture, logging, reports
- `FoundConfig` - central configuration type
- **Mixed concerns**: combines public utilities (capture, config) with CLI-only code (logging)
- Some components already usable independently, others are CLI-specific

**Issues:**
- Unclear what's public vs internal
- Logging setup is CLI-specific but lives in a "shared" module
- Name "shared" doesn't communicate intent

---

## Problems with Current Architecture

### 1. **Limited Reusability**
- Cannot use Scope functionality as a library in other Rust projects
- CLI args required even for programmatic use
- No clean API boundary

### 2. **Testing Challenges**
- Hard to unit test command routing without CLI parsing
- Integration tests must go through CLI layer
- Mocking is difficult due to tight coupling

### 3. **Code Duplication**
- Binary setup code repeated in both binaries
- CLI routing logic can't be reused
- Version info, config display logic in binary

### 4. **Unclear API Surface**
- Prelude pattern hides what's actually public
- No clear distinction between "library API" and "CLI internals"
- Users must dig through modules to find functionality

### 5. **Maintenance Burden**
- Changes to business logic often require CLI changes
- Refactoring is risky due to tight coupling
- Hard to add new interfaces (e.g., RPC, HTTP server)

### Mixed UX/Library Concerns (Current Findings)

The following places mix human-facing output or CLI interaction with library/business logic. These should return structured data and leave all printing/formatting to the CLI:

- Shared printing from library:
    - [src/shared/mod.rs#L53](src/shared/mod.rs#L53) and [src/shared/mod.rs#L74](src/shared/mod.rs#L74): uses `report_stdout!()` and colorized output inside library helper; move to CLI, return data.

- Doctor run/list formatting and interaction:
    - [src/doctor/commands/run.rs#L111](src/doctor/commands/run.rs#L111), [src/doctor/commands/run.rs#L122](src/doctor/commands/run.rs#L122): printing summary and blank lines; move to CLI formatting.
    - [src/doctor/runner.rs#L11](src/doctor/runner.rs#L11), [src/doctor/runner.rs#L368](src/doctor/runner.rs#L368), [src/doctor/runner.rs#L370](src/doctor/runner.rs#L370), [src/doctor/runner.rs#L451](src/doctor/runner.rs#L451): colorization and interactive prints; model as options (e.g., `auto_fix`, `ci_mode`) and return results; CLI handles prompts/formatting.

- Models printing:
    - [src/models/mod.rs#L141](src/models/mod.rs#L141), [src/models/mod.rs#L143](src/models/mod.rs#L143): direct `println!`; return pretty-printed strings or values; CLI prints.

- Logging setup:
    - [src/shared/logging.rs#L325](src/shared/logging.rs#L325): `println!` on telemetry failure; move logging setup to `cli/logging.rs` and use `tracing` or CLI messages there.

- Capture formatting:
    - [src/shared/capture.rs#L4](src/shared/capture.rs#L4): `colored::Colorize` import suggests formatting concerns; ensure capture remains pure data and remove colorization from library.

Guideline: library code may use `tracing` for debug logs; user-facing output belongs in CLI. Macros like `report_stdout!` should not be used in library functions.

---

## Proposed Library-First Architecture

### Design Principles

1. **Separation of Concerns**: CLI layer separate from business logic
2. **Thin Binaries**: Binaries only handle argument parsing and call library functions
3. **Clean API**: Library exposes clear, well-documented public functions
4. **Type Independence**: Core domain types don't depend on CLI types
5. **Testability**: Business logic testable without CLI layer
6. **Explicit Module Boundaries**: Eliminate prelude pattern in favor of clear, explicit module exports
7. **No Console Output in Library**: Library functions must return structured data; all human-facing printing and formatting lives in the CLI.

### Proposed Structure

```
src/
├── lib.rs                          # Library root - exposes public modules directly
│
├── bin/
│   ├── scope.rs                    # THIN wrapper - just CLI parsing + library calls
│   └── scope-intercept.rs          # THIN wrapper
│
├── cli/                            # NEW: CLI-specific code (NOT exported in lib.rs)
│   ├── mod.rs                      # CLI utilities
│   ├── args.rs                     # Clap argument definitions
│   ├── commands.rs                 # CLI command routing
│   ├── output.rs                   # CLI output formatting
│   └── logging.rs                  # Logging setup (moved from shared, CLI-only)
│
├── doctor/                         # Doctor feature - PUBLIC library API
│   ├── mod.rs                      # Exports public functions: run(), list(), etc.
│   ├── options.rs                  # Public option types (no clap dependency)
│   ├── runner.rs                   # Core doctor logic (pub(crate))
│   ├── check.rs                    # (pub(crate))
│   └── commands/                   # (pub(crate))
│
├── analyze/                        # Analyze feature - PUBLIC library API
│   ├── mod.rs                      # Exports public functions: analyze_text(), analyze_command()
│   ├── options.rs                  # Public option types
│   ├── engine.rs                   # Core analysis logic (pub(crate))
│   └── error.rs                    # Public error types
│
├── report/                         # Report feature - PUBLIC library API
│   ├── mod.rs                      # Exports public functions: generate()
│   ├── options.rs                  # Public option types
│   └── generator.rs                # Core report logic (pub(crate))
│
├── lint/                           # Lint feature - PUBLIC library API
│   ├── mod.rs                      # Exports public functions
│   └── options.rs
│
├── config/                         # Config loading - PUBLIC (renamed from shared/config_load)
│   ├── mod.rs                      # Public config loading functions
│   └── types.rs                    # FoundConfig and related types
│
├── capture/                        # Output capture - PUBLIC (extracted from shared)
│   ├── mod.rs                      # OutputCapture, CaptureOpts, etc.
│   └── providers.rs                # ExecutionProvider trait & impls
│
├── internal/                       # INTERNAL utilities (renamed from shared)
│   ├── mod.rs
│   ├── redact.rs                   # (pub(crate) only)
│   ├── macros.rs                   # Internal macros like report_stdout!
│   └── ...
│
└── models/                         # Data models (public)
    ├── core.rs
    └── v1alpha/
```

### New Module Breakdown

#### **Output & Interaction**

- **Library modules (`doctor`, `analyze`, `report`) do not print to console.** They return domain-specific result types (e.g., `RunResult`, `AnalysisResult`).
- **CLI module formats and prints** results using human-friendly tables, colors, and messages.
- **Debug/trace logs** can remain via `tracing`, but avoid `target="user"` from library code; reserve user-facing channels for CLI.
- **Macros like `report_stdout!`** belong in `internal/` or are used solely by the CLI layer, not by library functions.

#### **`src/doctor/`** - Doctor Module (Public API)

The doctor module becomes the **public library API** for health check functionality.

**`doctor/mod.rs`:**
```rust
//! Doctor - Development environment health checks
//!
//! Run health checks on a development environment to verify proper setup.
//!
//! # Examples
//!
//! ```no_run
//! use dev_scope::doctor::{run, RunOptions};
//! use dev_scope::config::load;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let config = load(None).await?;
//!     let options = RunOptions::default();
//!     let result = run(&config, options).await?;
//!     println!("Checks passed: {}", result.checks_passed);
//!     Ok(())
//! }
//! ```

// Public API
mod options;
mod result;

// Internal implementation
mod runner;
mod check;
mod commands;

pub use options::{RunOptions, ListOptions};
pub use result::{RunResult, CheckResult};

// Public functions - the actual library API
pub use runner::run;
pub use commands::list;
```

**`doctor/options.rs`:** (Public types, no CLI dependencies)
```rust
//! Public option types for doctor functionality

/// Options for running doctor checks
#[derive(Debug, Clone, Default)]
pub struct RunOptions {
    /// Groups to run (empty = all groups)
    pub groups: Vec<String>,
    /// Whether to run fixes automatically
    pub auto_fix: bool,
    /// Whether to run in CI mode (non-interactive)
    pub ci_mode: bool,
    /// Whether to run in file-cache mode
    pub file_cache: bool,
}

/// Options for listing doctor checks
#[derive(Debug, Clone, Default)]
pub struct ListOptions {
    /// Show all checks including disabled ones
    pub show_all: bool,
}
```

**`doctor/result.rs`:** (Public result types)
```rust
//! Public result types for doctor operations

/// Result of running doctor checks
#[derive(Debug)]
pub struct RunResult {
    pub groups_run: usize,
    pub checks_passed: usize,
    pub checks_failed: usize,
    pub checks_skipped: usize,
    pub exit_code: i32,
}

/// Information about a single check
#[derive(Debug, Clone)]
pub struct CheckInfo {
    pub name: String,
    pub group: String,
    pub description: String,
}
```

**`doctor/runner.rs`:** (Public function, internal implementation)
```rust
//! Doctor runner implementation

use crate::config::FoundConfig;
use super::{RunOptions, RunResult};
use anyhow::Result;

/// Run doctor health checks
///
/// # Examples
///
/// ```no_run
/// use dev_scope::doctor::{run, RunOptions};
/// use dev_scope::config::load;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let config = load(None).await?;
///     let options = RunOptions::default();
///     let result = run(&config, options).await?;
///     println!("Checks passed: {}", result.checks_passed);
///     Ok(())
/// }
/// ```
pub async fn run(config: &FoundConfig, options: RunOptions) -> Result<RunResult> {
    // Implementation here - calls internal runner logic
    // This function is public and part of the library API
    todo!("Implement using internal DoctorRunner")
}
```

#### **`src/analyze/`** - Analyze Module (Public API)

**`analyze/mod.rs`:**
```rust
//! Analyze - Detect known errors in command output or logs
//!
//! # Examples
//!
//! ```no_run
//! use dev_scope::analyze::{text, Options};
//! use dev_scope::config::load;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let config = load(None).await?;
//!     let log_content = std::fs::read_to_string("error.log")?;
//!     let result = text(&config, &log_content, Options::default()).await?;
//!     for error in result.matched_errors {
//!         println!("Found: {} - {}", error.error_name, error.help_text);
//!     }
//!     Ok(())
//! }
//! ```

// Public types
mod options;
mod result;

// Internal implementation
mod engine;

pub use options::Options;
pub use result::{Result as AnalysisResult, MatchedError};

// Public API functions
pub use engine::{text, command};
```

**`analyze/options.rs`:**
```rust
/// Options for analyzing output
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Maximum number of matches to return
    pub max_matches: Option<usize>,
    /// Include context lines around matches
    pub context_lines: usize,
}
```

**`analyze/result.rs`:**
```rust
/// A matched error in the analyzed content
#[derive(Debug, Clone)]
pub struct MatchedError {
    pub error_name: String,
    pub help_text: String,
    pub line_number: Option<usize>,
    pub context: String,
}

/// Result of analyzing content
#[derive(Debug)]
pub struct Result {
    pub matched_errors: Vec<MatchedError>,
    pub exit_code: i32,
}
```

#### **`src/report/`** - Report Module (Public API)

**`report/mod.rs`:**
```rust
//! Report - Generate reports from command execution

mod options;
mod result;
mod generator;

pub use options::Options;
pub use result::Result;
pub use generator::generate;
```

**`report/options.rs`:**
```rust
/// Options for generating a report
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Specific report location to use (None = use all configured)
    pub location: Option<String>,
    /// Include additional data commands
    pub include_additional_data: bool,
}
```

#### **`src/config/`** - Configuration Module (Public)

**`config/mod.rs`:**
```rust
//! Configuration loading and management
//!
//! # Examples
//!
//! ```no_run
//! use dev_scope::config;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let config = config::load(None).await?;
//!     println!("Loaded {} config files", config.raw_config.len());
//!     Ok(())
//! }
//! ```

mod loader;
mod types;

pub use types::{FoundConfig, ConfigSource};
pub use loader::load;

use anyhow::Result;
use std::path::PathBuf;

/// Load configuration from the filesystem
///
/// If `config_path` is None, searches standard locations:
/// - `.scope/` in current directory and parents
/// - Global config directory
pub async fn load(config_path: Option<PathBuf>) -> Result<FoundConfig> {
    loader::load_from_path(config_path).await
}
```

#### **`src/cli/`** - CLI Layer (NEW, Private)

This module contains ALL CLI-specific code. It's **not** exported in `lib.rs` - only used by binaries.

**`cli/args.rs`:**
```rust
//! Clap argument definitions
//! 
//! This is ONLY used by the binary, not part of the public library API

use clap::{Parser, Subcommand, Args};

#[derive(Parser)]
#[clap(author, version, about)]
pub struct Cli {
    #[clap(flatten)]
    pub logging: super::logging::LoggingOpts,  // Now in cli module

    #[clap(flatten)]
    pub config: crate::config::Options,  // Now in public config module

    #[clap(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    #[clap(alias("d"))]
    Doctor(DoctorArgs),
    
    #[clap(alias("r"))]
    Report(ReportArgs),
    
    #[clap(alias("a"))]
    Analyze(AnalyzeArgs),
    
    Lint(LintArgs),
    
    #[clap(alias("l"))]
    List,
    
    #[clap(alias("v"))]
    Version(VersionArgs),
    
    #[command(external_subcommand)]
    ExternalSubCommand(Vec<String>),
}

#[derive(Debug, Args)]
pub struct DoctorArgs {
    #[clap(subcommand)]
    pub command: DoctorCommands,
}

#[derive(Debug, Subcommand)]
pub enum DoctorCommands {
    Run {
        #[arg(short, long)]
        groups: Vec<String>,
        
        #[arg(long)]
        auto_fix: bool,
        
        #[arg(long)]
        ci_mode: bool,
        
        #[arg(long)]
        file_cache: bool,
    },
    List {
        #[arg(short, long)]
        all: bool,
    },
    Init,
}

// Similar for Analyze, Report, Lint, etc.
```

**`cli/commands.rs`:**
```rust
//! Command routing - converts CLI args to library API calls

use crate::{doctor, analyze, report, config};
use anyhow::Result;
use super::args::*;

/// Route the CLI command to the appropriate library function
pub async fn handle_command(config: &config::FoundConfig, command: &Command) -> Result<i32> {
    match command {
        Command::Doctor(args) => handle_doctor(config, args).await,
        Command::Report(args) => handle_report(config, args).await,
        Command::Analyze(args) => handle_analyze(config, args).await,
        Command::Lint(args) => handle_lint(config, args).await,
        Command::List => handle_list(config).await,
        Command::Version(args) => handle_version(args).await,
        Command::ExternalSubCommand(args) => handle_external(config, args).await,
    }
}

async fn handle_doctor(config: &config::FoundConfig, args: &DoctorArgs) -> Result<i32> {
    match &args.command {
        DoctorCommands::Run { groups, auto_fix, ci_mode, file_cache } => {
            // Convert CLI args to library options type
            let options = doctor::RunOptions {
                groups: groups.clone(),
                auto_fix: *auto_fix,
                ci_mode: *ci_mode,
                file_cache: *file_cache,
            };
            // Call public library function
            let result = doctor::run(config, options).await?;
            println!("Checks passed: {}/{}", result.checks_passed, result.groups_run);
            Ok(result.exit_code)
        }
        DoctorCommands::List { all } => {
            let options = doctor::ListOptions { show_all: *all };
            let checks = doctor::list(config, options).await?;
            for check in checks {
                println!("  - {}", check.name);
            }
            Ok(0)
        }
        DoctorCommands::Init => {
            // Call library function to generate example config
            Ok(0)
        }
    }
}

// Similar for other commands...
```

#### **`src/bin/scope.rs`** - THIN Binary

The binary becomes **minimal** - just setup and delegation:

```rust
// Binaries can access modules in src/ directly, even if not in lib.rs
mod cli;

use clap::Parser;
use cli::{Cli, commands};
use dev_scope::config;
use human_panic::setup_panic;

#[tokio::main]
async fn main() {
    setup_panic!();
    dotenvy::dotenv().ok();
    
    // Load env file from installation
    let exe_path = std::env::current_exe().unwrap();
    let env_path = exe_path.parent().unwrap().join("../etc/scope.env");
    dotenvy::from_path(env_path).ok();
    
    // Parse CLI args
    let cli = Cli::parse();
    
    // Setup logging (CLI-specific utility from cli module)
    let _logger = cli::logging::configure(&cli.logging, &cli.config.get_run_id(), "root").await;
    
    // Load config using public library API
    let cfg = match config::load(cli.config.config_path.clone()).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to load configuration: {}", e);
            std::process::exit(2);
        }
    };
    
    // Delegate to CLI command handler
    let exit_code = commands::handle_command(&cfg, &cli.command)
        .await
        .unwrap_or_else(|e| {
            eprintln!("Error: {}", e);
            1
        });
    
    std::process::exit(exit_code);
}
```

**That's it!** Binary is now ~45 lines instead of 232.

**Note:** The `cli` module lives in `src/cli/` but isn't exported through `lib.rs`, so only binaries in the same crate can access it - external crates cannot.

#### **`src/lib.rs`** - Library Root (Updated)

```rust
//! Scope - Development environment health checks and error analysis
//!
//! # Overview
//!
//! Scope provides tools for managing local machine checks, generating bug reports,
//! and analyzing command output for known errors.
//!
//! # Usage as a Library
//!
//! ```no_run
//! use dev_scope::{config, doctor};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Load configuration
//!     let cfg = config::load(None).await?;
//!     
//!     // Run doctor checks
//!     let options = doctor::RunOptions::default();
//!     let result = doctor::run(&cfg, options).await?;
//!     
//!     println!("Health check complete: {}/{} passed",
//!              result.checks_passed, result.groups_run);
//!     Ok(())
//! }
//! ```
//!
//! # Module Organization
//!
//! - [`doctor`] - Health checks for development environments
//! - [`analyze`] - Error detection in command output
//! - [`report`] - Bug report generation
//! - [`lint`] - Configuration validation
//! - [`config`] - Configuration loading and management
//! - [`capture`] - Command execution and output capture
//! - [`models`] - Data models and schemas

// ============================================================================
// Public API - These modules form the library's public interface
// ============================================================================

pub mod doctor;
pub mod analyze;
pub mod report;
pub mod lint;
pub mod config;
pub mod capture;
pub mod models;

// ============================================================================
// Internal modules - Used only within this crate
// ============================================================================

// Note: `internal` contains utilities used across multiple modules internally.
// Anything CLI-specific has been moved to the `cli` module.
// Anything useful for library consumers has been made public.
pub(crate) mod internal;
```

**Key changes:**
- **NO prelude!** Direct module exports only
- Feature modules (`doctor`, `analyze`, `report`, `lint`) are **public**
- `internal` is `pub(crate)` - truly internal utilities only
- **`cli` module is NOT in lib.rs** - only accessible to binaries
- Clear documentation about what's public vs internal
- Utilities extracted to appropriate locations (`config`, `capture` are public)

---

## Migration Path

### Phase 1: Refactor Without Breaking Changes (Low Risk)

1. **Remove prelude pattern**
   - Remove `prelude` modules from `doctor/`, `analyze/`, `report/`, `lint/`
   - Update internal imports to use explicit module paths
   - Update `lib.rs` to export modules directly, not via prelude
   - Add deprecation warnings if needed for external users

2. **Reorganize `shared` module**
   - Rename `shared` → `internal` for clarity
   - Extract public utilities:
     - `shared/config_load.rs` → `config/` (public module)
     - `shared/capture.rs` → `capture/` (public module)
     - `shared/report.rs` → Extract report builders to public locations
   - Move CLI-specific code:
     - `shared/logging.rs` → `cli/logging.rs`
   - Keep truly internal utilities in `internal/`:
     - Redaction, internal macros, etc.

3. **Extract domain types from CLI types**
   - Create `options.rs` files with CLI-independent types (e.g., `doctor::RunOptions`)
   - Keep existing CLI arg types in current locations temporarily
   - Add conversion functions between CLI args and domain options

4. **Remove console output from library code**
    - Audit and replace `report_stdout!`, `println!`, and colorized user messages in library modules.
    - Return structured results from library functions; move all formatting to `cli/`.
    - Keep `tracing` for debug, but avoid user-targeted logs in library.

5. **Make core functions public**
   - Export public functions directly from modules (e.g., `pub use runner::run` in `doctor/mod.rs`)
   - Keep implementation details `pub(crate)` or private
   - Add comprehensive documentation to public functions

6. **Add library-focused tests**
   - Test public module functions
   - Verify they work independently of CLI
   - Test with domain option types, not CLI args

### Phase 2: Move CLI Code (Medium Risk)

1. **Create `cli/` module**
   - Move CLI arg definitions to `cli/args.rs`
   - Keep existing `doctor/cli.rs` etc. for compatibility
   - Have them import from `cli/args.rs`

2. **Extract routing logic**
   - Move command routing to `cli/commands.rs`
   - Update binaries to use new routing
   - Remove routing from `doctor/cli.rs` etc.

### Phase 3: Refactor Binaries (Medium Risk)

1. **Simplify `bin/scope.rs`**
   - Move logic to `cli/` module
   - Keep binary as thin wrapper
   - Update `bin/scope-intercept.rs` similarly

2. **Update internal modules**
   - Keep `doctor`, `analyze`, `report`, `lint` public as library API
   - Ensure internal implementation is `pub(crate)` or private
   - Remove old `cli.rs` files from feature modules

### Phase 4: Polish & Documentation (Low Risk)

1. **Documentation**
   - Add comprehensive rustdoc to public modules (`doctor`, `analyze`, etc.)
   - Provide usage examples in module documentation
   - Create migration guide for any external consumers

2. **Testing**
   - Ensure 100% of public functions have tests
   - Add integration tests using library directly
   - Keep CLI integration tests

---

## Benefits of Refactoring

### For Library Users
✅ Clear, documented API  
✅ Use Scope from other Rust projects  
✅ No CLI dependencies  
✅ Stable public interface  

### For Maintainers
✅ Easier testing - mock at API boundary  
✅ Clearer separation of concerns  
✅ Reduced code duplication  
✅ Easier to add new interfaces (HTTP, gRPC, etc.)  

### For Contributors
✅ Obvious where to add new features  
✅ Less coupling between components  
✅ Easier to understand codebase  

---

## Comparison: Current vs. Proposed

### Adding a New Command Feature

**Current (CLI-coupled):**
1. Add to `Command` enum in `bin/scope.rs`
2. Add handler in `handle_commands()`
3. Create new module with `cli.rs` + `commands/`
4. Add `*_root()` function that takes CLI args
5. Implement logic mixed with CLI routing
6. Export via prelude

**Proposed (Library-first):**
1. Create new module `new_feature/` with internal implementation
2. Add public functions to `new_feature/mod.rs`
3. Export module from `lib.rs` as `pub mod new_feature;`
4. Add CLI args to `cli/args.rs`
5. Add routing case in `cli/commands.rs` that calls `new_feature::run()`

**Result:** Clear separation, logic testable without CLI.

### Using Scope from Another Tool

**Current:**
```rust
// Not possible - would need to:
// 1. Create fake CLI args
// 2. Call internal functions via prelude
// 3. Deal with CLI-specific types
use dev_scope::prelude::*;  // What does this even export?
let args = DoctorArgs { /* ... */ };  // Have to use CLI types!
doctor_root(&config, &args).await?;  // CLI-specific function
```

**Proposed:**
```rust
use dev_scope::{config, doctor};  // Clear, explicit imports

let cfg = config::load(None).await?;
let opts = doctor::RunOptions::default();  // Pure domain type
let result = doctor::run(&cfg, opts).await?;  // Clean library function
// No CLI involvement, clear module boundaries
```

---

## Open Questions & Considerations

### 1. **Backward Compatibility**
- Do we need to maintain existing preludes during transition?
  * No. We have not external consumers of the current "library" code.
- Are there external consumers of the current library API?
  * Nope. We have a chance to make whatever breaking changes we need to to create a clean API.
- Can we do this in multiple releases with deprecation warnings?
  * Yes, but again, we're not yet publisihing the crate as a library, so we can be pretty liberal with making changes _until_ we release a stable version of the library.

### 2. **Feature Flags**
- Should CLI code be behind a `cli` feature flag?
- Could allow library-only builds to be smaller
- Example: `cargo build --no-default-features --features=api`

### 3. **Async Runtime**
- API currently requires `tokio` - is this acceptable for library users?
- Could we provide sync wrappers for some functions?
- Consider `async-std` compatibility

### 4. **Error Handling**
- Current code uses `anyhow::Result` throughout
- Library API should use structured error types
- Consider creating `scope::Error` enum with specific variants

### 5. **Configuration Model**
- `FoundConfig` is complex - is it the right public type?
  * Maybe not, but we _know_ it's not a great name.
- Should we have a simpler builder pattern for library users?
- Consider separating "search for config" from "use this config"

---

## Recommended Next Steps

1. **Prototype library usage for analyze/fix**
    - Export `analyze::text()` and `analyze::command()` with `Options` and `AnalysisResult` types.
    - Export `doctor::run()` with `RunOptions` (including `auto_fix`, `groups`, `ci_mode`, `file_cache`) and `RunResult`.
    - Ensure fix execution is invokable via `RunOptions.auto_fix`; add granular fix hooks if needed (e.g., per-check fix controls).

2. **Decouple CLI from library**
    - Move all `clap` argument types into `cli/` and convert to library options (`analyze::Options`, `doctor::RunOptions`).
    - Remove prelude usage; prefer explicit module paths and public exports.
    - Keep CLI-only utilities (e.g., logging) in `cli/` and internal-only helpers in `internal/`.

3. **Add an example consumer crate**
    - Create `examples/library-consumer/` demonstrating programmatic use of `analyze` and `doctor` from another Rust program.
    - Include a minimal README and a runnable example that compiles against the library API.

4. **Tests and documentation**
    - Add rustdoc examples to `analyze` and `doctor` public functions showing typical usage.
    - Write unit tests for `analyze::text()`, `analyze::command()`, and `doctor::run()` using domain option types (not CLI).
    - Add integration tests verifying external consumption (build the example and run a simple scenario).

5. **Incremental execution**
    - Refactor `analyze` first to validate the pattern, then `doctor`.
    - Update binaries to call the new public functions; keep binaries thin.

6. **Success criteria**
    - External crate builds and runs using `analyze` and `doctor` without touching CLI types.
    - Binary lines of code decrease significantly and remain thin.
    - Test coverage of public library functions increases and stays stable.
    - No references to an `api` module; module boundaries are explicit and clear.

---

## Conclusion

The proposed refactoring transforms Scope from a **CLI-first tool** to a **library-first tool with a CLI interface**. This follows Rust ecosystem best practices and provides significant benefits:

- **Reusability**: Use Scope logic from other tools
- **Testability**: Test domain logic without CLI layer
- **Maintainability**: Clear boundaries between components
- **Extensibility**: Easy to add new interfaces (web, RPC, etc.)

The migration can be done incrementally with low risk, and the end result is a more robust, flexible, and maintainable codebase.
