//! The window: a bar with the drawing's name and New, Open, Save and Save
//! As, the editor under it filling the rest, and the keys.
//!
//! The page is under `UnitRoot` with `Keys::Host`: every key the window
//! hears is offered to unit first (an open menu's), then the app's own
//! shortcuts, then the canvas ([`CanvasHandle::key`]). A label's field is a
//! Blitz `<textarea>`, which takes its keys from the window itself, so
//! while one is open the canvas takes none.
//!
//! The window's size and appearance come from winit, as Blitz cannot be
//! asked for an element's box from a handler: the editor is the window less
//! the bar.

use std::path::{Path, PathBuf};

use dioxus::prelude::*;
use dioxus_native::winit::event::WindowEvent;
use dioxus_native::winit::keyboard::ModifiersState;
use dioxus_native::winit::window::Theme;
use thorn_dioxus::{Appearance, CanvasHandle, DrawingEditor, icons, use_canvas};
use thorn_svg_core::Drawing;
use unit::{Button, Keys, UnitRoot, use_unit};

use crate::keys;

/// The bar's height, in CSS pixels.
const BAR: f64 = 44.0;

/// The window's title: the file's name, or Untitled, and whether there is
/// anything unsaved.
pub fn title(path: Option<&Path>, edited: bool) -> String {
    let name = path
        .and_then(Path::file_name)
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Untitled".into());
    if edited {
        format!("{name} — Edited")
    } else {
        name
    }
}

/// The file the window has open, and the canvas revision last written to
/// it.
#[derive(Clone, Debug, PartialEq)]
struct Document {
    path: Option<PathBuf>,
    saved: u64,
}

/// Open `path`, or a new drawing for none. A file that will not open says
/// why, and the window starts on a new drawing rather than none.
fn open(path: Option<&Path>) -> (Drawing, Option<String>) {
    let Some(path) = path else {
        return (Drawing::fresh(), None);
    };
    match std::fs::read_to_string(path) {
        Ok(text) => match Drawing::open(&text) {
            Ok(d) => (d, None),
            Err(e) => (Drawing::fresh(), Some(format!("{}: {e}", path.display()))),
        },
        // A path that is not there yet is where a new drawing will be saved.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Drawing::fresh(), None),
        Err(e) => (Drawing::fresh(), Some(format!("{}: {e}", path.display()))),
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct AppProps {
    pub path: Option<PathBuf>,
}

#[allow(non_snake_case)]
pub fn App(props: AppProps) -> Element {
    let window = dioxus_native::use_window();
    let size = use_signal({
        let window = window.clone();
        move || {
            let s = window
                .surface_size()
                .to_logical::<f64>(window.scale_factor());
            (s.width, s.height)
        }
    });
    let dark = use_signal({
        let window = window.clone();
        move || window.theme() == Some(Theme::Dark)
    });
    let path = props.path.clone();
    // What the first file said as it opened, if it would not.
    let first = std::cell::RefCell::new(None);
    let canvas = use_canvas(|| {
        let (drawing, error) = open(path.as_deref());
        *first.borrow_mut() = error;
        drawing
    });
    let problem = use_signal(|| first.take());
    let doc = use_signal(|| Document {
        path: props.path.clone(),
        saved: 0,
    });

    rsx! {
        UnitRoot {
            keys: Keys::Host,
            appearance: if dark() { unit::Appearance::Dark } else { unit::Appearance::Light },
            class: "thorn-app",
            style { {APP_CSS} {thorn_dioxus::css()} }
            Window { canvas, doc, size, dark, problem }
        }
    }
}

/// Everything under `UnitRoot`, where unit's context is.
#[component]
fn Window(
    canvas: CanvasHandle,
    doc: Signal<Document>,
    size: Signal<(f64, f64)>,
    dark: Signal<bool>,
    problem: Signal<Option<String>>,
) -> Element {
    let unit = use_unit();
    let window = dioxus_native::use_window();

    // The window's events: its size, its appearance, a pinch, and the keys.
    let mut mods = ModifiersState::empty();
    let mut pointer = (0.0, 0.0);
    {
        let window = window.clone();
        let mut size = size;
        let mut dark = dark;
        dioxus_native::use_window_event(move |event, _| match event {
            WindowEvent::SurfaceResized(s) => {
                let s = s.to_logical::<f64>(window.scale_factor());
                size.set((s.width, s.height));
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                let s = window
                    .surface_size()
                    .to_logical::<f64>(window.scale_factor());
                size.set((s.width, s.height));
            }
            WindowEvent::ThemeChanged(t) => dark.set(*t == Theme::Dark),
            WindowEvent::ModifiersChanged(m) => mods = m.state(),
            WindowEvent::PointerMoved { position, .. } => {
                let p = position.to_logical::<f64>(window.scale_factor());
                pointer = (p.x, p.y - BAR);
            }
            WindowEvent::PinchGesture { delta, .. } => {
                let at = pointer;
                canvas.update(|c| c.zoom_by(1.0 + delta, at));
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let Some((key, m)) = keys::pressed(event, mods)
                    && !unit.key(&key, m)
                    && !unit.typing()
                    && !shortcut(&key, m, canvas, doc, problem)
                {
                    canvas.key(&key, m);
                }
            }
            _ => {}
        });
    }

    let appearance = if dark() {
        Appearance::Dark
    } else {
        Appearance::Light
    };
    if canvas.peek().appearance() != appearance {
        canvas.update(|c| c.set_appearance(appearance));
    }

    let revision = canvas.read().revision();
    let d = doc.read().clone();
    let edited = revision != d.saved;
    let name = title(d.path.as_deref(), false);
    window.set_title(&title(d.path.as_deref(), edited));
    let (w, h) = size();

    rsx! {
        div { class: "thorn-bar", style: "height: {BAR}px",
            div { class: "thorn-title",
                span { class: "thorn-name", "{name}" }
                if edited { span { class: "thorn-edited", "Edited" } }
            }
            div { class: "thorn-actions",
                Button { class: "unit-icon", refuses_keys: true, label: "New drawing", icon: icons::NEW,
                    onpress: move |_| new_drawing(canvas, doc) }
                Button { class: "unit-icon", refuses_keys: true, label: "Open…", icon: icons::OPEN,
                    onpress: move |_| open_dialog(canvas, doc, problem) }
                Button { class: "unit-icon", refuses_keys: true, label: "Save", icon: icons::SAVE,
                    onpress: move |_| save(canvas, doc, problem, false) }
            }
        }
        if let Some(p) = problem() {
            div { class: "thorn-problem", role: "alert",
                span { "{p}" }
                Button { variant: unit::Variant::Ghost, refuses_keys: true, onpress: move |_| problem.set(None), "Dismiss" }
            }
        }
        DrawingEditor { canvas, width: w, height: (h - BAR).max(0.0) }
    }
}

/// The app's own keys: ⌘N, ⌘O, ⌘S and ⇧⌘S (Control on other platforms).
/// `true` when the key was one.
fn shortcut(
    key: &Key,
    mods: Modifiers,
    canvas: CanvasHandle,
    doc: Signal<Document>,
    problem: Signal<Option<String>>,
) -> bool {
    if !(mods.meta() || mods.ctrl()) || mods.alt() {
        return false;
    }
    let Key::Character(c) = key else {
        return false;
    };
    match (c.to_lowercase().as_str(), mods.shift()) {
        ("n", false) => new_drawing(canvas, doc),
        ("o", false) => open_dialog(canvas, doc, problem),
        ("s", shift) => save(canvas, doc, problem, shift),
        _ => return false,
    }
    true
}

fn new_drawing(canvas: CanvasHandle, mut doc: Signal<Document>) {
    canvas.replace(Drawing::fresh());
    doc.set(Document {
        path: None,
        saved: canvas.peek().revision(),
    });
}

fn open_dialog(
    canvas: CanvasHandle,
    mut doc: Signal<Document>,
    mut problem: Signal<Option<String>>,
) {
    let folder = doc
        .peek()
        .path
        .as_ref()
        .and_then(|p| p.parent().map(Path::to_path_buf));
    spawn(async move {
        let mut dialog = rfd::AsyncFileDialog::new()
            .set_title("Open")
            .add_filter("SVG drawing", &["svg"]);
        if let Some(f) = folder {
            dialog = dialog.set_directory(f);
        }
        let Some(file) = dialog.pick_file().await else {
            return;
        };
        let path = file.path().to_path_buf();
        let (drawing, error) = open(Some(&path));
        if let Some(e) = error {
            problem.set(Some(e));
            return;
        }
        canvas.replace(drawing);
        problem.set(None);
        doc.set(Document {
            path: Some(path),
            saved: canvas.peek().revision(),
        });
    });
}

/// Write the drawing to its file; with `ask`, or with no file yet, to one
/// the save dialog names.
fn save(
    canvas: CanvasHandle,
    mut doc: Signal<Document>,
    mut problem: Signal<Option<String>>,
    ask: bool,
) {
    let path = doc.peek().path.clone();
    let mut write = move |path: PathBuf| {
        let (source, revision) = {
            let c = canvas.peek();
            (c.source().to_string(), c.revision())
        };
        match std::fs::write(&path, source) {
            Ok(()) => {
                problem.set(None);
                doc.set(Document {
                    path: Some(path),
                    saved: revision,
                });
            }
            Err(e) => problem.set(Some(format!("Not saved: {}: {e}", path.display()))),
        }
    };
    match path {
        Some(p) if !ask => write(p),
        _ => {
            let name = title(path.as_deref(), false);
            let name = if name.ends_with(".svg") {
                name
            } else {
                format!("{name}.svg")
            };
            let folder = path
                .as_ref()
                .and_then(|p| p.parent().map(Path::to_path_buf));
            spawn(async move {
                let mut dialog = rfd::AsyncFileDialog::new()
                    .set_title("Save")
                    .set_file_name(name)
                    .add_filter("SVG drawing", &["svg"]);
                if let Some(f) = folder {
                    dialog = dialog.set_directory(f);
                }
                if let Some(file) = dialog.save_file().await {
                    write(file.path().to_path_buf());
                }
            });
        }
    }
}

const APP_CSS: &str = r#"
html, body { margin: 0; padding: 0; overflow: hidden; }
.thorn-app { position: relative; width: 100vw; height: 100vh; overflow: hidden; background: var(--unit-background); color: var(--unit-text); font-family: var(--unit-font); }
.thorn-bar { display: flex; flex-direction: row; align-items: center; justify-content: space-between; padding: 0 10px; box-sizing: border-box; border-bottom: 1px solid var(--unit-border); background: var(--unit-surface); }
.thorn-title { display: flex; flex-direction: row; align-items: baseline; gap: 8px; }
.thorn-name { font-weight: 600; }
.thorn-edited { color: var(--unit-dim); font-size: 12px; }
.thorn-actions { display: flex; flex-direction: row; gap: 4px; }
.thorn-problem { position: absolute; left: 50%; bottom: 16px; transform: translateX(-50%); z-index: 10; display: flex; flex-direction: row; align-items: center; gap: 12px; padding: 8px 12px; border-radius: 8px; background: var(--unit-surface); border: 1px solid var(--unit-border); box-shadow: 0 2px 8px rgba(0, 0, 0, 0.2); }
"#;
