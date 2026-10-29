//! Thorn: a drawing in a window. `Thorn [drawing.svg]` opens the file, or a
//! new drawing with none; the window's bar has New, Open, Save and Save As,
//! and their keys.

mod app;
mod keys;

use std::path::PathBuf;

use dioxus_native::{Config, LogicalSize, WindowAttributes};

fn main() {
    let path = std::env::args_os().nth(1).map(PathBuf::from);
    let window = WindowAttributes::default()
        .with_title(app::title(path.as_deref(), false))
        .with_surface_size(LogicalSize::new(1100.0, 760.0));
    dioxus_native::launch_cfg_with_props(
        app::App,
        app::AppProps { path },
        vec![],
        vec![Box::new(Config::new().with_window_attributes(window))],
    );
}
