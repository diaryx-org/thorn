// The canvas as a view: an `NSView` on macOS, a `UIView` on iOS, each a
// few lines over `CanvasModel` — draw what it says, hand it the pointer,
// and turn keys into its commands.

import CoreGraphics
import ThornFFI

#if canImport(AppKit)
import AppKit

public final class DrawingCanvasView: NSView, NSTextViewDelegate {
    public let model: CanvasModel

    public init(model: CanvasModel) {
        self.model = model
        super.init(frame: .zero)
        model.needsDisplay = { [weak self] in self?.needsDisplay = true }
        // After the click that asked: a field made first responder inside a
        // double-click's mouseDown takes the rest of that click as its own
        // and selects a word with it.
        model.onTextEdit = { [weak self] edit in DispatchQueue.main.async { self?.openField(for: edit) } }
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    // SVG is y-down; so is this view.
    public override var isFlipped: Bool { true }
    public override var acceptsFirstResponder: Bool { true }

    public override func draw(_ dirtyRect: NSRect) {
        guard let context = NSGraphicsContext.current?.cgContext else { return }
        model.draw(in: context, rect: bounds, scale: window?.backingScaleFactor ?? 1)
    }

    // MARK: Typing a label

    /// The field over a label being typed, and the edit it is for. A text
    /// view rather than a field, so a wrapped label wraps as it is typed
    /// at the width it will wrap to.
    private var field: (NSTextView, TextEdit)?

    private func openField(for edit: TextEdit) {
        closeField(commit: true)
        let f = NSTextView(frame: edit.frame)
        f.string = edit.text
        f.font = edit.fontName.flatMap { NSFont(name: $0, size: edit.fontSize) } ?? .systemFont(ofSize: edit.fontSize)
        f.backgroundColor = .textBackgroundColor
        f.isRichText = false
        f.textContainerInset = .zero
        f.textContainer?.lineFragmentPadding = 0
        // The field grows down with every line typed, and, unless the
        // label wraps at its width, across with every character.
        // Large, not infinite: AppKit clamps a layout constant past its
        // limit and logs it, and no field is a hundred thousand points.
        let unbounded: CGFloat = 100_000
        f.isVerticallyResizable = true
        f.minSize = edit.frame.size
        if edit.wraps {
            f.textContainer?.widthTracksTextView = true
            f.maxSize = CGSize(width: edit.frame.width, height: unbounded)
        } else {
            f.isHorizontallyResizable = true
            f.maxSize = CGSize(width: unbounded, height: unbounded)
            f.textContainer?.widthTracksTextView = false
            f.textContainer?.containerSize = CGSize(width: unbounded, height: unbounded)
        }
        f.delegate = self
        addSubview(f)
        field = (f, edit)
        window?.makeFirstResponder(f)
        f.selectAll(nil)
    }

    /// Take the field down, committing what it holds or leaving the label.
    private func closeField(commit: Bool) {
        guard let (f, edit) = field else { return }
        field = nil
        f.delegate = nil
        f.removeFromSuperview()
        if commit { model.commitTextEdit(edit, text: f.string) }
        window?.makeFirstResponder(self)
    }

    /// Focus leaving commits.
    public func textDidEndEditing(_ notification: Notification) {
        closeField(commit: true)
    }

    /// Return commits; Shift-Return breaks the line; Escape leaves the
    /// label as it was.
    public func textView(_ textView: NSTextView, doCommandBy selector: Selector) -> Bool {
        // Shift-Return arrives as `insertNewline:` like Return does; the
        // shift is on the event.
        let shifted = NSApp.currentEvent?.modifierFlags.contains(.shift) ?? false
        switch selector {
        case #selector(insertNewline(_:)) where shifted, #selector(insertLineBreak(_:)):
            textView.insertText("\n", replacementRange: textView.selectedRange())
        case #selector(insertNewline(_:)): closeField(commit: true)
        case #selector(cancelOperation(_:)): closeField(commit: false)
        default: return false
        }
        return true
    }

    public override func mouseDown(with event: NSEvent) {
        closeField(commit: true)
        window?.makeFirstResponder(self)
        let at = convert(event.locationInWindow, from: nil)
        if event.clickCount == 2 {
            model.doubleClick(at: at)
        } else {
            model.beginPointer(at: at, extending: event.modifierFlags.contains(.shift))
        }
    }

    public override func mouseDragged(with event: NSEvent) {
        model.pointerDragged(to: convert(event.locationInWindow, from: nil))
    }

    public override func mouseUp(with event: NSEvent) {
        model.pointerUp()
    }

    // MARK: Zoom and pan

    /// A pinch on the trackpad zooms about the pointer; `magnification` is
    /// the change since the last event, as a fraction of the current size.
    public override func magnify(with event: NSEvent) {
        model.zoom(by: 1 + event.magnification, about: convert(event.locationInWindow, from: nil))
    }

    /// A two-finger double-tap zooms in a step about the pointer.
    public override func smartMagnify(with event: NSEvent) {
        model.zoom(by: CanvasModel.zoomStep * CanvasModel.zoomStep, about: convert(event.locationInWindow, from: nil))
    }

    /// A scroll pans; with the command key it zooms about the pointer, as
    /// Excalidraw's does. A mouse wheel's deltas are in lines, a
    /// trackpad's in points.
    public override func scrollWheel(with event: NSEvent) {
        let lines: CGFloat = event.hasPreciseScrollingDeltas ? 1 : 10
        let dx = event.scrollingDeltaX * lines, dy = event.scrollingDeltaY * lines
        if event.modifierFlags.contains(.command) {
            model.zoom(by: exp(dy / 100), about: convert(event.locationInWindow, from: nil))
        } else {
            model.pan(by: CGVector(dx: dx, dy: dy))
        }
    }

    /// Delete deletes the selection; a bare key is the model's — a tool's
    /// key, the lock, Escape. A field being typed into is first responder
    /// instead, so its letters never reach here.
    public override func keyDown(with event: NSEvent) {
        switch event.keyCode {
        case 51, 117: model.deleteSelection() // delete, forward delete
        default:
            let bare = event.modifierFlags.intersection(.deviceIndependentFlagsMask).subtracting([.shift, .capsLock]).isEmpty
            if bare, let c = event.charactersIgnoringModifiers?.first, model.key(c) { return }
            super.keyDown(with: event)
        }
    }

    @objc public func undo(_ sender: Any?) { model.undo() }
    @objc public func redo(_ sender: Any?) { model.redo() }
    @objc public func delete(_ sender: Any?) { model.deleteSelection() }
    @objc public func group(_ sender: Any?) { model.groupSelection() }
    @objc public func ungroup(_ sender: Any?) { model.ungroupSelection() }
    @objc public func zoomIn(_ sender: Any?) { model.zoomIn() }
    @objc public func zoomOut(_ sender: Any?) { model.zoomOut() }
    @objc public func zoomToFit(_ sender: Any?) { model.zoomToFit() }
}

#elseif canImport(UIKit)
import UIKit

public final class DrawingCanvasView: UIView, UITextViewDelegate {
    public let model: CanvasModel

    public init(model: CanvasModel) {
        self.model = model
        super.init(frame: .zero)
        backgroundColor = .clear
        // Two fingers are a pinch; the drag the first began is cancelled
        // rather than landed (see `touchesCancelled`).
        isMultipleTouchEnabled = true
        model.needsDisplay = { [weak self] in self?.setNeedsDisplay() }
        model.onTextEdit = { [weak self] edit in self?.openField(for: edit) }
        let doubleTap = UITapGestureRecognizer(target: self, action: #selector(doubleTapped(_:)))
        doubleTap.numberOfTapsRequired = 2
        addGestureRecognizer(doubleTap)
        addGestureRecognizer(UIPinchGestureRecognizer(target: self, action: #selector(pinched(_:))))
    }

    // MARK: Zoom

    /// A pinch zooms about its centre. `scale` is cumulative from the
    /// pinch's start, so it is reset after each step is taken.
    @objc private func pinched(_ g: UIPinchGestureRecognizer) {
        guard g.state == .began || g.state == .changed else { return }
        model.zoom(by: g.scale, about: g.location(in: self))
        g.scale = 1
    }

    // MARK: Typing a label

    private var field: (UITextView, TextEdit)?

    private func openField(for edit: TextEdit) {
        closeField(commit: true)
        let f = UITextView(frame: edit.frame)
        f.text = edit.text
        f.font = edit.fontName.flatMap { UIFont(name: $0, size: edit.fontSize) } ?? .systemFont(ofSize: edit.fontSize)
        f.backgroundColor = UIColor.systemBackground.withAlphaComponent(0.9)
        f.textContainerInset = .zero
        f.textContainer.lineFragmentPadding = 0
        f.isScrollEnabled = edit.wraps
        f.returnKeyType = .done
        f.delegate = self
        addSubview(f)
        field = (f, edit)
        f.becomeFirstResponder()
        f.selectAll(nil)
    }

    private func closeField(commit: Bool) {
        guard let (f, edit) = field else { return }
        field = nil
        f.delegate = nil
        f.removeFromSuperview()
        if commit { model.commitTextEdit(edit, text: f.text ?? "") }
    }

    /// Return commits; a label is one paragraph.
    public func textView(_ textView: UITextView, shouldChangeTextIn range: NSRange, replacementText text: String) -> Bool {
        guard text == "\n" else { return true }
        closeField(commit: true)
        return false
    }

    public func textViewDidEndEditing(_ textView: UITextView) {
        closeField(commit: true)
    }

    @objc private func doubleTapped(_ g: UITapGestureRecognizer) {
        model.doubleClick(at: g.location(in: self))
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    public override var canBecomeFirstResponder: Bool { true }

    /// A hardware keyboard's bare key is the model's: a tool's key, the
    /// lock, Escape. A field being typed into is first responder instead.
    public override func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        for press in presses {
            guard let key = press.key, key.modifierFlags.subtracting([.shift, .alphaShift]).isEmpty else { continue }
            let c: Character? = key.keyCode == .keyboardEscape ? "\u{1B}" : key.charactersIgnoringModifiers.first
            if let c, model.key(c) { return }
        }
        super.pressesBegan(presses, with: event)
    }

    public override func draw(_ rect: CGRect) {
        guard let context = UIGraphicsGetCurrentContext() else { return }
        model.draw(in: context, rect: bounds, scale: contentScaleFactor)
    }

    /// The one touch the pointer is: the first down. A second finger is
    /// the pinch's, and cancels the drag the first began.
    private var pointer: UITouch?

    public override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard let t = touches.first else { return }
        if pointer != nil {
            pointer = nil
            model.cancelPointer()
            return
        }
        pointer = t
        closeField(commit: true)
        becomeFirstResponder()
        model.beginPointer(at: t.location(in: self))
    }

    public override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard let pointer, touches.contains(pointer) else { return }
        model.pointerDragged(to: pointer.location(in: self))
    }

    public override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard let pointer, touches.contains(pointer) else { return }
        self.pointer = nil
        model.pointerUp()
    }

    /// The system took the touches, or the pinch recognised and took them:
    /// nothing lands.
    public override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard let pointer, touches.contains(pointer) else { return }
        self.pointer = nil
        model.cancelPointer()
    }
}
#endif
