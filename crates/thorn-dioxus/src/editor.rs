//! The canvas with its tools floating over it, in Excalidraw's shape: the
//! lock, the hand and select; the shapes; the marks; a menu of the layering
//! and grouping commands, delete, undo and redo; and the zoom — out, the
//! percentage (which fits the page again), in — at the bottom right. Under
//! the tools, when there is something for it to say, the options: a
//! shape's words, for the selection when there is one and for the tool
//! when there is not — the palette and a background on one row, the
//! stroke's weight and dash, a box's corners and an arrow's heads on the
//! next.
//!
//! Every control is unit's, and refuses the keys: a press leaves them with
//! the canvas, as a Mac's toolbar buttons do.

use dioxus::prelude::*;
use thorn_svg_core::{Dash, Heads, Hue, Order, Weight};
use unit::{Entry, Item, MenuButton, Swatch, SwatchAction, Swatches, ToggleButton};

use crate::canvas::{Appearance, ROUND_CORNER};
use crate::{CanvasHandle, DrawingCanvas, Tool, icons};

/// The editor, `width` × `height` CSS pixels.
#[component]
pub fn DrawingEditor(canvas: CanvasHandle, width: f64, height: f64) -> Element {
    rsx! {
        div { class: "thorn-editor", style: "width: {width}px; height: {height}px",
            DrawingCanvas { canvas, width, height }
            div { class: "thorn-chrome",
                Tools { canvas }
                Options { canvas }
            }
            Zoom { canvas }
        }
    }
}

/// A capsule of tiles.
#[component]
fn Cluster(children: Element) -> Element {
    rsx! { div { class: "thorn-cluster", {children} } }
}

/// A tile: an icon that lights when `on`.
#[component]
fn Tile(
    label: String,
    icon: &'static str,
    on: bool,
    #[props(default)] disabled: bool,
    onpress: EventHandler<()>,
) -> Element {
    rsx! {
        ToggleButton {
            class: "unit-icon thorn-tile",
            pressed: on,
            disabled,
            refuses_keys: true,
            label,
            icon,
            onchange: move |_| onpress.call(()),
        }
    }
}

/// What a tool's tile says: its name and its keys.
fn tool_label(tool: Tool) -> String {
    let keys: Vec<String> = tool
        .keys()
        .iter()
        .map(|k| k.to_uppercase().to_string())
        .collect();
    format!("{} — {}", tool.name(), keys.join(" or "))
}

#[component]
fn Tools(canvas: CanvasHandle) -> Element {
    let c = canvas.read();
    let (tool, locked) = (c.tool, c.locked);
    let selected = !c.selection().is_empty();
    let (can_group, can_ungroup, lone) = (c.can_group(), c.can_ungroup(), c.selection().len() == 1);
    drop(c);
    let tile = move |t: Tool| {
        rsx! {
            Tile {
                key: "{t:?}",
                label: tool_label(t),
                icon: t.icon(),
                on: tool == t,
                onpress: move |_| canvas.update(|c| c.tool = t),
            }
        }
    };
    let command = if cfg!(target_os = "macos") {
        "⌘"
    } else {
        "Ctrl+"
    };
    let layers = vec![
        Entry::from(
            Item::new(0, "Bring to front")
                .shortcut(format!("⇧{command}]"))
                .enabled(lone),
        ),
        Item::new(1, "Bring forward")
            .shortcut(format!("{command}]"))
            .enabled(lone)
            .into(),
        Item::new(2, "Send backward")
            .shortcut(format!("{command}["))
            .enabled(lone)
            .into(),
        Item::new(3, "Send to back")
            .shortcut(format!("⇧{command}["))
            .enabled(lone)
            .into(),
        Entry::Separator,
        Item::new(4, "Group")
            .shortcut(format!("{command}G"))
            .enabled(can_group)
            .into(),
        Item::new(5, "Ungroup")
            .shortcut(format!("⇧{command}G"))
            .enabled(can_ungroup)
            .into(),
    ];
    rsx! {
        div { class: "thorn-row",
            Cluster {
                Tile {
                    label: "Keep the tool after drawing — Q",
                    icon: if locked { icons::LOCK } else { icons::UNLOCK },
                    on: locked,
                    onpress: move |_| canvas.update(|c| c.locked = !c.locked),
                }
                {tile(Tool::Hand)}
                {tile(Tool::Select)}
            }
            Cluster {
                for t in [Tool::Rect, Tool::Diamond, Tool::Ellipse, Tool::Arrow, Tool::Line] {
                    {tile(t)}
                }
            }
            Cluster {
                for t in [Tool::Draw, Tool::Text, Tool::Note, Tool::Eraser] {
                    {tile(t)}
                }
            }
            Cluster {
                MenuButton {
                    class: "unit-icon thorn-tile",
                    label: "Layers and groups",
                    icon: icons::LAYERS,
                    refuses_keys: true,
                    disabled: !selected,
                    entries: layers,
                    onpick: move |i: usize| canvas.update(|c| match i {
                        0 => c.reorder_selection(Order::ToFront),
                        1 => c.reorder_selection(Order::Forward),
                        2 => c.reorder_selection(Order::Backward),
                        3 => c.reorder_selection(Order::ToBack),
                        4 => c.group_selection(),
                        _ => c.ungroup_selection(),
                    }),
                }
                Tile {
                    label: "Delete",
                    icon: unit::icon::paths::TRASH,
                    on: false,
                    disabled: !selected,
                    onpress: move |_| canvas.update(|c| c.delete_selection()),
                }
                Tile {
                    label: "Undo",
                    icon: unit::icon::paths::UNDO,
                    on: false,
                    onpress: move |_| canvas.update(|c| c.undo()),
                }
                Tile {
                    label: "Redo",
                    icon: icons::REDO,
                    on: false,
                    onpress: move |_| canvas.update(|c| c.redo()),
                }
            }
        }
    }
}

/// The hue's name, read out for its swatch.
fn hue_name(h: Hue) -> &'static str {
    match h {
        Hue::Red => "Red",
        Hue::Orange => "Orange",
        Hue::Yellow => "Yellow",
        Hue::Green => "Green",
        Hue::Blue => "Blue",
        Hue::Violet => "Violet",
        Hue::Pink => "Pink",
        Hue::Grey => "Grey",
    }
}

/// Where `value` is among `choices`, for a choice the shapes agree on.
fn chosen<T: PartialEq>(value: Option<T>, choices: &[T]) -> Option<usize> {
    value.and_then(|v| choices.iter().position(|c| *c == v))
}

#[component]
fn Options(canvas: CanvasHandle) -> Element {
    let c = canvas.read();
    let offers = c.offers();
    let dark = c.appearance() == Appearance::Dark;
    let (color, fill, dash, weight, corner, heads) = (
        c.color(),
        c.fill(),
        c.dash(),
        c.weight(),
        c.corner(),
        c.arrowheads(),
    );
    drop(c);
    if !(offers.color || offers.fill || offers.stroke || offers.corner || offers.heads) {
        return rsx! {};
    }
    let ink = if dark { "#e6e6e6" } else { "#222222" };
    let hues: Vec<Option<Hue>> = Hue::ALL.iter().copied().map(Some).collect();
    let mut strokes = vec![Swatch::new("The drawing's ink", ink)];
    strokes.extend(
        Hue::ALL
            .iter()
            .map(|&h| Swatch::new(hue_name(h), h.stroke(dark))),
    );
    let tints: Vec<Swatch> = Hue::ALL
        .iter()
        .map(|&h| {
            Swatch::new(format!("{} background", hue_name(h)), h.tint(dark)).edge(h.stroke(dark))
        })
        .collect();
    let mut stroke_choices = vec![None];
    stroke_choices.extend(hues.iter().copied());
    let weights = [
        (Some(Weight::Thin), "Thin", 1.0),
        (None, "Regular", 2.0),
        (Some(Weight::Bold), "Bold", 4.0),
    ];
    let dashes = [
        (None, "Solid", icons::SOLID),
        (Some(Dash::Dashed), "Dashed", icons::DASHED),
        (Some(Dash::Dotted), "Dotted", icons::DOTTED),
    ];
    let corners = [
        (None, "Sharp corners", icons::SHARP),
        (Some(ROUND_CORNER), "Round corners", icons::ROUND),
    ];
    let arrowheads = [
        (None, "No arrowhead", icons::SOLID),
        (Some(Heads::End), "Arrowhead at the end", icons::HEAD_END),
        (
            Some(Heads::Start),
            "Arrowhead at the start",
            icons::HEAD_START,
        ),
        (
            Some(Heads::Both),
            "Arrowheads at both ends",
            icons::HEAD_BOTH,
        ),
    ];
    rsx! {
        if offers.color || offers.fill {
            div { class: "thorn-row",
                if offers.color {
                    Cluster {
                        Swatches {
                            label: "Colour",
                            swatches: strokes,
                            selected: chosen(color, &stroke_choices),
                            onchange: move |i: usize| canvas.update(|c| c.set_color(if i == 0 { None } else { Some(Hue::ALL[i - 1]) })),
                        }
                    }
                }
                if offers.fill {
                    Cluster {
                        Swatches {
                            label: "Background",
                            swatches: tints,
                            selected: fill.flatten().and_then(|h| Hue::ALL.iter().position(|x| *x == h)),
                            onchange: move |i: usize| canvas.update(|c| c.set_fill(Some(Hue::ALL[i]))),
                            SwatchAction {
                                label: "No background",
                                icon: unit::icon::paths::CLOSE,
                                on: fill == Some(None),
                                onpress: move |_| canvas.update(|c| c.set_fill(None)),
                            }
                        }
                    }
                }
            }
        }
        if offers.stroke || offers.corner || offers.heads {
            div { class: "thorn-row",
                if offers.stroke {
                    Cluster {
                        for (w, name, px) in weights {
                            ToggleButton {
                                key: "{name}",
                                class: "unit-icon thorn-tile",
                                pressed: weight == Some(w),
                                refuses_keys: true,
                                label: name,
                                onchange: move |_| canvas.update(|c| c.set_weight(w)),
                                svg { width: "18", height: "18", view_box: "0 0 24 24", "aria-hidden": "true",
                                    path { d: "M4 12h16", stroke: "currentColor", stroke_width: "{px * 1.5}", stroke_linecap: "round" }
                                }
                            }
                        }
                    }
                    Cluster {
                        for (d, name, icon) in dashes {
                            Tile { key: "{name}", label: name, icon, on: dash == Some(d), onpress: move |_| canvas.update(|c| c.set_dash(d)) }
                        }
                    }
                }
                if offers.corner {
                    Cluster {
                        for (r, name, icon) in corners {
                            Tile { key: "{name}", label: name, icon, on: corner == Some(r), onpress: move |_| canvas.update(|c| c.set_corner(r)) }
                        }
                    }
                }
                if offers.heads {
                    Cluster {
                        for (h, name, icon) in arrowheads {
                            Tile { key: "{name}", label: name, icon, on: heads == Some(h), onpress: move |_| canvas.update(|c| c.set_heads(h)) }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn Zoom(canvas: CanvasHandle) -> Element {
    let zoom = canvas.read().zoom();
    let percent = (zoom * 100.0).round();
    rsx! {
        div { class: "thorn-zoom",
            Cluster {
                Tile { label: "Zoom out", icon: unit::icon::paths::MINUS, on: false, onpress: move |_| canvas.update(|c| c.zoom_out()) }
                unit::Button {
                    class: "thorn-percent",
                    refuses_keys: true,
                    label: "Fit the drawing",
                    onpress: move |_| canvas.update(|c| c.zoom_to_fit()),
                    "{percent}%"
                }
                Tile { label: "Zoom in", icon: unit::icon::paths::PLUS, on: false, onpress: move |_| canvas.update(|c| c.zoom_in()) }
            }
        }
    }
}

/// The editor's styles, the canvas's among them, for the page to put in a
/// `<style>` once.
pub fn css() -> String {
    format!("{}{EDITOR_CSS}", crate::view::CANVAS_CSS)
}

const EDITOR_CSS: &str = r#"
.thorn-editor { position: relative; overflow: hidden; }
.thorn-chrome { position: absolute; left: 0; right: 0; top: 10px; display: flex; flex-direction: column; align-items: center; gap: 8px; pointer-events: none; }
.thorn-row { display: flex; flex-direction: row; gap: 8px; align-items: center; justify-content: center; flex-wrap: wrap; }
.thorn-cluster { pointer-events: auto; display: flex; flex-direction: row; gap: 2px; align-items: center; padding: 4px; border-radius: 10px; background: var(--unit-surface); border: 1px solid var(--unit-border); box-shadow: 0 1px 4px rgba(0, 0, 0, 0.12); }
.thorn-cluster .unit-swatches { padding: 2px 4px; }
.thorn-tile.unit-button { border-color: transparent; background: transparent; }
.thorn-tile.unit-toggle.on { background: var(--unit-accent); color: var(--unit-accent-text); border-color: var(--unit-accent); }
.thorn-tile.unit-toggle.on svg { color: var(--unit-accent-text); }
.thorn-zoom { position: absolute; right: 10px; bottom: 10px; }
.thorn-percent.unit-button { min-width: 56px; border-color: transparent; background: transparent; }
"#;
