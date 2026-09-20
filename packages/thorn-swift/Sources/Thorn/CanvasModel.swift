// What a canvas does, with no view in it: the tool, the selection, the drag
// in flight, the mapping between view points and user units, and the
// drawing of picture + selection + handles into any CGContext. The AppKit
// and UIKit views are thin over this, and it is testable with a bitmap.
//
// During a drag the model keeps the in-flight geometry, and applies it to
// the document as the pointer moves — a shape moved, resized, dragged by
// an end, or being created — so the picture follows the hand; each
// application is undone before the next, so that when the pointer lifts
// the gesture is one undo step, as if it had landed once.

import CoreGraphics
import Foundation
import ThornFFI

/// What the next pointer-down does. Excalidraw's toolset; `Toolset.swift`
/// has each one's name, symbol and keys, and which the core can make yet.
public enum Tool: Hashable {
    /// Pan the view by dragging.
    case hand
    /// Select, move by dragging, resize by a handle.
    case select
    case rect
    case diamond
    case ellipse
    case arrow
    case line
    /// Freehand ink.
    case draw
    /// Place a label: with this text, or, with `nil`, one the user types
    /// into a field the view opens at the click (see `TextEdit`).
    case text(String? = nil)
    /// A box with a label in it.
    case note
    /// Delete what is dragged over.
    case eraser
}

/// A label being typed: what the view puts a text field over. `id` is
/// the label being re-worded, or `nil` for a new one at `anchor`; `frame`
/// and `fontSize` are in view points, where the field goes. `wraps` says
/// the label has a width and the field should wrap at the frame's.
public struct TextEdit: Equatable {
    public let id: String?
    public let anchor: CGPoint
    public let text: String
    public let frame: CGRect
    public let fontSize: CGFloat
    /// The face's PostScript name, when the layout could say.
    public let fontName: String?
    public let wraps: Bool
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
    /// Keep a create tool after the shape it made, rather than falling
    /// back to select: Excalidraw's lock.
    public var locked = false {
        didSet { if locked != oldValue { onLockChange?(locked) } }
    }
    /// Called when `locked` changes.
    public var onLockChange: ((Bool) -> Void)?
    /// How far the picture has been dragged from where it fits, in view
    /// points: the hand tool's doing.
    public private(set) var pan: CGVector = .zero

    /// The selected shapes' `data-id`s, in the order they were picked. A
    /// click on a member of a group selects the outermost group; a
    /// shift-click adds to or takes from the selection.
    public private(set) var selection: [String] = [] {
        didSet { if selection != oldValue { onSelectionChange?(selection) } }
    }
    /// Called when the selection changes.
    public var onSelectionChange: (([String]) -> Void)?
    /// Called when the picture or overlay needs redrawing.
    public var needsDisplay: (() -> Void)?
    /// Called to open a text field for a label — a double-click on one, or
    /// a click with the label tool. The view calls `commitTextEdit` with
    /// what was typed, or nothing to leave the label as it was.
    public var onTextEdit: ((TextEdit) -> Void)?

    /// How far, in view points, a click may miss a stroke or a handle.
    public var tolerance: CGFloat = 4
    /// The width of a stroke the draw tool makes, in user units.
    public var inkWidth: CGFloat = 3

    /// User units → view points for the last `draw`: asked of the picture, so
    /// it is the fit `SVGPicture.draw` drew under and cannot disagree with it.
    private var fit: CGAffineTransform = .identity

    private enum Drag {
        /// The selection moving as one; `start` is the box around it all.
        case move(ids: [String], start: CGRect, delta: CGVector)
        case resize(id: String, handle: Handle, start: CGRect, delta: CGVector)
        /// An end of a line being dragged; `other` is the end staying put.
        case endpoint(id: String, end: End, other: CGPoint, to: CGPoint)
        case create(from: CGPoint, to: CGPoint)
        /// The draw tool: the centreline so far, in user units.
        case ink(points: [CGPoint])
        /// The hand: `start` is `pan` when the drag began.
        case pan(start: CGVector, from: CGPoint)
        /// The eraser: what it has passed over, deleted when it lifts.
        case erase(ids: [String])
    }
    private var drag: Drag?
    /// Whether the drag in flight has been applied to the document, and so
    /// must be undone before it is applied again or dropped.
    private var previewed = false
    /// The shape a create drag last added.
    private var created: String?

    public init(document: DrawingDocument) {
        self.document = document
        document.onChange = { [weak self] in
            guard let self else { return }
            selection = selection.filter { document.shape(id: $0) != nil }
            needsDisplay?()
        }
    }

    // MARK: Coordinates

    /// User units → view points, for the last `draw`.
    public func viewPoint(_ p: CGPoint) -> CGPoint { p.applying(fit) }

    /// View points → user units, for the last `draw`.
    public func userPoint(_ p: CGPoint) -> CGPoint { p.applying(fit.inverted()) }

    private var userTolerance: CGFloat { tolerance / fit.a }

    // MARK: Drawing

    /// Draw the picture fitted into `rect`, then the selection and the drag
    /// in flight, in a **y-down** context (a `UIView`, a flipped `NSView`).
    /// `scale` is the context's device pixels per point.
    public func draw(in context: CGContext, rect: CGRect, scale: CGFloat) {
        guard let picture = document.picture else { return }
        let panned = rect.offsetBy(dx: pan.dx, dy: pan.dy)
        fit = picture.fitTransform(in: panned)

        context.setFillColor(CGColor(gray: 1, alpha: 1))
        context.fill(viewRect(CGRect(origin: .zero, size: picture.size)))
        picture.draw(in: context, rect: panned, scale: scale)

        let accent = CGColor(red: 0.0, green: 0.48, blue: 1.0, alpha: 1)
        context.setStrokeColor(accent)
        context.setLineWidth(1)

        context.setLineDash(phase: 0, lengths: [])
        for id in selection {
            guard let bounds = selectionBounds(id) else { continue }
            let r = viewRect(bounds)
            // A lone line is outlined as itself, its handles its ends —
            // filled when bound to a shape. Everything else is its box,
            // with handles when it can be resized by them; a group of
            // shapes moves as one and is resized one at a time.
            if selection.count == 1, let ends = endpoints(id), ends.count == 2 {
                context.move(to: viewPoint(ends[0].1))
                context.addLine(to: viewPoint(ends[1].1))
                context.strokePath()
                for (end, p) in ends {
                    let bound = document.binding(id: id, end) != nil
                    context.setFillColor(bound ? accent : CGColor(gray: 1, alpha: 1))
                    handleBox(at: viewPoint(p), in: context)
                }
                continue
            }
            context.stroke(r)
            guard resizable == id else { continue }
            context.setFillColor(CGColor(gray: 1, alpha: 1))
            for h in Handle.all {
                handleBox(at: h.position(on: r), in: context)
            }
        }

        // A shape being created has no box yet the document shows until
        // it is big enough to add; its first pixel is drawn here.
        if case .create(let from, let to) = drag, !previewed {
            context.setLineDash(phase: 0, lengths: [4, 3])
            let box = viewRect(CGRect(from: from, to: to))
            switch tool {
            case .line, .arrow:
                context.move(to: viewPoint(from))
                context.addLine(to: viewPoint(to))
                context.strokePath()
            case .diamond:
                context.move(to: CGPoint(x: box.midX, y: box.minY))
                context.addLine(to: CGPoint(x: box.maxX, y: box.midY))
                context.addLine(to: CGPoint(x: box.midX, y: box.maxY))
                context.addLine(to: CGPoint(x: box.minX, y: box.midY))
                context.closePath()
                context.strokePath()
            case .ellipse:
                context.strokeEllipse(in: box)
            default:
                context.stroke(box)
            }
            context.setLineDash(phase: 0, lengths: [])
        }

        // What the eraser has passed over is outlined until it lifts.
        if case .erase(let ids) = drag {
            context.setStrokeColor(CGColor(red: 0.9, green: 0.2, blue: 0.2, alpha: 1))
            context.setLineDash(phase: 0, lengths: [4, 3])
            for id in ids {
                guard let bounds = selectionBounds(id) else { continue }
                context.stroke(viewRect(bounds))
            }
            context.setLineDash(phase: 0, lengths: [])
        }
    }

    private func viewRect(_ r: CGRect) -> CGRect { r.applying(fit) }

    private func handleBox(at p: CGPoint, in context: CGContext) {
        let box = CGRect(x: p.x - 3.5, y: p.y - 3.5, width: 7, height: 7)
        context.fill(box)
        context.stroke(box)
    }

    /// A selected shape's box as drawn.
    private func selectionBounds(_ id: String) -> CGRect? {
        document.bounds(id: id)
    }

    /// A `<line>`'s ends, in user units — its handles; `nil` for any other
    /// kind.
    private func endpoints(_ id: String) -> [(End, CGPoint)]? {
        guard document.shape(id: id)?.kind == .line else { return nil }
        return [End.from, .to].compactMap { end in document.endPoint(id: id, end).map { (end, $0) } }
    }

    /// The box around the whole selection.
    private func selectionBounds() -> CGRect? {
        selection.compactMap(selectionBounds).reduce(nil as CGRect?) { acc, r in acc.map { $0.union(r) } ?? r }
    }

    /// Whether a lone selected shape can be resized by its box handles: a
    /// line is dragged by its ends instead. A label's handles set the
    /// width it wraps to.
    private var resizable: String? {
        guard selection.count == 1, let id = selection.first,
              let shape = document.shape(id: id), shape.kind != .line else { return nil }
        return id
    }

    // MARK: Pointer

    /// Pointer down at a view point. `extending` is the shift key: the hit
    /// shape joins or leaves the selection instead of replacing it, and
    /// nothing drags.
    public func pointerDown(at viewPoint: CGPoint, extending: Bool = false) {
        let p = userPoint(viewPoint)
        switch tool {
        case .select:
            if selection.count == 1, let id = selection.first, let ends = endpoints(id),
               let (end, at) = ends.first(where: { abs($0.1.x - p.x) <= userTolerance && abs($0.1.y - p.y) <= userTolerance }),
               let other = ends.first(where: { $0.0 != end })?.1 {
                drag = .endpoint(id: id, end: end, other: other, to: at)
            } else if let id = resizable, let b = selectionBounds(id),
               let h = Handle.at(p, on: b, tolerance: userTolerance) {
                drag = .resize(id: id, handle: h, start: b, delta: .zero)
            } else if let hit = document.hit(p, tolerance: userTolerance), let hitId = hit.id,
                      let id = document.outermost(id: hitId)?.id {
                if extending {
                    if let at = selection.firstIndex(of: id) { selection.remove(at: at) } else { selection.append(id) }
                } else {
                    // A click on what is already selected drags the lot.
                    if !selection.contains(id) { selection = [id] }
                    if let b = selectionBounds() { drag = .move(ids: selection, start: b, delta: .zero) }
                }
            } else if !extending {
                selection = []
            }
        case .hand:
            drag = .pan(start: pan, from: viewPoint)
        case .eraser:
            selection = []
            drag = .erase(ids: [])
            erase(at: p)
        case .text(let text):
            if let text {
                selection = (try? document.addText(text, at: p)).map { [$0] } ?? []
            } else {
                selection = []
                onTextEdit?(TextEdit(id: nil, anchor: p, text: "", frame: fieldFrame(at: viewPoint), fontSize: document.fontSize(id: nil) * fit.a, fontName: document.font(id: nil)?.postScriptName, wraps: false))
            }
            release()
        case .rect, .diamond, .ellipse, .arrow, .line, .note:
            drag = .create(from: p, to: p)
        case .draw:
            selection = []
            drag = .ink(points: [p])
        }
        needsDisplay?()
    }

    /// A one-shot tool's shape is made: back to select, unless locked.
    private func release() {
        if !locked, tool.isOneShot { tool = .select }
    }

    /// The eraser passing a user point: the outermost shape there joins
    /// what it will delete.
    private func erase(at p: CGPoint) {
        guard case .erase(var ids) = drag,
              let hit = document.hit(p, tolerance: userTolerance), let hitId = hit.id,
              let id = document.outermost(id: hitId)?.id, !ids.contains(id) else { return }
        ids.append(id)
        drag = .erase(ids: ids)
    }

    /// Pointer moved to a view point with the button down.
    public func pointerDragged(to viewPoint: CGPoint) {
        let p = userPoint(viewPoint)
        switch drag {
        case .move(let ids, let start, _):
            let origin = userPoint(startViewPoint ?? viewPoint)
            drag = .move(ids: ids, start: start, delta: CGVector(dx: p.x - origin.x, dy: p.y - origin.y))
        case .resize(let id, let handle, let start, _):
            let origin = userPoint(startViewPoint ?? viewPoint)
            drag = .resize(id: id, handle: handle, start: start, delta: CGVector(dx: p.x - origin.x, dy: p.y - origin.y))
        case .endpoint(let id, let end, let other, _):
            drag = .endpoint(id: id, end: end, other: other, to: p)
        case .create(let from, _):
            drag = .create(from: from, to: p)
        case .ink(var points):
            // A point the format could not tell from the last is noise.
            if let last = points.last, abs(last.x - p.x) < 0.5, abs(last.y - p.y) < 0.5 { return }
            points.append(p)
            drag = .ink(points: points)
        case .pan(let start, let from):
            pan = CGVector(dx: start.dx + viewPoint.x - from.x, dy: start.dy + viewPoint.y - from.y)
            needsDisplay?()
            return
        case .erase:
            erase(at: p)
            needsDisplay?()
            return
        case nil:
            return
        }
        preview()
        needsDisplay?()
    }

    /// Apply the drag in flight to the document, the previous application
    /// undone first, so the picture shows the gesture as it stands.
    private func preview() {
        if previewed { _ = try? document.undo() }
        previewed = apply()
    }

    /// Apply the drag in flight; `true` when it wrote something.
    private func apply() -> Bool {
        switch drag {
        case .move(let ids, _, let delta) where delta != .zero:
            if ids.count == 1 { try? document.move(id: ids[0], by: delta) } else { try? document.move(ids: ids, by: delta) }
            return true
        case .resize(let id, let handle, let start, let delta) where delta != .zero:
            try? document.resize(id: id, to: handle.drag(start, by: delta))
            return true
        case .endpoint(let id, let end, _, let to):
            // Dropped on a shape, the end binds to it and follows it from
            // now on; dropped on nothing, it is unbound.
            try? document.dropEnd(id: id, end, at: to, tolerance: userTolerance)
            return true
        case .create(let from, let to):
            let box = CGRect(from: from, to: to)
            guard box.width > 0 || box.height > 0 else { return false }
            switch tool {
            case .rect: created = try? document.addRect(box)
            case .diamond: created = try? document.addDiamond(in: box)
            case .note: created = try? document.addNote(in: box)
            case .ellipse: created = try? document.addEllipse(in: box)
            case .line: created = try? document.addLine(from: from, to: to)
            case .arrow: created = try? document.addArrow(from: from, to: to)
            default: return false
            }
            return created != nil
        case .ink(let points) where !points.isEmpty:
            // One point is a dot of the nib.
            created = try? document.addInk(points, width: inkWidth)
            return created != nil
        default:
            return false
        }
    }

    /// The view point the drag began at, remembered by `pointerDown`.
    private var startViewPoint: CGPoint?

    /// Pointer up: the gesture in flight lands as one splice — the last
    /// preview, which is already in the document.
    public func pointerUp() {
        defer { drag = nil; startViewPoint = nil; previewed = false; created = nil; needsDisplay?() }
        switch drag {
        case .move, .resize, .endpoint:
            if !previewed { previewed = apply() }
        case .create:
            if !previewed { previewed = apply() }
            selection = previewed ? created.map { [$0] } ?? [] : []
            let made = created
            release()
            // A new note opens its label for typing at once.
            if let id = made, document.note(id: id) != nil { editNote(id: id) }
        case .ink:
            if !previewed { previewed = apply() }
            selection = previewed ? created.map { [$0] } ?? [] : []
            release()
        case .erase(let ids) where !ids.isEmpty:
            // One undo step for the whole sweep.
            if ids.count == 1 { try? document.delete(id: ids[0]) } else { try? document.delete(ids: ids) }
        default:
            break
        }
    }

    /// A key with no modifier, as the canvas sees it: a tool's key picks
    /// the tool, `q` toggles the lock, Escape is select with nothing
    /// selected. `true` when the key meant something.
    @discardableResult
    public func key(_ key: Character) -> Bool {
        if key == "\u{1B}" {
            tool = .select
            select([])
        } else if key == Tool.lockKey {
            locked.toggle()
        } else if let picked = Tool.forKey(key), picked.isAvailable {
            tool = picked
        } else {
            return false
        }
        return true
    }

    /// A double-click at a view point: on a label, opens it for editing.
    public func doubleClick(at viewPoint: CGPoint) {
        guard tool == .select, let hit = document.hit(userPoint(viewPoint), tolerance: userTolerance),
              let hitId = hit.id else { return }
        if let outer = document.outermost(id: hitId)?.id, document.note(id: outer) != nil {
            editNote(id: outer)
        } else if hit.kind == .text {
            editText(id: hitId)
        }
    }

    /// Open a note's label for editing: the field fills the box inside
    /// its padding, and wraps at that width.
    public func editNote(id: String) {
        guard let note = document.note(id: id), let box = document.bounds(id: note.frame),
              let shape = document.shape(id: note.label) else { return }
        selection = [id]
        let pad = DrawingDocument.notePadding
        let inner = box.insetBy(dx: pad, dy: pad)
        let fontSize = document.fontSize(id: note.label) * fit.a
        onTextEdit?(TextEdit(id: note.label, anchor: inner.origin, text: shape.text ?? "", frame: viewRect(inner), fontSize: fontSize, fontName: document.font(id: note.label)?.postScriptName, wraps: true))
    }

    /// Open a label for editing.
    public func editText(id: String) {
        guard let shape = document.shape(id: id), shape.kind == .text, let bounds = document.bounds(id: id) else { return }
        selection = [id]
        let width = document.width(id: id)
        let fontSize = document.fontSize(id: id) * fit.a
        var frame = viewRect(bounds)
        if let width {
            frame.size.width = width * fit.a
            frame.size.height += fontSize * 1.3
        } else {
            frame.size.width = max(frame.width + fontSize, fontSize * 4)
        }
        onTextEdit?(TextEdit(id: id, anchor: bounds.origin, text: shape.text ?? "", frame: frame, fontSize: fontSize, fontName: document.font(id: id)?.postScriptName, wraps: width != nil))
    }

    /// What was typed: a new label at the edit's anchor, or the label's
    /// characters replaced; a newline is a line break of the label's own.
    /// Empty text adds nothing, and deletes a label being re-worded. One
    /// undo step either way.
    public func commitTextEdit(_ edit: TextEdit, text: String) {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        if let id = edit.id {
            if trimmed.isEmpty {
                // A note with nothing in it goes, box and all.
                let outer = document.outermost(id: id)?.id
                let victim = outer.flatMap { document.note(id: $0) != nil ? $0 : nil } ?? id
                try? document.delete(id: victim)
            } else if trimmed != edit.text { try? document.setText(id: id, trimmed) }
        } else if !trimmed.isEmpty {
            selection = (try? document.addText(trimmed, at: edit.anchor)).map { [$0] } ?? []
        }
        needsDisplay?()
    }

    /// Where a field for a new label goes: its baseline at the click.
    private func fieldFrame(at viewPoint: CGPoint) -> CGRect {
        let size = document.fontSize(id: nil) * fit.a
        return CGRect(x: viewPoint.x, y: viewPoint.y - size, width: size * 8, height: size * 1.3)
    }

    // MARK: Commands

    /// Delete the selection, as one undo step.
    public func deleteSelection() {
        guard !selection.isEmpty else { return }
        if selection.count == 1 { try? document.delete(id: selection[0]) } else { try? document.delete(ids: selection) }
    }

    /// Reorder a lone selected shape.
    public func reorderSelection(_ order: Order) {
        guard selection.count == 1 else { return }
        try? document.reorder(id: selection[0], order)
    }

    /// Wrap the selection in a group, which becomes the selection. Needs two
    /// or more shapes with one parent; otherwise nothing happens.
    public func groupSelection() {
        guard selection.count >= 2, let id = try? document.group(ids: selection) else { return }
        selection = [id]
    }

    /// Replace a lone selected group with its members, which become the
    /// selection.
    public func ungroupSelection() {
        guard selection.count == 1, let members = try? document.ungroup(id: selection[0]) else { return }
        selection = members
    }

    /// Whether `groupSelection` would do something.
    public var canGroup: Bool {
        selection.count >= 2 && Set(selection.compactMap { document.shape(id: $0)?.group }).count <= 1
            && selection.allSatisfy { document.shape(id: $0) != nil }
    }

    /// Whether `ungroupSelection` would do something.
    public var canUngroup: Bool {
        selection.count == 1 && document.shape(id: selection[0])?.kind == .group
    }

    public func undo() { try? document.undo() }
    public func redo() { try? document.redo() }

    /// Select shapes by id (or nothing).
    public func select(_ ids: [String]) {
        selection = ids
        needsDisplay?()
    }

    /// Select one shape by id (or nothing).
    public func select(_ id: String?) {
        select(id.map { [$0] } ?? [])
    }
}

extension CanvasModel {
    /// Pointer down, remembering where, so a drag reports its delta from
    /// here. Views call this rather than `pointerDown(at:)`.
    public func beginPointer(at viewPoint: CGPoint, extending: Bool = false) {
        startViewPoint = viewPoint
        pointerDown(at: viewPoint, extending: extending)
    }
}

extension CGRect {
    init(from a: CGPoint, to b: CGPoint) {
        self.init(x: min(a.x, b.x), y: min(a.y, b.y), width: abs(b.x - a.x), height: abs(b.y - a.y))
    }
}
