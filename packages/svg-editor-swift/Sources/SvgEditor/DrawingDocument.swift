// The Swift-shaped layer over the binding: a drawing as a value an AppKit or
// UIKit document can own, with the core's gestures as methods and Foundation
// types at the edges. The canvas view that draws it and turns pointer events
// into these gestures is docs/tasks/swift-canvas.md.

import CoreGraphics
import Foundation
import SvgEditorFFI

/// A drawing being edited. Wraps the Rust `Drawing`; every method is one
/// twig splice and one undo step on the Rust side.
public final class DrawingDocument {
    private let inner: Drawing

    /// Open an SVG's text. Throws `DrawingError` for what is not XML or whose
    /// document element is not `<svg>`; a file that breaks the profile still
    /// opens, and `check()` says how.
    public init(source: String) throws {
        inner = try Drawing.open(source: source)
    }

    /// Open an SVG file.
    public convenience init(contentsOf url: URL) throws {
        try self.init(source: String(contentsOf: url, encoding: .utf8))
    }

    /// The current bytes — what saving writes.
    public var source: String { inner.source() }

    /// The shapes, in paint order.
    public var shapes: [Shape] { inner.shapes() }

    /// Hold the drawing to the profile. Empty means it conforms.
    public func check() -> [Finding] { inner.check() }

    /// Add a rectangle as the topmost shape; returns its `data-id`.
    @discardableResult
    public func addRect(_ rect: CGRect) throws -> String {
        try inner.addRect(rect: Rect(
            x: Double(rect.origin.x), y: Double(rect.origin.y),
            width: Double(rect.size.width), height: Double(rect.size.height)))
    }

    /// Delete the shape with this `data-id`.
    public func delete(id: String) throws { try inner.delete(id: id) }

    /// The bounds a shape's attributes state; `nil` for a `<path>` or `<g>`.
    public func bounds(id: String) -> CGRect? {
        inner.bounds(id: id).map { CGRect(x: $0.x, y: $0.y, width: $0.width, height: $0.height) }
    }

    /// Move a shape by a vector.
    public func move(id: String, by delta: CGVector) throws {
        try inner.moveBy(id: id, dx: Double(delta.dx), dy: Double(delta.dy))
    }

    /// Fit a shape to a box.
    public func resize(id: String, to rect: CGRect) throws {
        try inner.resize(id: id, to: Bounds(
            x: Double(rect.origin.x), y: Double(rect.origin.y),
            width: Double(rect.size.width), height: Double(rect.size.height)))
    }

    /// Change a shape's place in paint order; `false` when it was already
    /// there.
    @discardableResult
    public func reorder(id: String, _ order: Order) throws -> Bool {
        try inner.reorder(id: id, order: order)
    }

    /// Undo the last gesture; `false` when there was nothing to undo.
    @discardableResult
    public func undo() throws -> Bool { try inner.undo() }

    /// Redo the last undone gesture; `false` when there was nothing to redo.
    @discardableResult
    public func redo() throws -> Bool { try inner.redo() }
}
