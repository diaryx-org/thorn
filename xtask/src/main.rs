//! The workspace's task runner — the [cargo-xtask] pattern: a plain Rust binary,
//! invoked as `cargo xtask <task>` through the alias in `.cargo/config.toml`, so
//! every developer already has the only toolchain it needs.
//!
//! Releasing is not here. It is `dx release`, from the org's devtools,
//! configured by `.config/release.toml`.
//!
//! [cargo-xtask]: https://github.com/matklad/cargo-xtask

mod ci;
mod swift;
mod util;
mod versions;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "cargo xtask",
    about = "Check the workspace and drive the steps that leave Rust",
    disable_help_subcommand = true
)]
struct Cli {
    #[command(subcommand)]
    task: Task,
}

#[derive(Subcommand)]
enum Task {
    /// Run the checks a release has to pass — all of them, or one by id.
    Ci(ci::Args),
    /// (Re)generate the committed UniFFI Swift binding from crates/thorn-svg-ffi.
    Bindings,
    /// Build and launch Thorn, the Apple app (apps/thorn-editor), over packages/thorn-swift.
    Swift(swift::Args),
    /// Write the workspace version into the files no manifest parser reaches
    /// (apps/thorn-editor/project.yml). The release bump runs this.
    SyncVersions,
}

fn main() -> Result<()> {
    match Cli::parse().task {
        Task::Ci(args) => ci::run_task(args),
        Task::Bindings => util::run(util::cmd("bash").arg("scripts/gen-bindings.sh")),
        Task::Swift(args) => swift::run_task(args),
        Task::SyncVersions => versions::run_task(),
    }
}
