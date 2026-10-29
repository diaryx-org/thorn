//! What a canvas does, with no view in it: the tool, the selection, the
//! drag in flight, the mapping between view points and user units, and
//! what is drawn over the picture. [`DrawingCanvas`](crate::DrawingCanvas)
//! is thin over this, and it is testable with no window.
//!
//! During a drag the canvas keeps the in-flight geometry and applies it to
//! the drawing as the pointer moves — a shape moved, resized, dragged by an
//! end, or being created — so the picture follows the hand; each
//! application is undone before the next, so that when the pointer lifts
//! the gesture is one undo step, as if it had landed once.
//!
//! A view point is a CSS pixel from the canvas's top-left corner.

use thorn_svg_core::{
    Bounds, Dash, Drawing, End, Handle, Heads, Hue, NOTE_PAD, Order, ShapeKind, Weight,
    measure::Usvg,
};

use crate::Tool;

/// A point, in view points or in user units as the name says.
pub type Point = (f64, f64);

/// Light or dark: the greys the canvas draws the desk, the sheet and the
/// handles in, and whether the picture is drawn with the file's own rules
/// for a dark page. The file is untouched either way.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Appearance {
    #[default]
    Light,
    Dark,
}

impl Appearance {
    /// The sheet the page is drawn on.
    pub fn sheet(self) -> &'static str {
        match self {
            Appearance::Light => "#ffffff",
            Appearance::Dark => "#1f1f1f",
        }
    }

    /// What is round the page.
    pub fn desk(self) -> &'static str {
        match self {
            Appearance::Light => "#f0f0f0",
            Appearance::Dark => "#121212",
        }
    }
}

/// A label being typed: what the view puts a text field over. `id` is the
/// label being re-worded, or `None` for a new one at `anchor`, in user
/// units; `frame` (x, y, width, height) and `font_size` are in view points,
/// where the field goes. `wraps` says the label has a width and the field
/// should wrap at the frame's.
#[derive(Clone, Debug, PartialEq)]
pub struct TextEdit {
    pub id: Option<String>,
    pub anchor: Point,
    pub text: String,
    pub frame: (f64, f64, f64, f64),
    pub font_size: f64,
    pub font_family: Option<String>,
    pub wraps: bool,
}

/// User units → view points: `view = user * scale + offset`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fit {
    pub scale: f64,
    pub offset: Point,
}

impl Fit {
    pub fn view(&self, (x, y): Point) -> Point {
        (
            x * self.scale + self.offset.0,
            y * self.scale + self.offset.1,
        )
    }

    pub fn user(&self, (x, y): Point) -> Point {
        (
            (x - self.offset.0) / self.scale,
            (y - self.offset.1) / self.scale,
        )
    }

    pub fn view_rect(&self, b: Bounds) -> Bounds {
        let (x, y) = self.view((b.x, b.y));
        Bounds {
            x,
            y,
            width: b.width * self.scale,
            height: b.height * self.scale,
        }
    }

    /// `boxed` fitted into a `w` × `h` view, uniformly scaled and centred on
    /// the leftover axis — resvg's own `xMidYMid meet`. The identity when
    /// either is empty.
    fn of(boxed: Bounds, w: f64, h: f64) -> Fit {
        if boxed.width <= 0.0 || boxed.height <= 0.0 || w <= 0.0 || h <= 0.0 {
            return Fit {
                scale: 1.0,
                offset: (0.0, 0.0),
            };
        }
        let scale = (w / boxed.width).min(h / boxed.height);
        let ox = (w - boxed.width * scale) / 2.0;
        let oy = (h - boxed.height * scale) / 2.0;
        Fit {
            scale,
            offset: (ox - boxed.x * scale, oy - boxed.y * scale),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Drag {
    /// The selection moving as one.
    Move {
        ids: Vec<String>,
        delta: Point,
    },
    Resize {
        id: String,
        handle: Handle,
        start: Bounds,
        delta: Point,
    },
    /// An end of a connector being dragged.
    Endpoint {
        id: String,
        end: End,
        to: Point,
    },
    /// A connector's midpoint handle being dragged: the bend.
    Bend {
        id: String,
        to: Point,
    },
    Create {
        from: Point,
        to: Point,
    },
    /// The draw tool: the centreline so far, in user units.
    Ink {
        points: Vec<Point>,
    },
    /// The hand: `start` is `pan` when the drag began, `from` the view
    /// point it began at.
    Pan {
        start: Point,
        from: Point,
    },
    /// The eraser: what it has passed over, deleted when it lifts.
    Erase {
        ids: Vec<String>,
    },
}

/// A lone selected connector as the overlay draws it: its path data, its
/// ends (each with whether it is bound to a shape), and its bend handle.
pub type ConnectorHandles = (String, [(Point, bool); 2], Point);

/// What is drawn over the picture, in view points, for a view to paint.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Overlay {
    /// Each selected shape's box, but a lone connector's.
    pub boxes: Vec<Bounds>,
    /// The resize handles' centres.
    pub handles: Vec<Point>,
    /// A lone selected connector, as path data, with its ends' handles
    /// (filled when bound to a shape) and its bend handle.
    pub connector: Option<ConnectorHandles>,
    /// A shape being created, before it is big enough to add: path data.
    pub ghost: Option<String>,
    /// A stroke in flight, as polyline points, its colour and width.
    pub ink: Option<(String, String, f64)>,
    /// What the eraser has passed over.
    pub erasing: Vec<Bounds>,
}

/// The canvas as a value: everything a view needs to draw and to answer a
/// pointer, and nothing about the platform.
pub struct Canvas {
    drawing: Drawing,
    /// What the next pointer-down does. A one-shot tool falls back to
    /// select by itself.
    pub tool: Tool,
    /// Keep a create tool after the shape it made, rather than falling back
    /// to select: Excalidraw's lock.
    pub locked: bool,
    /// Which ends of the next arrow have a head; `None` draws a plain line.
    /// The canvas's, never the file's, until a shape is drawn with it.
    pub heads: Option<Heads>,
    appearance: Appearance,
    /// How far, in view points, a click may miss a stroke or a handle: a
    /// mouse's few, a finger's dozen.
    pub tolerance: f64,
    /// The side of a handle as drawn, in view points.
    pub handle_size: f64,
    /// The width of a stroke the draw tool makes, in user units.
    pub ink_width: f64,
    /// How far the picture has been dragged from where it fits, in view
    /// points.
    pan: Point,
    /// How much larger than fitted the picture is drawn.
    zoom: f64,
    /// The box that `zoom == 1` fits to the view: the page as it stood when
    /// the drawing was opened or last zoomed to fit. The page follows the
    /// shapes, and a page that grows under a drag must not move what is on
    /// screen, so the mapping is pinned to this; `zoom_to_fit` is where it
    /// catches up.
    fitted: Bounds,
    /// The view's size, in view points.
    size: (f64, f64),
    selection: Vec<String>,
    drag: Option<Drag>,
    /// Whether the drag in flight has been applied to the drawing, and so
    /// must be undone before it is applied again or dropped.
    previewed: bool,
    /// The shape a create drag last added.
    created: Option<String>,
    /// The view point the drag began at.
    start: Option<Point>,
    /// The selection before the pointer went down, for `cancel_pointer`.
    selection_before: Option<Vec<String>>,
    /// The field open for a label, if one is.
    text_edit: Option<TextEdit>,
    /// Moves on every edit that lands in the drawing, a preview's included:
    /// the host's cue that `source` has changed.
    revision: u64,
}

/// The least and the most the picture is drawn at: Excalidraw's 10% to
/// 3000%.
pub const ZOOM_RANGE: (f64, f64) = (0.1, 30.0);
/// What a zoom command zooms by: √2, so two of them double.
pub const ZOOM_STEP: f64 = std::f64::consts::SQRT_2;
/// The radius a rounded box takes: Excalidraw's round edges, in user units.
pub const ROUND_CORNER: f64 = 8.0;
/// The ink a stroke in flight is drawn in with no hue picked, on a light
/// page and a dark one: the template's.
const INK: (&str, &str) = ("#222222", "#e6e6e6");

impl Canvas {
    /// A canvas over `drawing`, which it lends resvg's text layout so a
    /// label's box is the one the picture draws.
    pub fn new(mut drawing: Drawing) -> Self {
        drawing.set_measure(Box::new(Usvg));
        let fitted = sheet(&drawing);
        Self {
            drawing,
            tool: Tool::Select,
            locked: false,
            heads: Some(Heads::End),
            appearance: Appearance::Light,
            tolerance: 4.0,
            handle_size: 8.0,
            ink_width: 3.0,
            pan: (0.0, 0.0),
            zoom: 1.0,
            fitted,
            size: (0.0, 0.0),
            selection: Vec::new(),
            drag: None,
            previewed: false,
            created: None,
            start: None,
            selection_before: None,
            text_edit: None,
            revision: 0,
        }
    }

    pub fn drawing(&self) -> &Drawing {
        &self.drawing
    }

    /// The bytes as they stand: what saving writes.
    pub fn source(&self) -> &str {
        self.drawing.source()
    }

    /// Moves on every edit that lands in the drawing.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn selection(&self) -> &[String] {
        &self.selection
    }

    pub fn zoom(&self) -> f64 {
        self.zoom
    }

    pub fn appearance(&self) -> Appearance {
        self.appearance
    }

    pub fn set_appearance(&mut self, appearance: Appearance) {
        self.appearance = appearance;
    }

    /// Whether the picture is drawn with the file's own rules for a dark
    /// page: in a dark appearance, for a file that keeps some.
    pub fn draws_dark(&self) -> bool {
        self.appearance == Appearance::Dark && self.has_dark_rules()
    }

    fn has_dark_rules(&self) -> bool {
        !self.drawing.dark_rules().is_empty()
    }

    /// What the page is drawn on: the appearance's sheet, but a light one
    /// for a file that keeps no rules for a dark page, whose ink is meant
    /// for a light one and would be lost on a dark sheet.
    pub fn paper(&self) -> &'static str {
        if self.has_dark_rules() {
            self.appearance.sheet()
        } else {
            Appearance::Light.sheet()
        }
    }

    /// The field open for a label, if one is.
    pub fn text_edit(&self) -> Option<&TextEdit> {
        self.text_edit.as_ref()
    }

    /// Set the view's size, in view points.
    pub fn set_size(&mut self, width: f64, height: f64) {
        self.size = (width, height);
    }

    pub fn size(&self) -> (f64, f64) {
        self.size
    }

    // ----- coordinates -------------------------------------------------

    /// User units → view points: `fitted` fitted into the view, then zoomed
    /// about the view's origin and slid by `pan`.
    pub fn fit(&self) -> Fit {
        let base = Fit::of(self.fitted, self.size.0, self.size.1);
        Fit {
            scale: base.scale * self.zoom,
            offset: (
                base.offset.0 * self.zoom + self.pan.0,
                base.offset.1 * self.zoom + self.pan.1,
            ),
        }
    }

    fn user_tolerance(&self) -> f64 {
        self.tolerance / self.fit().scale
    }

    /// The box, in user units, the picture is drawn over: the page, and
    /// every shape off its edge, so a shape dragged off the page is seen
    /// taking the page with it.
    pub fn picture_box(&self) -> Bounds {
        let page = sheet(&self.drawing);
        match self.drawing.extent() {
            Some(e) => page.union(&e.expanded(thorn_svg_core::PAGE_MARGIN)),
            None => page,
        }
    }

    /// The page, in user units, the sheet is drawn under.
    pub fn page(&self) -> Bounds {
        sheet(&self.drawing)
    }

    /// The picture as the view paints it: the file over
    /// [`Canvas::picture_box`], with its dark rules when it draws dark, laid
    /// out by usvg and written back resolved — every rule of its stylesheet
    /// applied as an attribute, every label an outline — so that no
    /// `<style>` reaches the page, where Blitz would apply it to everything
    /// (the toolbar's icons, the selection's handles). `None` for a file usvg
    /// cannot lay out.
    pub fn picture(&self) -> Option<String> {
        let svg = self.drawing.picture(self.picture_box(), self.draws_dark());
        let options = usvg::Options {
            fontdb: thorn_svg_core::measure::fonts(),
            ..usvg::Options::default()
        };
        let tree = usvg::Tree::from_str(&svg, &options).ok()?;
        Some(tree.to_string(&usvg::WriteOptions {
            id_prefix: Some("thorn-".into()),
            ..usvg::WriteOptions::default()
        }))
    }

    // ----- zoom and pan ------------------------------------------------

    /// Zoom by `factor` about a view point, which stays over the same user
    /// point: a pinch's increment, or a wheel's with the command key.
    pub fn zoom_by(&mut self, factor: f64, about: Point) {
        self.set_zoom(self.zoom * factor, about);
    }

    /// Zoom to `zoom` about a view point, which stays over the same user
    /// point. Clamped to [`ZOOM_RANGE`].
    pub fn set_zoom(&mut self, zoom: f64, about: Point) {
        let clamped = zoom.clamp(ZOOM_RANGE.0, ZOOM_RANGE.1);
        if clamped == self.zoom {
            return;
        }
        // The view point is `zoom * v₀ + pan` for its unzoomed point `v₀`;
        // scaling by `k` keeps it there when `pan` moves to
        // `c - k * (c - pan)`.
        let k = clamped / self.zoom;
        self.pan = (
            about.0 - k * (about.0 - self.pan.0),
            about.1 - k * (about.1 - self.pan.1),
        );
        self.zoom = clamped;
    }

    /// One step in, about the middle of the view: ⌘+.
    pub fn zoom_in(&mut self) {
        self.zoom_by(ZOOM_STEP, self.middle());
    }

    /// One step out: ⌘−.
    pub fn zoom_out(&mut self) {
        self.zoom_by(1.0 / ZOOM_STEP, self.middle());
    }

    fn middle(&self) -> Point {
        (self.size.0 / 2.0, self.size.1 / 2.0)
    }

    /// Back to the page fitted in the view, un-panned: ⌘0. The page as it
    /// stands now, which is around the shapes as they stand now.
    pub fn zoom_to_fit(&mut self) {
        self.fitted = sheet(&self.drawing);
        self.zoom = 1.0;
        self.pan = (0.0, 0.0);
    }

    /// Slide the picture by a view vector: a scroll's doing.
    pub fn pan_by(&mut self, dx: f64, dy: f64) {
        self.pan = (self.pan.0 + dx, self.pan.1 + dy);
    }

    // ----- what is drawn over the picture ------------------------------

    /// Whether a lone selected shape can be resized by its box handles: a
    /// connector is dragged by its ends and its bend instead.
    fn resizable(&self) -> Option<&str> {
        match self.selection.as_slice() {
            [id] if self.drawing.shape(id).is_some() && self.drawing.connector(id).is_none() => {
                Some(id)
            }
            _ => None,
        }
    }

    /// The selection, the drag in flight and the eraser's sweep, in view
    /// points.
    pub fn overlay(&self) -> Overlay {
        let fit = self.fit();
        let mut out = Overlay::default();
        for id in &self.selection {
            if self.selection.len() == 1
                && let Some(c) = self.drawing.connector(id)
            {
                let (from, to) = (fit.view(c.from), fit.view(c.to));
                let d = match c.control {
                    Some(q) => {
                        let q = fit.view(q);
                        format!("M{} {}Q{} {} {} {}", from.0, from.1, q.0, q.1, to.0, to.1)
                    }
                    None => format!("M{} {}L{} {}", from.0, from.1, to.0, to.1),
                };
                let bound = |end: End| {
                    self.drawing
                        .shape(id)
                        .and_then(|s| s.attr(end.binding()))
                        .is_some()
                };
                out.connector = Some((
                    d,
                    [(from, bound(End::From)), (to, bound(End::To))],
                    fit.view(c.midpoint()),
                ));
                continue;
            }
            let Some(b) = self.drawing.bounds(id) else {
                continue;
            };
            let r = fit.view_rect(b);
            out.boxes.push(r);
            if self.resizable() == Some(id.as_str()) {
                out.handles = Handle::ALL.iter().map(|h| h.position(r)).collect();
            }
        }
        match &self.drag {
            Some(Drag::Create { from, to }) if !self.previewed => {
                let (a, b) = (fit.view(*from), fit.view(*to));
                let r = between(a, b);
                let (x0, y0, x1, y1) = (r.x, r.y, r.x + r.width, r.y + r.height);
                let (xm, ym) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
                out.ghost = Some(match self.tool {
                    Tool::Line | Tool::Arrow => format!("M{} {}L{} {}", a.0, a.1, b.0, b.1),
                    Tool::Diamond => {
                        format!("M{xm} {y0}L{x1} {ym}L{xm} {y1}L{x0} {ym}Z")
                    }
                    Tool::Ellipse => {
                        let (rx, ry) = (r.width / 2.0, r.height / 2.0);
                        format!("M{x0} {ym}A{rx} {ry} 0 1 0 {x1} {ym}A{rx} {ry} 0 1 0 {x0} {ym}Z")
                    }
                    _ => format!("M{x0} {y0}H{x1}V{y1}H{x0}Z"),
                });
            }
            Some(Drag::Ink { points }) if !self.previewed => {
                // Not in the file yet — it lands once, as it lifts — so it
                // is drawn as the nib's line along the centreline, in the
                // pen's hue or the template's ink.
                let dark = self.draws_dark();
                let colour = self
                    .drawing
                    .pen()
                    .hue
                    .map(|h| h.stroke(dark).to_string())
                    .unwrap_or_else(|| if dark { INK.1 } else { INK.0 }.to_string());
                let mut pts: Vec<Point> = points.iter().map(|p| fit.view(*p)).collect();
                if pts.len() == 1 {
                    pts.push(pts[0]);
                }
                let list = pts
                    .iter()
                    .map(|(x, y)| format!("{x:.1},{y:.1}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                out.ink = Some((list, colour, self.ink_width * fit.scale));
            }
            Some(Drag::Erase { ids }) => {
                out.erasing = ids
                    .iter()
                    .filter_map(|id| self.drawing.bounds(id))
                    .map(|b| fit.view_rect(b))
                    .collect();
            }
            _ => {}
        }
        out
    }

    // ----- the pointer -------------------------------------------------

    /// Pointer down at a view point. `extending` is the shift key: the hit
    /// shape joins or leaves the selection instead of replacing it, and
    /// nothing drags.
    pub fn pointer_down(&mut self, at: Point, extending: bool) {
        self.start = Some(at);
        self.selection_before = Some(self.selection.clone());
        let p = self.fit().user(at);
        let tol = self.user_tolerance();
        let near = |a: Point, b: Point| (a.0 - b.0).abs() <= tol && (a.1 - b.1).abs() <= tol;
        match self.tool {
            Tool::Select => {
                let lone = match self.selection.as_slice() {
                    [id] => Some(id.clone()),
                    _ => None,
                };
                let connector = lone
                    .as_deref()
                    .and_then(|id| self.drawing.connector(id).map(|c| (id.to_string(), c)));
                if let Some((id, c)) = &connector
                    && let Some(end) = [End::From, End::To]
                        .into_iter()
                        .find(|&e| near(c.end(e), p))
                {
                    self.drag = Some(Drag::Endpoint {
                        id: id.clone(),
                        end,
                        to: c.end(end),
                    });
                } else if let Some((id, c)) = &connector
                    && near(c.midpoint(), p)
                {
                    self.drag = Some(Drag::Bend {
                        id: id.clone(),
                        to: c.midpoint(),
                    });
                } else if let Some(id) = self.resizable().map(str::to_string)
                    && let Some(b) = self.drawing.bounds(&id)
                    && let Some(handle) = Handle::at(b, p.0, p.1, tol)
                {
                    self.drag = Some(Drag::Resize {
                        id,
                        handle,
                        start: b,
                        delta: (0.0, 0.0),
                    });
                } else if let Some(id) = self.outermost_at(p) {
                    if extending {
                        match self.selection.iter().position(|s| *s == id) {
                            Some(i) => {
                                self.selection.remove(i);
                            }
                            None => self.selection.push(id),
                        }
                    } else {
                        // A click on what is already selected drags the lot.
                        if !self.selection.contains(&id) {
                            self.selection = vec![id];
                        }
                        self.drag = Some(Drag::Move {
                            ids: self.selection.clone(),
                            delta: (0.0, 0.0),
                        });
                    }
                } else if !extending {
                    self.selection.clear();
                }
            }
            Tool::Hand => {
                self.drag = Some(Drag::Pan {
                    start: self.pan,
                    from: at,
                });
            }
            Tool::Eraser => {
                self.selection.clear();
                self.drag = Some(Drag::Erase { ids: Vec::new() });
                self.erase(p);
            }
            Tool::Text => {
                self.selection.clear();
                let fit = self.fit();
                let size = self.drawing.font_size(None) * fit.scale;
                self.text_edit = Some(TextEdit {
                    id: None,
                    anchor: p,
                    text: String::new(),
                    frame: (at.0, at.1 - size, size * 8.0, size * 1.3),
                    font_size: size,
                    font_family: self.drawing.font(None).map(|f| f.family),
                    wraps: false,
                });
                self.release();
            }
            Tool::Rect | Tool::Diamond | Tool::Ellipse | Tool::Arrow | Tool::Line | Tool::Note => {
                self.drag = Some(Drag::Create { from: p, to: p });
            }
            Tool::Draw => {
                self.selection.clear();
                self.drag = Some(Drag::Ink { points: vec![p] });
            }
        }
    }

    /// The outermost shape under a user point, within the tolerance.
    fn outermost_at(&self, p: Point) -> Option<String> {
        let hit = self.drawing.hit(p.0, p.1, self.user_tolerance())?;
        self.drawing.outermost(&hit.key).map(|s| s.key.clone())
    }

    /// The outermost shape under a view point, within the tolerance, or
    /// `None` over nothing: what a context menu asks before it offers one.
    pub fn shape_at(&self, at: Point) -> Option<String> {
        self.outermost_at(self.fit().user(at))
    }

    /// A one-shot tool's shape is made: back to select, unless locked.
    fn release(&mut self) {
        if !self.locked && self.tool.is_one_shot() {
            self.tool = Tool::Select;
        }
    }

    /// The eraser passing a user point: the outermost shape there joins
    /// what it will delete.
    fn erase(&mut self, p: Point) {
        let hit = self.outermost_at(p);
        if let Some(Drag::Erase { ids }) = &mut self.drag
            && let Some(id) = hit
            && !ids.contains(&id)
        {
            ids.push(id);
        }
    }

    /// Whether a pointer is down on the canvas.
    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// Pointer moved to a view point with the button down.
    pub fn pointer_moved(&mut self, at: Point) {
        let fit = self.fit();
        let p = fit.user(at);
        let origin = fit.user(self.start.unwrap_or(at));
        let delta = (p.0 - origin.0, p.1 - origin.1);
        match &mut self.drag {
            Some(Drag::Move { delta: d, .. }) | Some(Drag::Resize { delta: d, .. }) => *d = delta,
            Some(Drag::Endpoint { to, .. }) | Some(Drag::Bend { to, .. }) => *to = p,
            Some(Drag::Create { to, .. }) => *to = p,
            Some(Drag::Ink { points }) => {
                // Drawn over the picture until it lifts, rather than
                // previewed: a preview re-parses the whole file per sample.
                // A point the format could not tell from the last is noise.
                if points
                    .last()
                    .is_none_or(|l| (l.0 - p.0).abs() >= 0.5 || (l.1 - p.1).abs() >= 0.5)
                {
                    points.push(p);
                }
                return;
            }
            Some(Drag::Pan { start, from }) => {
                self.pan = (start.0 + at.0 - from.0, start.1 + at.1 - from.1);
                return;
            }
            Some(Drag::Erase { .. }) => {
                self.erase(p);
                return;
            }
            None => return,
        }
        self.preview();
    }

    /// Apply the drag in flight to the drawing, the previous application
    /// undone first, so the picture shows the gesture as it stands.
    fn preview(&mut self) {
        if self.previewed {
            let _ = self.drawing.undo();
            self.revision += 1;
        }
        self.previewed = self.apply();
    }

    /// Apply the drag in flight; `true` when it wrote something.
    fn apply(&mut self) -> bool {
        let tol = self.user_tolerance();
        let Some(drag) = self.drag.clone() else {
            return false;
        };
        let wrote = match drag {
            Drag::Move { ids, delta } if delta != (0.0, 0.0) => {
                let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
                self.drawing.move_all(&ids, delta.0, delta.1).is_ok()
            }
            Drag::Resize {
                id,
                handle,
                start,
                delta,
            } if delta != (0.0, 0.0) => self
                .drawing
                .resize(&id, handle.drag(start, delta.0, delta.1))
                .is_ok(),
            Drag::Endpoint { id, end, to } => {
                // Dropped on a shape, the end binds to it and follows it
                // from now on; dropped on nothing, it is unbound.
                self.drawing.drop_end(&id, end, to.0, to.1, tol).is_ok()
            }
            Drag::Bend { id, to } => {
                // Dragged back onto the chord, within the tolerance, it is
                // straight again — which, for a line not yet bent, writes
                // nothing, and `preview` must not undo the step before it.
                let Some(c) = self.drawing.connector(&id) else {
                    return false;
                };
                let straight = distance_to_segment(to, c.from, c.to) <= tol;
                self.drawing
                    .bend(&id, (!straight).then_some(to))
                    .unwrap_or(false)
            }
            Drag::Create { from, to } => {
                let b = between(from, to);
                if b.width <= 0.0 && b.height <= 0.0 {
                    return false;
                }
                let made = match self.tool {
                    Tool::Rect => self.drawing.add_rect(thorn_svg_core::Rect {
                        x: b.x,
                        y: b.y,
                        width: b.width,
                        height: b.height,
                    }),
                    Tool::Diamond => self.drawing.add_diamond(b),
                    Tool::Note => self.drawing.add_note(b, ""),
                    Tool::Ellipse => self.drawing.add_ellipse(b),
                    Tool::Line => self.drawing.add_line(from.0, from.1, to.0, to.1),
                    Tool::Arrow => match self.heads {
                        Some(h) => self.drawing.add_arrow(from.0, from.1, to.0, to.1, h),
                        None => self.drawing.add_line(from.0, from.1, to.0, to.1),
                    },
                    _ => return false,
                };
                self.created = made.ok();
                self.created.is_some()
            }
            Drag::Ink { points } if !points.is_empty() => {
                // One point is a dot of the nib.
                self.created = self
                    .drawing
                    .add_ink(&points, &[self.ink_width], thorn_svg_core::Nib::Monoline)
                    .ok();
                self.created.is_some()
            }
            _ => false,
        };
        if wrote {
            self.revision += 1;
        }
        wrote
    }

    /// Pointer up: the gesture in flight lands as one splice — the last
    /// preview, which is already in the drawing.
    pub fn pointer_up(&mut self) {
        let drag = self.drag.clone();
        match drag {
            Some(
                Drag::Move { .. } | Drag::Resize { .. } | Drag::Endpoint { .. } | Drag::Bend { .. },
            ) => {
                if !self.previewed {
                    self.previewed = self.apply();
                }
            }
            Some(Drag::Create { .. }) => {
                if !self.previewed {
                    self.previewed = self.apply();
                }
                self.selection = match (&self.created, self.previewed) {
                    (Some(id), true) => vec![id.clone()],
                    _ => Vec::new(),
                };
                let made = self.created.clone();
                self.release();
                // A new note opens its label for typing at once.
                if let Some(id) = made
                    && self.drawing.note(&id).is_some()
                {
                    self.edit_note(&id);
                }
            }
            Some(Drag::Ink { .. }) => {
                // Not selected: the pen is kept for the next stroke.
                if !self.previewed {
                    self.previewed = self.apply();
                }
                self.release();
            }
            Some(Drag::Erase { ids }) if !ids.is_empty() => {
                // One undo step for the whole sweep.
                let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
                if self.drawing.delete_all(&ids).is_ok() {
                    self.revision += 1;
                }
            }
            _ => {}
        }
        self.end_drag();
    }

    /// The pointer taken away mid-gesture: nothing lands, what was
    /// previewed is undone, and the selection is as it was before the
    /// pointer went down.
    pub fn cancel_pointer(&mut self) {
        if self.previewed {
            let _ = self.drawing.undo();
            self.revision += 1;
        }
        if let Some(before) = self.selection_before.take() {
            self.selection = before;
        }
        self.end_drag();
    }

    fn end_drag(&mut self) {
        self.drag = None;
        self.start = None;
        self.selection_before = None;
        self.previewed = false;
        self.created = None;
        self.keep_selection_honest();
    }

    fn keep_selection_honest(&mut self) {
        let drawing = &self.drawing;
        self.selection.retain(|id| drawing.shape(id).is_some());
    }

    // ----- text --------------------------------------------------------

    /// Whether `edit_selected_text` would open a field: one note or one
    /// label is selected.
    pub fn can_edit_text(&self) -> bool {
        match self.selection.as_slice() {
            [id] => {
                self.drawing.note(id).is_some()
                    || self
                        .drawing
                        .shape(id)
                        .is_some_and(|s| s.kind == ShapeKind::Text)
            }
            _ => false,
        }
    }

    /// Open the lone selected note's or label's field.
    pub fn edit_selected_text(&mut self) {
        if let [id] = self.selection.as_slice() {
            let id = id.clone();
            if self.drawing.note(&id).is_some() {
                self.edit_note(&id);
            } else {
                self.edit_text(&id);
            }
        }
    }

    /// A double-click at a view point: on a label or a note, opens it for
    /// editing.
    pub fn double_click(&mut self, at: Point) {
        if self.tool != Tool::Select {
            return;
        }
        let p = self.fit().user(at);
        let Some(hit) = self.drawing.hit(p.0, p.1, self.user_tolerance()) else {
            return;
        };
        let (key, kind) = (hit.key.clone(), hit.kind);
        let outer = self.drawing.outermost(&key).map(|s| s.key.clone());
        if let Some(outer) = outer.filter(|o| self.drawing.note(o).is_some()) {
            self.edit_note(&outer);
        } else if kind == ShapeKind::Text {
            self.edit_text(&key);
        }
    }

    /// Open a note's label for editing: the field fills the box inside its
    /// padding, and wraps at that width.
    pub fn edit_note(&mut self, id: &str) {
        let Some(note) = self.drawing.note(id) else {
            return;
        };
        let Some(b) = self.drawing.bounds(&note.frame) else {
            return;
        };
        let text = self
            .drawing
            .shape(&note.label)
            .and_then(|s| s.text.clone())
            .unwrap_or_default();
        self.selection = vec![id.to_string()];
        let fit = self.fit();
        let inner = Bounds {
            x: b.x + NOTE_PAD,
            y: b.y + NOTE_PAD,
            width: (b.width - 2.0 * NOTE_PAD).max(1.0),
            height: (b.height - 2.0 * NOTE_PAD).max(1.0),
        };
        let r = fit.view_rect(inner);
        self.text_edit = Some(TextEdit {
            id: Some(note.label.clone()),
            anchor: (inner.x, inner.y),
            text,
            frame: (r.x, r.y, r.width, r.height),
            font_size: self.drawing.font_size(Some(&note.label)) * fit.scale,
            font_family: self.drawing.font(Some(&note.label)).map(|f| f.family),
            wraps: true,
        });
    }

    /// Open a label for editing.
    pub fn edit_text(&mut self, id: &str) {
        let Some(shape) = self.drawing.shape(id).filter(|s| s.kind == ShapeKind::Text) else {
            return;
        };
        let text = shape.text.clone().unwrap_or_default();
        let width = shape.number("data-width");
        let Some(b) = self.drawing.bounds(id) else {
            return;
        };
        self.selection = vec![id.to_string()];
        let fit = self.fit();
        let size = self.drawing.font_size(Some(id)) * fit.scale;
        let r = fit.view_rect(b);
        let (w, h) = match width {
            Some(w) => (w * fit.scale, r.height + size * 1.3),
            None => ((r.width + size).max(size * 4.0), r.height),
        };
        self.text_edit = Some(TextEdit {
            id: Some(id.to_string()),
            anchor: (b.x, b.y),
            text,
            frame: (r.x, r.y, w, h),
            font_size: size,
            font_family: self.drawing.font(Some(id)).map(|f| f.family),
            wraps: width.is_some(),
        });
    }

    /// What was typed into the open field: a new label at its anchor, or
    /// the label's characters replaced; a newline is a line break of the
    /// label's own. Empty text adds nothing, and deletes a label being
    /// re-worded — a note, box and all. One undo step either way. `None`
    /// closes the field and leaves the label as it was.
    pub fn commit_text_edit(&mut self, text: Option<&str>) {
        let Some(edit) = self.text_edit.take() else {
            return;
        };
        let Some(text) = text else {
            return;
        };
        let trimmed = text.trim();
        let landed = match &edit.id {
            Some(id) if trimmed.is_empty() => {
                let outer = self.drawing.outermost(id).map(|s| s.key.clone());
                let victim = outer
                    .filter(|o| self.drawing.note(o).is_some())
                    .unwrap_or_else(|| id.clone());
                self.drawing.delete(&victim).is_ok()
            }
            Some(id) if trimmed != edit.text => self.drawing.set_text(id, trimmed).is_ok(),
            Some(_) => false,
            None if !trimmed.is_empty() => {
                match self.drawing.add_text(edit.anchor.0, edit.anchor.1, trimmed) {
                    Ok(id) => {
                        self.selection = vec![id];
                        true
                    }
                    Err(_) => false,
                }
            }
            None => false,
        };
        if landed {
            self.revision += 1;
        }
        self.keep_selection_honest();
    }

    // ----- keys --------------------------------------------------------

    /// A key with no modifier but shift, as the canvas sees it: a tool's
    /// key picks the tool, `q` toggles the lock, Escape is select with
    /// nothing selected. `true` when the key meant something.
    pub fn key(&mut self, key: char) -> bool {
        if key == '\u{1b}' {
            self.tool = Tool::Select;
            self.selection.clear();
        } else if key.eq_ignore_ascii_case(&Tool::LOCK_KEY) {
            self.locked = !self.locked;
        } else if let Some(tool) = Tool::for_key(key) {
            self.tool = tool;
        } else {
            return false;
        }
        true
    }

    // ----- commands ----------------------------------------------------

    /// Run `edit` on the drawing, counting it as a revision when it wrote.
    fn edited(&mut self, edit: impl FnOnce(&mut Drawing) -> bool) {
        if edit(&mut self.drawing) {
            self.revision += 1;
        }
        self.keep_selection_honest();
    }

    fn ids(&self, wanted: impl Fn(&Drawing, &str) -> bool) -> Vec<String> {
        self.selection
            .iter()
            .filter(|id| wanted(&self.drawing, id))
            .cloned()
            .collect()
    }

    /// Delete the selection, as one undo step.
    pub fn delete_selection(&mut self) {
        let ids = self.selection.clone();
        if ids.is_empty() {
            return;
        }
        self.edited(|d| d.delete_all(&refs(&ids)).is_ok());
    }

    /// Reorder a lone selected shape.
    pub fn reorder_selection(&mut self, order: Order) {
        if let [id] = self.selection.as_slice() {
            let id = id.clone();
            self.edited(|d| d.reorder(&id, order).unwrap_or(false));
        }
    }

    /// Whether `group_selection` would do something: two or more shapes
    /// with one parent.
    pub fn can_group(&self) -> bool {
        let shapes: Vec<_> = self
            .selection
            .iter()
            .filter_map(|id| self.drawing.shape(id))
            .collect();
        shapes.len() >= 2
            && shapes.len() == self.selection.len()
            && shapes.windows(2).all(|w| w[0].group == w[1].group)
    }

    /// Wrap the selection in a group, which becomes the selection.
    pub fn group_selection(&mut self) {
        if !self.can_group() {
            return;
        }
        let ids = self.selection.clone();
        if let Ok(id) = self.drawing.group(&refs(&ids)) {
            self.revision += 1;
            self.selection = vec![id];
        }
    }

    /// Whether `ungroup_selection` would do something.
    pub fn can_ungroup(&self) -> bool {
        matches!(self.selection.as_slice(), [id] if self.drawing.shape(id).is_some_and(|s| s.kind == ShapeKind::Group))
    }

    /// Replace a lone selected group with its members, which become the
    /// selection.
    pub fn ungroup_selection(&mut self) {
        if !self.can_ungroup() {
            return;
        }
        let id = self.selection[0].clone();
        if let Ok(members) = self.drawing.ungroup(&id) {
            self.revision += 1;
            self.selection = members;
        }
    }

    pub fn can_undo(&self) -> bool {
        true
    }

    pub fn undo(&mut self) {
        self.edited(|d| d.undo().unwrap_or(false));
    }

    pub fn redo(&mut self) {
        self.edited(|d| d.redo().unwrap_or(false));
    }

    /// Select shapes by id (or nothing).
    pub fn select(&mut self, ids: Vec<String>) {
        self.selection = ids;
        self.keep_selection_honest();
    }

    /// Every outermost shape.
    pub fn select_all(&mut self) {
        let ids = self
            .drawing
            .shapes()
            .iter()
            .filter(|s| s.group.is_none())
            .map(|s| s.key.clone())
            .collect();
        self.select(ids);
    }

    // ----- options -----------------------------------------------------

    /// The connectors among the selection: what an arrowhead lands on.
    pub fn selected_connectors(&self) -> Vec<String> {
        self.ids(|d, id| d.connector(id).is_some())
    }

    /// The shapes among the selection a dash or a weight lands on.
    pub fn selected_stroked(&self) -> Vec<String> {
        self.ids(|d, id| d.takes_dash(id))
    }

    /// The shapes among the selection a colour lands on.
    pub fn selected_colorable(&self) -> Vec<String> {
        self.ids(|d, id| d.takes_color(id))
    }

    /// The shapes among the selection a background lands on.
    pub fn selected_closed(&self) -> Vec<String> {
        self.ids(|d, id| d.takes_fill(id))
    }

    /// The boxes among the selection a corner lands on.
    pub fn selected_boxes(&self) -> Vec<String> {
        self.ids(|d, id| d.takes_corner(id))
    }

    /// Which choices the options strip shows: for the selection when there
    /// is one, else for the tool.
    pub fn offers(&self) -> Offers {
        if self.selection.is_empty() {
            let t = self.tool;
            Offers {
                color: t.makes_shape(),
                fill: t.makes_closed_shape(),
                stroke: t.makes_stroke(),
                corner: t.makes_box(),
                heads: t == Tool::Arrow,
            }
        } else {
            Offers {
                color: !self.selected_colorable().is_empty(),
                fill: !self.selected_closed().is_empty(),
                stroke: !self.selected_stroked().is_empty(),
                corner: !self.selected_boxes().is_empty(),
                heads: !self.selected_connectors().is_empty(),
            }
        }
    }

    /// The hue the next shape is drawn in, or the selected shapes agree on
    /// (`Some(None)` for the drawing's ink); `None` when they differ.
    pub fn color(&self) -> Option<Option<Hue>> {
        if self.selection.is_empty() {
            return Some(self.drawing.pen().hue);
        }
        agreed(
            self.selected_colorable()
                .iter()
                .map(|id| self.drawing.color(id)),
        )
    }

    pub fn fill(&self) -> Option<Option<Hue>> {
        if self.selection.is_empty() {
            return Some(self.drawing.pen().fill);
        }
        agreed(
            self.selected_closed()
                .iter()
                .map(|id| self.drawing.fill(id)),
        )
    }

    pub fn dash(&self) -> Option<Option<Dash>> {
        if self.selection.is_empty() {
            return Some(self.drawing.pen().dash);
        }
        agreed(
            self.selected_stroked()
                .iter()
                .map(|id| self.drawing.dash(id)),
        )
    }

    pub fn weight(&self) -> Option<Option<Weight>> {
        if self.selection.is_empty() {
            return Some(self.drawing.pen().weight);
        }
        agreed(
            self.selected_stroked()
                .iter()
                .map(|id| self.drawing.weight(id)),
        )
    }

    pub fn corner(&self) -> Option<Option<f64>> {
        if self.selection.is_empty() {
            return Some(self.drawing.pen().corner);
        }
        agreed(
            self.selected_boxes()
                .iter()
                .map(|id| self.drawing.corner(id)),
        )
    }

    pub fn arrowheads(&self) -> Option<Option<Heads>> {
        if self.selection.is_empty() {
            return Some(self.heads);
        }
        agreed(
            self.selected_connectors()
                .iter()
                .map(|id| self.drawing.shape(id).and_then(|s| s.heads())),
        )
    }

    /// Colour: the selected shapes, as one undo step, when any takes one;
    /// otherwise the next drawn.
    pub fn set_color(&mut self, hue: Option<Hue>) {
        let ids = self.selected_colorable();
        if ids.is_empty() {
            let mut pen = self.drawing.pen();
            pen.hue = hue;
            self.drawing.set_pen(pen);
        } else {
            self.edited(|d| d.set_color_all(&refs(&ids), hue).is_ok());
        }
    }

    pub fn set_fill(&mut self, hue: Option<Hue>) {
        let ids = self.selected_closed();
        if ids.is_empty() {
            let mut pen = self.drawing.pen();
            pen.fill = hue;
            self.drawing.set_pen(pen);
        } else {
            self.edited(|d| d.set_fill_all(&refs(&ids), hue).is_ok());
        }
    }

    pub fn set_dash(&mut self, dash: Option<Dash>) {
        let ids = self.selected_stroked();
        if ids.is_empty() {
            let mut pen = self.drawing.pen();
            pen.dash = dash;
            self.drawing.set_pen(pen);
        } else {
            self.edited(|d| d.set_dash_all(&refs(&ids), dash).is_ok());
        }
    }

    pub fn set_weight(&mut self, weight: Option<Weight>) {
        let ids = self.selected_stroked();
        if ids.is_empty() {
            let mut pen = self.drawing.pen();
            pen.weight = weight;
            self.drawing.set_pen(pen);
        } else {
            self.edited(|d| d.set_weight_all(&refs(&ids), weight).is_ok());
        }
    }

    pub fn set_corner(&mut self, radius: Option<f64>) {
        let ids = self.selected_boxes();
        if ids.is_empty() {
            let mut pen = self.drawing.pen();
            pen.corner = radius;
            self.drawing.set_pen(pen);
        } else {
            self.edited(|d| d.set_corner_all(&refs(&ids), radius).is_ok());
        }
    }

    pub fn set_heads(&mut self, heads: Option<Heads>) {
        let ids = self.selected_connectors();
        if ids.is_empty() {
            self.heads = heads;
        } else {
            self.edited(|d| d.set_heads_all(&refs(&ids), heads).is_ok());
        }
    }
}

/// Which choices the options strip has something to say about.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Offers {
    pub color: bool,
    pub fill: bool,
    pub stroke: bool,
    pub corner: bool,
    pub heads: bool,
}

/// The box `zoom_to_fit` fits to the view: the page, or, for a drawing with
/// no `viewBox`, the box round its shapes.
fn sheet(drawing: &Drawing) -> Bounds {
    drawing
        .page()
        .or_else(|| {
            drawing
                .extent()
                .map(|e| e.expanded(thorn_svg_core::PAGE_MARGIN))
        })
        .unwrap_or(Bounds {
            x: 0.0,
            y: 0.0,
            width: 640.0,
            height: 400.0,
        })
}

fn between(a: Point, b: Point) -> Bounds {
    Bounds {
        x: a.0.min(b.0),
        y: a.1.min(b.1),
        width: (b.0 - a.0).abs(),
        height: (b.1 - a.1).abs(),
    }
}

fn distance_to_segment(p: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let length = dx * dx + dy * dy;
    let t = if length == 0.0 {
        0.0
    } else {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / length).clamp(0.0, 1.0)
    };
    ((p.0 - (a.0 + t * dx)).powi(2) + (p.1 - (a.1 + t * dy)).powi(2)).sqrt()
}

fn agreed<T: PartialEq>(mut values: impl Iterator<Item = T>) -> Option<T> {
    let first = values.next()?;
    values.all(|v| v == first).then_some(first)
}

fn refs(ids: &[String]) -> Vec<&str> {
    ids.iter().map(String::as_str).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh drawing in a 400 × 300 view.
    fn canvas() -> Canvas {
        let mut c = Canvas::new(Drawing::fresh());
        c.set_size(400.0, 300.0);
        c
    }

    fn drag(c: &mut Canvas, from: Point, to: Point) {
        c.pointer_down(from, false);
        let steps = 4;
        for i in 1..=steps {
            let t = i as f64 / steps as f64;
            c.pointer_moved((from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t));
        }
        c.pointer_up();
    }

    #[test]
    fn a_rectangle_dragged_out_is_one_undo_step_and_falls_back_to_select() {
        let mut c = canvas();
        c.tool = Tool::Rect;
        drag(&mut c, (100.0, 100.0), (200.0, 160.0));
        assert_eq!(c.drawing().shapes().len(), 1);
        assert_eq!(c.selection().len(), 1);
        assert_eq!(c.tool, Tool::Select);
        c.undo();
        assert!(c.drawing().shapes().is_empty());
        assert!(c.selection().is_empty());
    }

    #[test]
    fn the_lock_keeps_the_tool_and_the_pen_keeps_itself() {
        let mut c = canvas();
        assert!(c.key('q'));
        assert!(c.key('r'));
        drag(&mut c, (10.0, 10.0), (60.0, 60.0));
        assert_eq!(c.tool, Tool::Rect);
        c.locked = false;
        c.key('p');
        drag(&mut c, (10.0, 100.0), (90.0, 120.0));
        assert_eq!(c.tool, Tool::Draw);
        assert!(c.selection().is_empty(), "a stroke is not selected");
        assert_eq!(c.drawing().shapes().len(), 2);
    }

    #[test]
    fn a_drag_moves_what_it_lands_on_once() {
        let mut c = canvas();
        c.tool = Tool::Rect;
        drag(&mut c, (100.0, 100.0), (200.0, 160.0));
        let id = c.selection()[0].clone();
        let before = c.drawing().bounds(&id).unwrap();
        // On its top edge, between the handles: an unfilled box is hit by its
        // stroke.
        drag(&mut c, (125.0, 100.0), (155.0, 120.0));
        let after = c.drawing().bounds(&id).unwrap();
        let s = c.fit().scale;
        assert!(
            (after.x - before.x - 30.0 / s).abs() < 0.1,
            "{before:?} → {after:?}"
        );
        // The move is one step: one undo puts it back.
        c.undo();
        assert_eq!(c.drawing().bounds(&id).unwrap(), before);
    }

    #[test]
    fn the_picture_reaches_the_page_with_no_stylesheet_in_it() {
        let mut c = canvas();
        c.tool = Tool::Rect;
        drag(&mut c, (100.0, 100.0), (200.0, 160.0));
        let picture = c.picture().expect("the template lays out");
        assert!(!picture.contains("<style"), "{picture}");
        assert!(
            picture.contains("stroke=\"#222222\""),
            "the template's ink, applied: {picture}"
        );
        c.set_appearance(Appearance::Dark);
        let dark = c.picture().unwrap();
        assert!(
            dark.contains("stroke=\"#e6e6e6\""),
            "the file's dark rules, applied: {dark}"
        );
    }

    #[test]
    fn zoom_keeps_the_point_under_the_pointer() {
        let mut c = canvas();
        let at = (123.0, 77.0);
        let user = c.fit().user(at);
        c.zoom_by(2.0, at);
        let back = c.fit().view(user);
        assert!((back.0 - at.0).abs() < 1e-9 && (back.1 - at.1).abs() < 1e-9);
        c.zoom_by(1000.0, at);
        assert_eq!(c.zoom(), ZOOM_RANGE.1);
        c.zoom_to_fit();
        assert_eq!(c.zoom(), 1.0);
    }

    #[test]
    fn a_label_is_typed_into_a_field_and_lands_once() {
        let mut c = canvas();
        c.tool = Tool::Text;
        c.pointer_down((50.0, 50.0), false);
        c.pointer_up();
        let edit = c.text_edit().cloned().expect("a field opens");
        assert_eq!(edit.id, None);
        c.commit_text_edit(Some("  hello  "));
        assert!(c.text_edit().is_none());
        let id = c.selection()[0].clone();
        assert_eq!(
            c.drawing().shape(&id).unwrap().text.as_deref(),
            Some("hello")
        );
        // Emptied, it goes.
        c.edit_selected_text();
        c.commit_text_edit(Some(""));
        assert!(c.drawing().shapes().is_empty());
    }

    #[test]
    fn a_new_note_opens_its_label_and_an_empty_one_goes_box_and_all() {
        let mut c = canvas();
        c.tool = Tool::Note;
        drag(&mut c, (50.0, 50.0), (200.0, 150.0));
        let edit = c.text_edit().cloned().expect("the note's label opens");
        assert!(edit.wraps);
        c.commit_text_edit(Some(""));
        assert!(c.drawing().shapes().is_empty());
    }

    #[test]
    fn the_eraser_deletes_its_sweep_as_one_step() {
        let mut c = canvas();
        c.locked = true;
        c.tool = Tool::Rect;
        drag(&mut c, (20.0, 20.0), (60.0, 60.0));
        drag(&mut c, (100.0, 20.0), (140.0, 60.0));
        c.tool = Tool::Eraser;
        let fit = c.fit();
        let (a, b) = (
            c.drawing().shapes()[0].key.clone(),
            c.drawing().shapes()[1].key.clone(),
        );
        let pa = fit.view(c.drawing().connector(&a).map_or_else(
            || {
                let r = c.drawing().bounds(&a).unwrap();
                (r.x, r.y + r.height / 2.0)
            },
            |c| c.from,
        ));
        let r = c.drawing().bounds(&b).unwrap();
        let pb = fit.view((r.x, r.y + r.height / 2.0));
        c.pointer_down(pa, false);
        c.pointer_moved(pb);
        assert_eq!(c.overlay().erasing.len(), 2);
        c.pointer_up();
        assert!(c.drawing().shapes().is_empty());
        c.undo();
        assert_eq!(c.drawing().shapes().len(), 2);
    }

    #[test]
    fn options_are_the_tools_with_nothing_selected_and_the_selections_after() {
        let mut c = canvas();
        c.tool = Tool::Arrow;
        assert!(c.offers().heads && !c.offers().fill);
        c.set_color(Some(Hue::Red));
        assert_eq!(c.color(), Some(Some(Hue::Red)));
        drag(&mut c, (20.0, 20.0), (120.0, 60.0));
        assert_eq!(c.selection().len(), 1);
        assert_eq!(c.color(), Some(Some(Hue::Red)), "born in the pen's hue");
        assert_eq!(c.arrowheads(), Some(Some(Heads::End)));
        c.set_heads(Some(Heads::Both));
        assert_eq!(c.arrowheads(), Some(Some(Heads::Both)));
        assert!(!c.offers().fill);
    }
}
