// The canvas as a view: an `NSView` on macOS, a `UIView` on iOS, each a
// few lines over `CanvasModel` — draw what it says, hand it the pointer,
// and turn keys into its commands.

import CoreGraphics
import ThornFFI

#if canImport(AppKit)
import AppKit

public final class DrawingCanvasView: NSView {
    public let model: CanvasModel

    public init(model: CanvasModel) {
        self.model = model
        super.init(frame: .zero)
        model.needsDisplay = { [weak self] in self?.needsDisplay = true }
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
        window?.makeFirstResponder(self)
        model.beginPointer(at: convert(event.locationInWindow, from: nil))
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
}

#elseif canImport(UIKit)
import UIKit

public final class DrawingCanvasView: UIView {
    public let model: CanvasModel

    public init(model: CanvasModel) {
        self.model = model
        super.init(frame: .zero)
        backgroundColor = .clear
        isMultipleTouchEnabled = false
        model.needsDisplay = { [weak self] in self?.setNeedsDisplay() }
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
