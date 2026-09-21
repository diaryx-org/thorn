// The Swift-shaped layer over the binding: a drawing as an object an AppKit
// or UIKit document can own, with the core's gestures as methods, Foundation
// types at the edges, and a parsed picture kept current for the canvas.

import CoreGraphics
import Foundation
import ResvgCoreGraphics
import ThornFFI

/// A drawing being edited. Wraps the Rust `Drawing`; every mutating method
/// is one twig splice and one undo step on the Rust side, after which
/// `picture` is re-parsed and `onChange` is called.
public final class DrawingDocument {
    private let inner: Drawing

    /// The drawing as resvg draws it, re-parsed after every gesture. `nil`
    /// only when usvg refuses what twig accepted — an `<svg>` with no size,
    /// say — in which case the canvas shows nothing but the selection.
    public private(set) var picture: SVGPicture?

    /// Called after every gesture; a view redraws here.
    public var onChange: (() -> Void)?

    /// Open an SVG's text. Throws `DrawingError` for what is not XML or whose
    /// document element is not `<svg>`; a file that breaks the profile still
    /// opens, and `check()` says how.
    public init(source: String) throws {
        inner = try Drawing.open(source: source)
        picture = try? SVGPicture(data: Data(source.utf8))
    }

    /// Open an SVG file.
    public convenience init(contentsOf url: URL) throws {
        try self.init(source: String(contentsOf: url, encoding: .utf8))
    }

    /// A new, empty drawing: the profile's template opened.
    public static func fresh() -> DrawingDocument {
        // The template is a drawing by the core's own test; `try!` would say
        // the same thing less politely.
        try! DrawingDocument(source: template)
    }

    /// The bytes a new drawing is created with — for a host that writes the
    /// file before it opens it.
    public static var template: String { ThornFFI.template() }

    /// Whether `source` is a drawing of the profile — an `<svg>` whose root
    /// carries `data-diaryx-drawing` — as opposed to any other SVG, which a
    /// viewer shows and this editor would only find fault with.
    public static func isDrawing(_ source: String) -> Bool { ThornFFI.isDrawing(source: source) }

    /// The current bytes — what saving writes.
    public var source: String { inner.source() }

    /// The shapes, in paint order.
    public var shapes: [Shape] { inner.shapes() }

    /// The shape with this `data-id`.
    public func shape(id: String) -> Shape? { shapes.first { $0.id == id } }

    /// Hold the drawing to the profile. Empty means it conforms.
    public func check() -> [Finding] { inner.check() }

    /// A shape's extent in user units, through its transform chain; a
    /// `<g>`'s is its members'; a `<text>`'s is measured by the layout the
    /// picture is drawn with. `nil` for an empty group.
    public func bounds(id: String) -> CGRect? {
        inner.bounds(id: id).map(CGRect.init)
    }

    /// The topmost shape within `tolerance` user units of a point. A member
    /// of a group is returned itself; `outermost(id:)` is what to select.
    public func hit(_ point: CGPoint, tolerance: CGFloat) -> Shape? {
        inner.hit(x: Double(point.x), y: Double(point.y), tolerance: Double(tolerance))
    }

    /// The outermost group a shape is in, or the shape itself.
    public func outermost(id: String) -> Shape? { inner.outermost(id: id) }

    /// The shapes directly inside a `<g>`, in paint order.
    public func members(id: String) -> [Shape] { inner.members(id: id) }

    /// Where an end of a `<line>` is, in user units; `nil` for any other
    /// kind.
    public func endPoint(id: String, _ end: End) -> CGPoint? {
        inner.endPoint(id: id, end: end).map { CGPoint(x: $0.x, y: $0.y) }
    }

    /// What an end of a `<line>` is bound to — the `data-id` in its
    /// `data-from` or `data-to` — if anything.
    public func binding(id: String, _ end: End) -> String? {
        let name = end == .from ? "data-from" : "data-to"
        return shape(id: id)?.attrs.first { $0.name == name }?.value
    }

    // MARK: Gestures

    /// Add a rectangle as the topmost shape; returns its `data-id`.
    @discardableResult
    public func addRect(_ rect: CGRect) throws -> String {
        try changed { try inner.addRect(rect: Rect(
            x: Double(rect.origin.x), y: Double(rect.origin.y),
            width: Double(rect.size.width), height: Double(rect.size.height))) }
    }

    /// Add an ellipse filling a box; returns its `data-id`.
    @discardableResult
    public func addEllipse(in rect: CGRect) throws -> String {
        try changed { try inner.addEllipse(bounds: Bounds(rect)) }
    }

    /// Add a diamond filling a box — a polygon through the midpoints of
    /// its sides; returns its `data-id`.
    @discardableResult
    public func addDiamond(in rect: CGRect) throws -> String {
        try changed { try inner.addDiamond(bounds: Bounds(rect)) }
    }

    /// Add a note filling a box — a group of a box and a label wrapped to
    /// it; returns the group's `data-id`. `note(id:)` names its members.
    @discardableResult
    public func addNote(in rect: CGRect, text: String = "") throws -> String {
        try changed { try inner.addNote(bounds: Bounds(rect), text: text) }
    }

    /// A note's box and label, when the shape is a note.
    public func note(id: String) -> Note? { inner.note(id: id) }

    /// The space between a note's box and its label, in user units.
    public static let notePadding: CGFloat = 8

    /// Add a line; returns its `data-id`.
    @discardableResult
    public func addLine(from a: CGPoint, to b: CGPoint) throws -> String {
        try changed { try inner.addLine(x1: Double(a.x), y1: Double(a.y), x2: Double(b.x), y2: Double(b.y)) }
    }

    /// Add an arrow: a line with a head at `b` (or at both ends), which
    /// the drawing's `<style>` draws from `data-arrow`.
    @discardableResult
    public func addArrow(from a: CGPoint, to b: CGPoint, heads: Heads = .end) throws -> String {
        try changed { try inner.addArrow(x1: Double(a.x), y1: Double(a.y), x2: Double(b.x), y2: Double(b.y), heads: heads) }
    }

    /// Which ends of a shape have a head; `nil` for one that is not an arrow.
    public func heads(id: String) -> Heads? { inner.heads(id: id) }

    /// Add a freehand stroke along `points`, `width` wide, as a monoline:
    /// the outline is the file's, the centreline and width beside it.
    @discardableResult
    public func addInk(_ points: [CGPoint], width: CGFloat) throws -> String {
        try changed { try inner.addInk(points: points.map { Point(x: Double($0.x), y: Double($0.y)) }, widths: [Double(width)], nib: .monoline) }
    }

    /// Add a label anchored at a point; returns its `data-id`.
    @discardableResult
    public func addText(_ text: String, at point: CGPoint) throws -> String {
        try changed { try inner.addText(x: Double(point.x), y: Double(point.y), text: text) }
    }

    /// Replace a label's characters; plain text, written escaped. One undo
    /// step.
    public func setText(id: String, _ text: String) throws {
        try changed { try inner.setText(id: id, text: text) }
    }

    /// Wrap a label to a width in user units, its words flowed into lines;
    /// or, with `nil`, put them back on one line. One undo step. A resize
    /// by the handles does this too.
    public func setWidth(id: String, _ width: CGFloat?) throws {
        try changed { try inner.setWidth(id: id, width: width.map(Double.init)) }
    }

    /// The size a label lays out at, in user units — its `font-size`, or
    /// what the stylesheet gave it; with `nil`, a new label's.
    public func fontSize(id: String?) -> CGFloat { CGFloat(inner.fontSize(id: id)) }

    /// The face a label lays out in, as resvg resolved it from the
    /// stylesheet — the PostScript name a platform font API instantiates
    /// exactly — and its size; with `nil`, a new label's.
    public func font(id: String?) -> Font? { inner.font(id: id) }

    /// The width a label wraps to, if it does.
    public func width(id: String) -> CGFloat? {
        shape(id: id)?.attrs.first { $0.name == "data-width" }?.value.flatMap { Double($0) }.map { CGFloat($0) }
    }

    /// Delete the shape with this `data-id`.
    public func delete(id: String) throws {
        try changed { try inner.delete(id: id) }
    }

    /// Delete several shapes as one undo step.
    public func delete(ids: [String]) throws {
        try changed { try inner.deleteAll(ids: ids) }
    }

    /// Move a shape by a vector.
    public func move(id: String, by delta: CGVector) throws {
        try changed { try inner.moveBy(id: id, dx: Double(delta.dx), dy: Double(delta.dy)) }
    }

    /// Move several shapes by a vector as one undo step.
    public func move(ids: [String], by delta: CGVector) throws {
        try changed { try inner.moveAll(ids: ids, dx: Double(delta.dx), dy: Double(delta.dy)) }
    }

    /// Wrap sibling shapes in a new `<g>`; returns its `data-id`. One undo
    /// step. Throws `DrawingError.NotSiblings` for shapes in different groups.
    @discardableResult
    public func group(ids: [String]) throws -> String {
        try changed { try inner.group(ids: ids) }
    }

    /// Replace a `<g>` with its members, nothing moving; returns their ids.
    /// One undo step.
    @discardableResult
    public func ungroup(id: String) throws -> [String] {
        try changed { try inner.ungroup(id: id) }
    }

    /// Bind an end of a `<line>` to a shape, the end put on its edge; or,
    /// with `nil`, unbind it. One undo step.
    public func bind(id: String, _ end: End, to target: String?) throws {
        try changed { try inner.bind(id: id, end: end, target: target) }
    }

    /// Drop an end of a `<line>` at a point: it goes there, bound to the
    /// topmost shape within `tolerance` — any but the arrow — or unbound.
    /// Returns what it was bound to. One undo step.
    @discardableResult
    public func dropEnd(id: String, _ end: End, at point: CGPoint, tolerance: CGFloat) throws -> String? {
        try changed { try inner.dropEnd(id: id, end: end, x: Double(point.x), y: Double(point.y), tolerance: Double(tolerance)) }
    }

    /// Fit a shape to a box.
    public func resize(id: String, to rect: CGRect) throws {
        try changed { try inner.resize(id: id, to: Bounds(rect)) }
    }

    /// Change a shape's place in paint order; `false` when it was already
    /// there (and nothing changed).
    @discardableResult
    public func reorder(id: String, _ order: Order) throws -> Bool {
        try changed { try inner.reorder(id: id, order: order) }
    }

    /// Undo the last gesture; `false` when there was nothing to undo.
    @discardableResult
    public func undo() throws -> Bool {
        try changed { try inner.undo() }
    }

    /// Redo the last undone gesture; `false` when there was nothing to redo.
    @discardableResult
    public func redo() throws -> Bool {
        try changed { try inner.redo() }
    }

    private func changed<T>(_ gesture: () throws -> T) rethrows -> T {
        let result = try gesture()
        picture = try? SVGPicture(data: Data(source.utf8))
        onChange?()
        return result
    }
}

extension CGRect {
    init(_ b: Bounds) { self.init(x: b.x, y: b.y, width: b.width, height: b.height) }
}

extension Bounds {
    init(_ r: CGRect) {
        self.init(x: Double(r.origin.x), y: Double(r.origin.y), width: Double(r.size.width), height: Double(r.size.height))
    }
}

extension Handle {
    /// Where this handle sits on a box.
    public func position(on rect: CGRect) -> CGPoint {
        let p = handlePosition(handle: self, bounds: Bounds(rect))
        return CGPoint(x: p.x, y: p.y)
    }

    /// The handle of a box within `tolerance` of a point, if any.
    public static func at(_ point: CGPoint, on rect: CGRect, tolerance: CGFloat) -> Handle? {
        handleAt(bounds: Bounds(rect), x: Double(point.x), y: Double(point.y), tolerance: Double(tolerance))
    }

    /// The box after this handle is dragged by a vector.
    public func drag(_ rect: CGRect, by delta: CGVector) -> CGRect {
        CGRect(handleDrag(handle: self, bounds: Bounds(rect), dx: Double(delta.dx), dy: Double(delta.dy)))
    }

    /// Every handle, clockwise from the top-left.
    public static let all: [Handle] = [.topLeft, .top, .topRight, .right, .bottomRight, .bottom, .bottomLeft, .left]
}
