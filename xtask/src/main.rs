//! The workspace's task runner — the [cargo-xtask] pattern: a plain Rust binary,
//! invoked as `cargo xtask <task>` through the alias in `.cargo/config.toml`, so
//! every developer already has the only toolchain it needs.
//!
//! CI and releasing are not here. Both are `ship`, configured by
//! `.config/ship.figl`: `ship ci` runs the checks (some of them call tasks
//! here), and `ship` cuts a release, running `sync-versions` after the bump.
//!
//! [cargo-xtask]: https://github.com/matklad/cargo-xtask

mod isolation;
mod package;
mod swift;
mod util;
mod versions;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "cargo xtask",
    about = "Drive the steps that leave Rust, and the checks too big for one command",
    disable_help_subcommand = true
)]
struct Cli {
    #[command(subcommand)]
    task: Task,
}

#[derive(Subcommand)]
enum Task {
    /// Check each crate alone, as a consumer would build it.
    PackageIsolation,
    /// (Re)generate the committed UniFFI Swift binding from crates/thorn-svg-ffi.
    Bindings,
    /// Build and launch Thorn, the Apple app (apps/thorn-editor), over packages/thorn-swift.
    Swift(swift::Args),
    /// Build a signed, notarised Thorn.app in a .dmg for other Macs (target/package/).
    Package(package::Args),
    /// Write the workspace version into the files no manifest parser reaches
    /// (apps/thorn-editor/project.yml). `ship`'s bump runs this.
    SyncVersions {
        /// Write nothing; fail unless every file already states the version.
        #[arg(long)]
        check: bool,
    },
    /// Moved: CI is `ship ci` now, configured by `.config/ship.figl`.
    #[command(hide = true)]
    Ci {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        rest: Vec<String>,
    },
}

fn main() -> Result<()> {
    match Cli::parse().task {
        Task::PackageIsolation => isolation::run_task(),
        Task::Bindings => util::run(util::cmd("bash").arg("scripts/gen-bindings.sh")),
        Task::Swift(args) => swift::run_task(args),
        Task::Package(args) => package::run_task(args),
        Task::SyncVersions { check: false } => versions::run_task(),
        Task::SyncVersions { check: true } => versions::check(),
        Task::Ci { .. } => anyhow::bail!(
            "`cargo xtask ci` is `ship ci` now (`ship ci <job>` for one job, `ship ci --list` for the jobs)"
        ),
    }
}
