//! The canvas as a Dioxus component: the desk, the page's sheet, the
//! picture, what is drawn over it, a layer on top that every pointer lands
//! on, and the field a label is typed into.
//!
//! The picture is the drawing's own SVG, laid out and put in the page
//! inline ([`Canvas::picture`]): Blitz lays an inline `<svg>` out as it is
//! parsed, where an `<img>`'s `data:` URL is fetched a frame later and every
//! version of the file kept in its cache. The overlay is an `<svg>` of the canvas's size in
//! view points.

use dioxus::html::NamedKey;
use dioxus::html::input_data::MouseButton;
use dioxus::prelude::*;
use thorn_svg_core::{Drawing, Order};

use crate::canvas::{Canvas, Point};

/// The canvas a page holds: the model, and a count that moves whenever it
/// has changed, which is what a component reading it subscribes to. Copy,
/// so every handler takes it as it is.
#[derive(Clone, Copy, PartialEq)]
pub struct CanvasHandle {
    canvas: CopyValue<Canvas>,
    tick: Signal<u64>,
}

/// A canvas over `drawing`, made once for the component that calls it.
pub fn use_canvas(drawing: impl FnOnce() -> Drawing) -> CanvasHandle {
    let canvas = use_hook(|| CopyValue::new(Canvas::new(drawing())));
    let tick = use_signal(|| 0);
    CanvasHandle { canvas, tick }
}

impl CanvasHandle {
    /// A handle over a canvas made outside a component.
    pub fn new(canvas: Canvas) -> Self {
        Self {
            canvas: CopyValue::new(canvas),
            tick: Signal::new(0),
        }
    }

    /// Read the canvas, and be drawn again when it changes.
    pub fn read(&self) -> ReadableRef<'static, CopyValue<Canvas>> {
        self.tick.read();
        self.canvas.read_unchecked()
    }

    /// Read the canvas without subscribing: from a handler.
    pub fn peek(&self) -> ReadableRef<'static, CopyValue<Canvas>> {
        self.canvas.read_unchecked()
    }

    /// Change the canvas, and draw what reads it again.
    pub fn update<R>(&self, f: impl FnOnce(&mut Canvas) -> R) -> R {
        let mut canvas = self.canvas;
        let r = f(&mut canvas.write());
        let mut tick = self.tick;
        tick += 1;
        r
    }

    /// Replace the drawing, as opening another file does: a canvas of its
    /// own, in the appearance and size this one had.
    pub fn replace(&self, drawing: Drawing) {
        self.update(|c| {
            let mut next = Canvas::new(drawing);
            next.set_appearance(c.appearance());
            let (w, h) = c.size();
            next.set_size(w, h);
            *c = next;
        });
    }

    /// A key, as the canvas takes it: Delete and Backspace delete the
    /// selection; ⌘Z and ⇧⌘Z undo and redo (Control on other platforms,
    /// and Control-Y); ⌘G and ⇧⌘G group and ungroup; ⌘] and ⌘[ bring
    /// forward and send backward, with Shift to the front and back; ⌘+, ⌘−
    /// and ⌘0 zoom; ⌘A selects everything; Return opens the lone selected
    /// label; and a bare key is a tool's, the lock's or Escape's
    /// ([`Canvas::key`]). `true` when the key meant something. While a
    /// label's field is open its keys are the field's, and this takes none.
    pub fn key(&self, key: &Key, mods: Modifiers) -> bool {
        if self.peek().text_edit().is_some() {
            return false;
        }
        let command = mods.meta() || mods.ctrl();
        let shift = mods.shift();
        if mods.alt() {
            return false;
        }
        let action: Option<Command> = match key {
            Key::Named(NamedKey::Delete | NamedKey::Backspace) if !command => {
                Some(Box::new(Canvas::delete_selection))
            }
            Key::Named(NamedKey::Escape) => Some(Box::new(|c| {
                c.key('\u{1b}');
            })),
            Key::Named(NamedKey::Enter) if !command && self.peek().can_edit_text() => {
                Some(Box::new(Canvas::edit_selected_text))
            }
            Key::Character(s) if command => {
                let c = s.to_lowercase();
                match (c.as_str(), shift) {
                    ("z", false) => Some(Box::new(Canvas::undo)),
                    ("z", true) | ("y", false) => Some(Box::new(Canvas::redo)),
                    ("g", false) => Some(Box::new(Canvas::group_selection)),
                    ("g", true) => Some(Box::new(Canvas::ungroup_selection)),
                    ("]", false) => Some(Box::new(|c| c.reorder_selection(Order::Forward))),
                    ("]" | "}", true) => Some(Box::new(|c| c.reorder_selection(Order::ToFront))),
                    ("[", false) => Some(Box::new(|c| c.reorder_selection(Order::Backward))),
                    ("[" | "{", true) => Some(Box::new(|c| c.reorder_selection(Order::ToBack))),
                    ("=" | "+", _) => Some(Box::new(Canvas::zoom_in)),
                    ("-" | "_", _) => Some(Box::new(Canvas::zoom_out)),
                    ("0", false) => Some(Box::new(Canvas::zoom_to_fit)),
                    ("a", false) => Some(Box::new(Canvas::select_all)),
                    _ => None,
                }
            }
            Key::Character(s) if !command => {
                let mut chars = s.chars();
                match (chars.next(), chars.next()) {
                    (Some(ch), None)
                        if crate::Tool::for_key(ch).is_some()
                            || ch.eq_ignore_ascii_case(&crate::Tool::LOCK_KEY) =>
                    {
                        Some(Box::new(move |c| {
                            c.key(ch);
                        }))
                    }
                    _ => None,
                }
            }
            _ => None,
        };
        match action {
            Some(f) => {
                self.update(f);
                true
            }
            None => false,
        }
    }
}

/// What a key does to the canvas.
type Command = Box<dyn FnOnce(&mut Canvas)>;

/// The accent the selection is drawn in.
const ACCENT: &str = "#007aff";
/// What the eraser's sweep is outlined in.
const ERASING: &str = "#e53935";

/// The canvas, `width` × `height` CSS pixels: the host measures it, since
/// a handler cannot ask Blitz for an element's box.
#[component]
pub fn DrawingCanvas(canvas: CanvasHandle, width: f64, height: f64) -> Element {
    if canvas.peek().size() != (width, height) {
        // Not an edit, and nothing else reads the size: set without asking
        // for another render.
        let mut inner = canvas.canvas;
        inner.write().set_size(width, height);
    }
    let c = canvas.read();
    let fit = c.fit();
    let appearance = c.appearance();
    let page = fit.view_rect(c.page());
    let boxed = fit.view_rect(c.picture_box());
    let picture = c.picture().unwrap_or_default();
    let paper = c.paper();
    let overlay = c.overlay();
    let edit = c.text_edit().cloned();
    let hs = c.handle_size;
    let cursor = match c.tool {
        crate::Tool::Hand => "grab",
        crate::Tool::Select => "default",
        crate::Tool::Text => "text",
        _ => "crosshair",
    };
    drop(c);

    let at = |e: &PointerEvent| -> Point {
        let p = e.element_coordinates();
        (p.x, p.y)
    };
    let handle_rect = move |(x, y): Point| (x - hs / 2.0, y - hs / 2.0);

    rsx! {
        div {
            class: "thorn-canvas",
            style: "width: {width}px; height: {height}px; background: {appearance.desk()}",
            div {
                class: "thorn-sheet",
                style: "left: {page.x}px; top: {page.y}px; width: {page.width}px; height: {page.height}px; background: {paper}",
            }
            div {
                class: "thorn-picture",
                style: "left: {boxed.x}px; top: {boxed.y}px; width: {boxed.width}px; height: {boxed.height}px",
                dangerous_inner_html: "{picture}",
            }
            svg {
                class: "thorn-overlay",
                width: "{width}",
                height: "{height}",
                view_box: "0 0 {width} {height}",
                for b in overlay.boxes.iter() {
                    rect { x: "{b.x}", y: "{b.y}", width: "{b.width}", height: "{b.height}", fill: "none", stroke: ACCENT, stroke_width: "1" }
                }
                for p in overlay.handles.iter() {
                    {
                        let (x, y) = handle_rect(*p);
                        rsx! { rect { x: "{x}", y: "{y}", width: "{hs}", height: "{hs}", fill: paper, stroke: ACCENT, stroke_width: "1" } }
                    }
                }
                if let Some((d, ends, mid)) = overlay.connector.as_ref() {
                    path { d: "{d}", fill: "none", stroke: ACCENT, stroke_width: "1" }
                    for (p, bound) in ends.iter() {
                        {
                            let (x, y) = handle_rect(*p);
                            let fill = if *bound { ACCENT } else { paper };
                            rsx! { rect { x: "{x}", y: "{y}", width: "{hs}", height: "{hs}", fill, stroke: ACCENT, stroke_width: "1" } }
                        }
                    }
                    circle { cx: "{mid.0}", cy: "{mid.1}", r: "{hs / 2.0}", fill: paper, stroke: ACCENT, stroke_width: "1" }
                }
                if let Some(d) = overlay.ghost.as_ref() {
                    path { d: "{d}", fill: "none", stroke: ACCENT, stroke_width: "1", stroke_dasharray: "4 3" }
                }
                if let Some((points, colour, w)) = overlay.ink.as_ref() {
                    polyline { points: "{points}", fill: "none", stroke: "{colour}", stroke_width: "{w}", stroke_linecap: "round", stroke_linejoin: "round" }
                }
                for b in overlay.erasing.iter() {
                    rect { x: "{b.x}", y: "{b.y}", width: "{b.width}", height: "{b.height}", fill: "none", stroke: ERASING, stroke_width: "1", stroke_dasharray: "4 3" }
                }
            }
            // On top, so every press lands here and its coordinates are the
            // canvas's own.
            div {
                class: "thorn-touch",
                style: "cursor: {cursor}",
                onpointerdown: move |e: PointerEvent| {
                    if e.trigger_button() != Some(MouseButton::Primary) {
                        return;
                    }
                    let p = at(&e);
                    let shift = e.modifiers().shift();
                    canvas.update(|c| c.pointer_down(p, shift));
                },
                onpointermove: move |e: PointerEvent| {
                    if canvas.peek().dragging() {
                        let p = at(&e);
                        canvas.update(|c| c.pointer_moved(p));
                    }
                },
                onpointerup: move |_| {
                    if canvas.peek().dragging() {
                        canvas.update(Canvas::pointer_up);
                    }
                },
                onpointerleave: move |_| {
                    if canvas.peek().dragging() {
                        canvas.update(Canvas::pointer_up);
                    }
                },
                onpointercancel: move |_| canvas.update(Canvas::cancel_pointer),
                ondoubleclick: move |e: MouseEvent| {
                    let p = e.element_coordinates();
                    canvas.update(|c| c.double_click((p.x, p.y)));
                },
                onwheel: move |e: WheelEvent| {
                    let d = e.delta().strip_units();
                    let lines = match e.delta() {
                        dioxus::html::geometry::WheelDelta::Pixels(_) => 1.0,
                        dioxus::html::geometry::WheelDelta::Lines(_) => 20.0,
                        dioxus::html::geometry::WheelDelta::Pages(_) => height,
                    };
                    let (dx, dy) = (d.x * lines, d.y * lines);
                    let m = e.modifiers();
                    if m.meta() || m.ctrl() {
                        let p = e.element_coordinates();
                        canvas.update(|c| c.zoom_by((-dy / 200.0).exp(), (p.x, p.y)));
                    } else {
                        canvas.update(|c| c.pan_by(-dx, -dy));
                    }
                },
            }
            if let Some(edit) = edit {
                LabelField { canvas, edit }
            }
        }
    }
}

/// The field a label is typed into, over where the label is: Return lands
/// it, Shift-Return breaks the line, Escape leaves the label as it was,
/// and a click away lands it.
#[component]
fn LabelField(canvas: CanvasHandle, edit: crate::canvas::TextEdit) -> Element {
    let mut text = use_signal(|| edit.text.clone());
    let (x, y, w, h) = edit.frame;
    let family = edit
        .font_family
        .clone()
        .map(|f| format!("font-family: \"{f}\";"))
        .unwrap_or_default();
    let wrap = if edit.wraps { "pre-wrap" } else { "pre" };
    rsx! {
        textarea {
            class: "thorn-field",
            style: "left: {x}px; top: {y}px; width: {w.max(40.0)}px; height: {h.max(edit.font_size * 1.3)}px; font-size: {edit.font_size}px; line-height: 1.2; white-space: {wrap}; {family}",
            autofocus: true,
            value: "{text}",
            oninput: move |e: FormEvent| text.set(e.value()),
            onkeydown: move |e: KeyboardEvent| {
                match e.key() {
                    Key::Named(NamedKey::Enter) if !e.modifiers().shift() => {
                        e.prevent_default();
                        let t = text.peek().clone();
                        canvas.update(|c| c.commit_text_edit(Some(&t)));
                    }
                    Key::Named(NamedKey::Escape) => {
                        e.prevent_default();
                        canvas.update(|c| c.commit_text_edit(None));
                    }
                    _ => {}
                }
            },
            onblur: move |_| {
                if canvas.peek().text_edit().is_some() {
                    let t = text.peek().clone();
                    canvas.update(|c| c.commit_text_edit(Some(&t)));
                }
            },
        }
    }
}

/// The canvas's styles, for the page to put in a `<style>` once.
pub const CANVAS_CSS: &str = r#"
.thorn-canvas { position: relative; overflow: hidden; }
.thorn-sheet, .thorn-picture, .thorn-overlay, .thorn-touch, .thorn-field { position: absolute; }
.thorn-sheet { box-shadow: 0 1px 4px rgba(0, 0, 0, 0.18); }
.thorn-picture svg { display: block; width: 100%; height: 100%; }
.thorn-overlay { left: 0; top: 0; pointer-events: none; }
.thorn-touch { left: 0; top: 0; right: 0; bottom: 0; }
.thorn-field { margin: 0; padding: 0; border: 1px dashed #007aff; background: transparent; color: inherit; outline: none; resize: none; overflow: hidden; box-sizing: content-box; }
"#;
