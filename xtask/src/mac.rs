//! `cargo xtask mac [file.svg]` — open the Mac app around `DrawingEditor`.
//!
//! The two commands README.md → Building spells out, in order: the Rust
//! staticlib the app force-loads must exist before `swift run` links it, and
//! `apps/thorn-mac/Package.swift` names it by path, so a stale or missing
//! `target/debug/libthorn_svg_ffi.a` is a link error rather than a message.

use crate::util::{cargo, cmd, run};
use anyhow::Result;
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct Args {
    /// The drawing to open; written back on quit. A fresh page without one.
    file: Option<PathBuf>,

    /// Rebuild nothing; run the app as last built.
    #[arg(long)]
    no_build: bool,
}

pub fn run_task(args: Args) -> Result<()> {
    if !args.no_build {
        run(cargo().args(["build", "-p", "thorn-svg-ffi"]))?;
    }
    let mut swift = cmd("swift");
    swift.args(["run", "--package-path", "apps/thorn-mac"]);
    // Before the product: everything after it is the app's own arguments.
    if args.no_build {
        swift.arg("--skip-build");
    }
    swift.arg("thorn-mac");
    // The subprocess runs from the repo root; the path was typed from wherever
    // the caller stood.
    if let Some(file) = args.file {
        swift.arg(std::path::absolute(file)?);
    }
    run(&mut swift)
}
