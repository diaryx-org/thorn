// The canvas as a view: an `NSView` on macOS, a `UIView` on iOS, each a
// few lines over `CanvasModel` — draw what it says, hand it the pointer,
// and turn keys into its commands.

import CoreGraphics
import ThornFFI

#if canImport(AppKit)
import AppKit

public final class DrawingCanvasView: NSView, NSTextFieldDelegate {
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

    // MARK: Typing a label

    /// The field over a label being typed, and the edit it is for.
    private var field: (NSTextField, TextEdit)?

    private func openField(for edit: TextEdit) {
        closeField(commit: true)
        let f = NSTextField(frame: edit.frame)
        f.stringValue = edit.text
        f.font = .systemFont(ofSize: edit.fontSize)
        f.isBordered = false
        f.focusRingType = .none
        f.backgroundColor = NSColor.textBackgroundColor.withAlphaComponent(0.9)
        f.delegate = self
        addSubview(f)
        field = (f, edit)
        window?.makeFirstResponder(f)
        f.currentEditor()?.selectAll(nil)
    }

    /// Take the field down, committing what it holds or leaving the label.
    private func closeField(commit: Bool) {
        guard let (f, edit) = field else { return }
        field = nil
        f.delegate = nil
        f.removeFromSuperview()
        if commit { model.commitTextEdit(edit, text: f.stringValue) }
        window?.makeFirstResponder(self)
    }

    /// Return commits and ends editing; focus leaving does the same.
    public func controlTextDidEndEditing(_ notification: Notification) {
        closeField(commit: true)
    }

    /// Escape leaves the label as it was.
    public func control(_ control: NSControl, textView: NSTextView, doCommandBy selector: Selector) -> Bool {
        guard selector == #selector(cancelOperation(_:)) else { return false }
        closeField(commit: false)
        return true
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

    public override func keyDown(with event: NSEvent) {
        switch event.keyCode {
        case 51, 117: model.deleteSelection() // delete, forward delete
        default: super.keyDown(with: event)
        }
    }

    @objc public func undo(_ sender: Any?) { model.undo() }
    @objc public func redo(_ sender: Any?) { model.redo() }
    @objc public func delete(_ sender: Any?) { model.deleteSelection() }
    @objc public func group(_ sender: Any?) { model.groupSelection() }
    @objc public func ungroup(_ sender: Any?) { model.ungroupSelection() }
}

#elseif canImport(UIKit)
import UIKit

public final class DrawingCanvasView: UIView, UITextFieldDelegate {
    public let model: CanvasModel

    public init(model: CanvasModel) {
        self.model = model
        super.init(frame: .zero)
        backgroundColor = .clear
        isMultipleTouchEnabled = false
        model.needsDisplay = { [weak self] in self?.setNeedsDisplay() }
        model.onTextEdit = { [weak self] edit in self?.openField(for: edit) }
        let doubleTap = UITapGestureRecognizer(target: self, action: #selector(doubleTapped(_:)))
        doubleTap.numberOfTapsRequired = 2
        addGestureRecognizer(doubleTap)
    }

    // MARK: Typing a label

    private var field: (UITextField, TextEdit)?

    private func openField(for edit: TextEdit) {
        closeField(commit: true)
        let f = UITextField(frame: edit.frame)
        f.text = edit.text
        f.font = .systemFont(ofSize: edit.fontSize)
        f.backgroundColor = UIColor.systemBackground.withAlphaComponent(0.9)
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

    public func textFieldShouldReturn(_ textField: UITextField) -> Bool {
        closeField(commit: true)
        return true
    }

    public func textFieldDidEndEditing(_ textField: UITextField) {
        closeField(commit: true)
    }

    @objc private func doubleTapped(_ g: UITapGestureRecognizer) {
        model.doubleClick(at: g.location(in: self))
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    public override var canBecomeFirstResponder: Bool { true }

    public override func draw(_ rect: CGRect) {
        guard let context = UIGraphicsGetCurrentContext() else { return }
        model.draw(in: context, rect: bounds, scale: contentScaleFactor)
    }

    public override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard let t = touches.first else { return }
        closeField(commit: true)
        becomeFirstResponder()
        model.beginPointer(at: t.location(in: self))
    }

    public override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard let t = touches.first else { return }
        model.pointerDragged(to: t.location(in: self))
    }

    public override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        model.pointerUp()
    }

    public override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        model.pointerUp()
    }
}
#endif
