//! `thorn` — the profile on the command line. `check` is the test the
//! profile is held to; `render` is the picture a viewer with no script and no
//! stylesheet would draw, through resvg, for a thumbnail or a fixture.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use thorn_svg_core::Drawing;

#[derive(Parser)]
#[command(name = "thorn", version, about = "A drawing editor over twig's SVG")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Hold a file to the Diaryx drawing profile. Exits 1 on a finding.
    Check { file: PathBuf },
    /// List a file's shapes in paint order.
    Shapes { file: PathBuf },
    /// Render a file to a PNG through resvg.
    Render {
        file: PathBuf,
        /// Where to write the PNG; defaults to the input with `.png`.
        #[arg(short, long)]
        out: Option<PathBuf>,
        /// Device pixels per user unit.
        #[arg(long, default_value_t = 1.0)]
        scale: f32,
    },
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("thorn: {e:#}");
            ExitCode::from(2)
        }
    }
}

fn run(cli: Cli) -> Result<ExitCode> {
    match cli.command {
        Command::Check { file } => {
            let drawing = open(&file)?;
            let findings = drawing.check();
            for f in &findings {
                match &f.shape {
                    Some(id) => println!("{}: {id}: {}", file.display(), f.message),
                    None => println!("{}: {}", file.display(), f.message),
                }
            }
            if findings.is_empty() {
                println!(
                    "{}: conforms to profile {}",
                    file.display(),
                    thorn_svg_core::profile::VERSION
                );
                Ok(ExitCode::SUCCESS)
            } else {
                Ok(ExitCode::FAILURE)
            }
        }
        Command::Shapes { file } => {
            let drawing = open(&file)?;
            for shape in drawing.shapes() {
                let indent = "  ".repeat(shape.depth);
                let id = shape.id.as_deref().unwrap_or("(no data-id)");
                let geometry = shape
                    .kind
                    .geometry_attrs()
                    .iter()
                    .filter_map(|name| shape.attr(name).map(|v| format!("{name}={v}")))
                    .collect::<Vec<_>>()
                    .join(" ");
                println!("{indent}{} {id} {geometry}", shape.kind.tag());
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Render { file, out, scale } => {
            let out = out.unwrap_or_else(|| file.with_extension("png"));
            let source =
                std::fs::read(&file).with_context(|| format!("reading {}", file.display()))?;
            let png = render(&source, scale)?;
            std::fs::write(&out, png).with_context(|| format!("writing {}", out.display()))?;
            println!("{}", out.display());
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn open(file: &Path) -> Result<Drawing> {
    let source =
        std::fs::read_to_string(file).with_context(|| format!("reading {}", file.display()))?;
    Drawing::open(&source).with_context(|| format!("opening {}", file.display()))
}

/// The SVG as resvg draws it, at `scale` device pixels per user unit.
fn render(source: &[u8], scale: f32) -> Result<Vec<u8>> {
    // System fonts, so a `<text>` label lays out — without them usvg drops
    // text in a family it cannot find, and the picture has no labels.
    let mut options = resvg::usvg::Options::default();
    options.fontdb_mut().load_system_fonts();
    let tree = resvg::usvg::Tree::from_data(source, &options).context("parsing for render")?;
    let size = tree
        .size()
        .to_int_size()
        .scale_by(scale)
        .context("a size resvg can allocate")?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width(), size.height())
        .context("allocating the pixmap")?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap.encode_png().context("encoding the PNG")
}
