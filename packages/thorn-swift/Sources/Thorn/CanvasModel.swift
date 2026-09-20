// What a canvas does, with no view in it: the tool, the selection, the drag
// in flight, the mapping between view points and user units, and the
// drawing of picture + selection + handles into any CGContext. The AppKit
// and UIKit views are thin over this, and it is testable with a bitmap.
//
// During a drag the model keeps the in-flight geometry and draws it as an
// outline over the picture; the gesture lands as one splice on pointer-up.
// Nothing is written to the document while the pointer is down.

import CoreGraphics
import Foundation
import ThornFFI

/// What the next pointer-down does.
public enum Tool: Equatable {
    /// Select, move by dragging, resize by a handle.
    case select
    case rect
    case ellipse
    case line
    /// Place a label with this text.
    case text(String)
}

/// The canvas as a value: everything a view needs to draw and to answer a
/// pointer, and nothing about the platform.
public final class CanvasModel {
    public let document: DrawingDocument
    public var tool: Tool = .select {
        didSet { if tool != oldValue { onToolChange?(tool) } }
    }
    /// Called when the tool changes — a create tool is one-shot and falls
    /// back to select on its own, so a toolbar has to be told.
    public var onToolChange: ((Tool) -> Void)?

    /// The selected shape's `data-id`, if any.
    public private(set) var selection: String? {
        didSet { onSelectionChange?(selection) }
    }
    /// Called when the selection changes.
    public var onSelectionChange: ((String?) -> Void)?
    /// Called when the picture or overlay needs redrawing.
    public var needsDisplay: (() -> Void)?

    /// How far, in view points, a click may miss a stroke or a handle.
    public var tolerance: CGFloat = 4

    /// The user-unit box the picture is drawn into, set by the view from its
    /// bounds on every draw; the same fit `SVGPicture.draw` uses.
    private var fit: (scale: CGFloat, origin: CGPoint) = (1, .zero)

    private enum Drag {
        case move(id: String, start: CGRect, delta: CGVector)
        case resize(id: String, handle: Handle, start: CGRect, delta: CGVector)
        case create(from: CGPoint, to: CGPoint)
    }
    private var drag: Drag?

    public init(document: DrawingDocument) {
        self.document = document
        document.onChange = { [weak self] in
            guard let self else { return }
            if let id = selection, document.shape(id: id) == nil { selection = nil }
            needsDisplay?()
        }
    }

    // MARK: Coordinates

    /// User units → view points, for the last `draw`.
    public func viewPoint(_ p: CGPoint) -> CGPoint {
        CGPoint(x: fit.origin.x + p.x * fit.scale, y: fit.origin.y + p.y * fit.scale)
    }

    /// View points → user units, for the last `draw`.
    public func userPoint(_ p: CGPoint) -> CGPoint {
        CGPoint(x: (p.x - fit.origin.x) / fit.scale, y: (p.y - fit.origin.y) / fit.scale)
    }

    private var userTolerance: CGFloat { tolerance / fit.scale }

    // MARK: Drawing

    /// Draw the picture fitted into `rect`, then the selection and the drag
    /// in flight, in a **y-down** context (a `UIView`, a flipped `NSView`).
    /// `scale` is the context's device pixels per point.
    public func draw(in context: CGContext, rect: CGRect, scale: CGFloat) {
        let size = document.picture?.size ?? CGSize(width: 1, height: 1)
        let s = min(rect.width / size.width, rect.height / size.height)
        let drawn = CGSize(width: size.width * s, height: size.height * s)
        let origin = CGPoint(x: rect.minX + (rect.width - drawn.width) / 2, y: rect.minY + (rect.height - drawn.height) / 2)
        fit = (s, origin)

        context.setFillColor(CGColor(gray: 1, alpha: 1))
        context.fill(CGRect(origin: origin, size: drawn))
        document.picture?.draw(in: context, rect: rect, scale: scale)

        let accent = CGColor(red: 0.0, green: 0.48, blue: 1.0, alpha: 1)
        context.setStrokeColor(accent)
        context.setLineWidth(1)

        if let id = selection, let bounds = selectionBounds(id) {
            let r = viewRect(bounds)
            context.setLineDash(phase: 0, lengths: [])
            context.stroke(r)
            context.setFillColor(CGColor(gray: 1, alpha: 1))
            for h in Handle.all {
                let p = h.position(on: r)
                let box = CGRect(x: p.x - 3.5, y: p.y - 3.5, width: 7, height: 7)
                context.fill(box)
                context.stroke(box)
            }
        }

        if let outline = inFlightOutline() {
            context.setLineDash(phase: 0, lengths: [4, 3])
            switch outline {
            case .box(let r): context.stroke(viewRect(r))
            case .segment(let a, let b):
                context.move(to: viewPoint(a))
                context.addLine(to: viewPoint(b))
                context.strokePath()
            }
            context.setLineDash(phase: 0, lengths: [])
        }
    }

    private func viewRect(_ r: CGRect) -> CGRect {
        let o = viewPoint(r.origin)
        return CGRect(x: o.x, y: o.y, width: r.width * fit.scale, height: r.height * fit.scale)
    }

    /// The selection's box as drawn: its stated bounds, or a nominal box
    /// around a label's anchor, since a `<text>`'s extent is its font's.
    private func selectionBounds(_ id: String) -> CGRect? {
        guard let shape = document.shape(id: id) else { return nil }
        if shape.kind == .text, let b = document.bounds(id: id) {
            return CGRect(x: b.minX - 2, y: b.minY - 12, width: 24, height: 14)
        }
        return document.bounds(id: id)
    }

    private enum Outline { case box(CGRect), segment(CGPoint, CGPoint) }

    private func inFlightOutline() -> Outline? {
        switch drag {
        case .move(_, let start, let delta):
            return .box(start.offsetBy(dx: delta.dx, dy: delta.dy))
        case .resize(_, let handle, let start, let delta):
            return .box(handle.drag(start, by: delta))
        case .create(let from, let to):
            if tool == .line { return .segment(from, to) }
            return .box(CGRect(from: from, to: to))
        case nil:
            return nil
        }
    }

    // MARK: Pointer

    /// Pointer down at a view point.
    public func pointerDown(at viewPoint: CGPoint) {
        let p = userPoint(viewPoint)
        switch tool {
        case .select:
            if let id = selection, let b = selectionBounds(id),
               let h = Handle.at(p, on: b, tolerance: userTolerance), document.shape(id: id)?.kind != .text {
                drag = .resize(id: id, handle: h, start: b, delta: .zero)
            } else if let hit = document.hit(p, tolerance: userTolerance), let id = hit.id {
                selection = id
                if let b = selectionBounds(id) { drag = .move(id: id, start: b, delta: .zero) }
            } else {
                selection = nil
            }
        case .text(let text):
            selection = try? document.addText(text, at: p)
            tool = .select
        case .rect, .ellipse, .line:
            drag = .create(from: p, to: p)
        }
        needsDisplay?()
    }

    /// Pointer moved to a view point with the button down.
    public func pointerDragged(to viewPoint: CGPoint) {
        let p = userPoint(viewPoint)
        switch drag {
        case .move(let id, let start, _):
            let origin = userPoint(startViewPoint ?? viewPoint)
            drag = .move(id: id, start: start, delta: CGVector(dx: p.x - origin.x, dy: p.y - origin.y))
        case .resize(let id, let handle, let start, _):
            let origin = userPoint(startViewPoint ?? viewPoint)
            drag = .resize(id: id, handle: handle, start: start, delta: CGVector(dx: p.x - origin.x, dy: p.y - origin.y))
        case .create(let from, _):
            drag = .create(from: from, to: p)
        case nil:
            return
        }
        needsDisplay?()
    }

    /// The view point the drag began at, remembered by `pointerDown`.
    private var startViewPoint: CGPoint?

    /// Pointer up: the gesture in flight lands as one splice.
    public func pointerUp() {
        defer { drag = nil; startViewPoint = nil; needsDisplay?() }
        switch drag {
        case .move(let id, _, let delta) where delta != .zero:
            try? document.move(id: id, by: delta)
        case .resize(let id, let handle, let start, let delta) where delta != .zero:
            try? document.resize(id: id, to: handle.drag(start, by: delta))
        case .create(let from, let to):
            let box = CGRect(from: from, to: to)
            guard box.width > 0 || box.height > 0 else { return }
            switch tool {
            case .rect: selection = try? document.addRect(box)
            case .ellipse: selection = try? document.addEllipse(in: box)
            case .line: selection = try? document.addLine(from: from, to: to)
            default: break
            }
            tool = .select
        default:
            break
        }
    }

    // MARK: Commands

    /// Delete the selection.
    public func deleteSelection() {
        guard let id = selection else { return }
        try? document.delete(id: id)
    }

    /// Reorder the selection.
    public func reorderSelection(_ order: Order) {
        guard let id = selection else { return }
        try? document.reorder(id: id, order)
    }

    public func undo() { try? document.undo() }
    public func redo() { try? document.redo() }

    /// Select a shape by id (or nothing).
    public func select(_ id: String?) {
        selection = id
        needsDisplay?()
    }
}

extension CanvasModel {
    /// Pointer down, remembering where, so a drag reports its delta from
    /// here. Views call this rather than `pointerDown(at:)`.
    public func beginPointer(at viewPoint: CGPoint) {
        startViewPoint = viewPoint
        pointerDown(at: viewPoint)
    }
}

extension CGRect {
    init(from a: CGPoint, to b: CGPoint) {
        self.init(x: min(a.x, b.x), y: min(a.y, b.y), width: abs(b.x - a.x), height: abs(b.y - a.y))
    }
}
