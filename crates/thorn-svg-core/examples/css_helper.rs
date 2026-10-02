//! thorn's CSS, served to a `fig` binary over stdio — so the command line
//! reads, edits and checks a stylesheet or a `style` attribute through the
//! same language the editor does:
//!
//! ```fig
//! # ~/.config/fig/languages.figl
//! language[]
//! > name = css
//! > extensions = [css]
//! > command = [target/debug/examples/css_helper]
//! ```
//!
//! ```sh
//! fig lang table -i css drawing.css
//! fig set drawing.css 'rect.stroke' '#222'
//! ```

fn main() -> std::io::Result<()> {
    fig::helper::serve(thorn_svg_core::css::Css)
}
