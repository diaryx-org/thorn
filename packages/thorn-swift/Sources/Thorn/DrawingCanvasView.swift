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

    // The canvas follows the view's appearance: light or dark ink, sheet
    // and desk. Read when the view lands in a window and whenever the
    // system, or the app, changes it.
    public override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        followAppearance()
    }

    public override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        followAppearance()
    }

    private func followAppearance() {
        let dark = effectiveAppearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
        model.appearance = dark ? .dark : .light
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

public final class DrawingCanvasView: UIView, UITextViewDelegate, UIGestureRecognizerDelegate, UIEditMenuInteractionDelegate {
    public let model: CanvasModel

    /// How far a finger may miss a stroke or a handle, and how big a handle
    /// is drawn: a fingertip's, where the Mac's are a pointer's. Set on the
    /// model when the view is made; a host with a pointer can set its own.
    public static let touchTolerance: CGFloat = 12
    public static let touchHandleSize: CGFloat = 11

    public init(model: CanvasModel) {
        self.model = model
        super.init(frame: .zero)
        // The draw covers the whole view — desk, sheet, picture — so the
        // layer is opaque, and a bounds change (a rotation, a keyboard)
        // redraws rather than stretching the last bitmap to fit.
        isOpaque = true
        contentMode = .redraw
        // Two fingers are a pinch or a pan; the drag the first began is
        // cancelled rather than landed (see `touchesCancelled`).
        isMultipleTouchEnabled = true
        model.tolerance = Self.touchTolerance
        model.handleSize = Self.touchHandleSize
        model.needsDisplay = { [weak self] in self?.setNeedsDisplay() }
        model.onTextEdit = { [weak self] edit in self?.openField(for: edit) }

        let doubleTap = UITapGestureRecognizer(target: self, action: #selector(doubleTapped(_:)))
        doubleTap.numberOfTapsRequired = 2
        addGestureRecognizer(doubleTap)

        let pinch = UIPinchGestureRecognizer(target: self, action: #selector(pinched(_:)))
        pinch.delegate = self
        addGestureRecognizer(pinch)

        // Two fingers dragged together pan, whatever the tool: a phone has
        // no scroll wheel and the hand tool is a tile away.
        let twoFingers = UIPanGestureRecognizer(target: self, action: #selector(panned(_:)))
        twoFingers.minimumNumberOfTouches = 2
        twoFingers.maximumNumberOfTouches = 2
        twoFingers.delegate = self
        addGestureRecognizer(twoFingers)

        // A trackpad's or a mouse wheel's scroll, on an iPad: a pan
        // recogniser that takes scroll events and no touches at all, so a
        // mouse drag is still the pointer's.
        let scroll = UIPanGestureRecognizer(target: self, action: #selector(scrolled(_:)))
        scroll.allowedScrollTypesMask = .all
        scroll.maximumNumberOfTouches = 0
        addGestureRecognizer(scroll)

        // A long press on a shape offers what the phone's toolbar keeps
        // behind "More": layers, grouping, delete, and the label's field.
        let longPress = UILongPressGestureRecognizer(target: self, action: #selector(longPressed(_:)))
        addGestureRecognizer(longPress)
        addInteraction(UIEditMenuInteraction(delegate: self))

        let center = NotificationCenter.default
        center.addObserver(self, selector: #selector(keyboardWillShow(_:)), name: UIResponder.keyboardWillShowNotification, object: nil)
        center.addObserver(self, selector: #selector(keyboardWillHide(_:)), name: UIResponder.keyboardWillHideNotification, object: nil)
    }

    deinit { NotificationCenter.default.removeObserver(self) }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    // MARK: Zoom and pan

    /// A pinch zooms about its centre. `scale` is cumulative from the
    /// pinch's start, so it is reset after each step is taken.
    @objc private func pinched(_ g: UIPinchGestureRecognizer) {
        guard g.state == .began || g.state == .changed else { return }
        model.zoom(by: g.scale, about: g.location(in: self))
        g.scale = 1
    }

    /// Two fingers pan by how far they moved since the last step.
    @objc private func panned(_ g: UIPanGestureRecognizer) {
        guard g.state == .began || g.state == .changed else { return }
        let t = g.translation(in: self)
        model.pan(by: CGVector(dx: t.x, dy: t.y))
        g.setTranslation(.zero, in: self)
    }

    /// A scroll pans; with the command key it zooms about the pointer, as
    /// the Mac's does.
    @objc private func scrolled(_ g: UIPanGestureRecognizer) {
        guard g.state == .began || g.state == .changed else { return }
        let t = g.translation(in: self)
        if g.modifierFlags.contains(.command) {
            model.zoom(by: exp(t.y / 100), about: g.location(in: self))
        } else {
            model.pan(by: CGVector(dx: t.x, dy: t.y))
        }
        g.setTranslation(.zero, in: self)
    }

    /// The pinch and the two-finger pan are one gesture with two readings.
    public func gestureRecognizer(_ g: UIGestureRecognizer, shouldRecognizeSimultaneouslyWith other: UIGestureRecognizer) -> Bool {
        (g is UIPinchGestureRecognizer && other is UIPanGestureRecognizer)
            || (g is UIPanGestureRecognizer && other is UIPinchGestureRecognizer)
    }

    // MARK: The long-press menu

    /// The interaction the long press presents through; found rather than
    /// kept, since the view owns it.
    private var editMenu: UIEditMenuInteraction? {
        interactions.lazy.compactMap { $0 as? UIEditMenuInteraction }.first
    }

    /// The touch that became a long press is the pointer's no more: it is
    /// cancelled here, before the shape under it is picked, since the
    /// cancel the recogniser sends afterwards would put the selection
    /// back as it was.
    @objc private func longPressed(_ g: UILongPressGestureRecognizer) {
        guard g.state == .began else { return }
        let at = g.location(in: self)
        pointer = nil
        model.cancelPointer()
        guard let id = model.shape(at: at) else { return }
        if !model.selection.contains(id) { model.select(id) }
        editMenu?.presentEditMenu(with: UIEditMenuConfiguration(identifier: nil, sourcePoint: at))
    }

    public func editMenuInteraction(_ interaction: UIEditMenuInteraction, menuFor configuration: UIEditMenuConfiguration, suggestedActions: [UIMenuElement]) -> UIMenu? {
        let model = model
        var items: [UIMenuElement] = []
        if model.canEditText {
            items.append(UIAction(title: "Edit Text") { _ in model.editSelectedText() })
        }
        // Nested, so the strip is Edit Text / Arrange / Group / Delete and
        // not four layer commands paging Delete out of sight.
        items.append(UIMenu(title: "Arrange", children: [
            UIAction(title: "Bring to Front") { _ in model.reorderSelection(.toFront) },
            UIAction(title: "Bring Forward") { _ in model.reorderSelection(.forward) },
            UIAction(title: "Send Backward") { _ in model.reorderSelection(.backward) },
            UIAction(title: "Send to Back") { _ in model.reorderSelection(.toBack) },
        ]))
        if model.canGroup {
            items.append(UIAction(title: "Group") { _ in model.groupSelection() })
        }
        if model.canUngroup {
            items.append(UIAction(title: "Ungroup") { _ in model.ungroupSelection() })
        }
        items.append(UIAction(title: "Delete", attributes: .destructive) { _ in model.deleteSelection() })
        return UIMenu(children: items)
    }

    // MARK: Typing a label

    /// The field over a label being typed, the edit it is for, and where
    /// it goes in user units — the frame and the font size the edit gave
    /// in view points, taken back through the fit they were given under —
    /// so the field can be put back over the label after every draw. The
    /// view under a keyboard is resized by its host and re-fits the
    /// picture; a field left at its old view point would sit over nothing.
    private var field: (view: UITextView, edit: TextEdit, frame: CGRect, fontSize: CGFloat)?

    /// Where the keyboard is, in screen coordinates, while it is up.
    private var keyboard: CGRect?

    private func openField(for edit: TextEdit) {
        closeField(commit: true)
        let f = UITextView(frame: edit.frame)
        f.text = edit.text
        f.backgroundColor = UIColor.systemBackground.withAlphaComponent(0.9)
        f.textContainerInset = .zero
        f.textContainer.lineFragmentPadding = 0
        // The field grows with what is typed (see `layoutField`) rather
        // than scrolling inside a fixed frame.
        f.isScrollEnabled = false
        f.returnKeyType = .done
        f.autocorrectionType = .no
        f.inputAccessoryView = accessoryBar(for: f)
        f.delegate = self
        addSubview(f)
        field = (f, edit, model.userRect(edit.frame), edit.fontSize / model.viewScale)
        layoutField()
        f.becomeFirstResponder()
        f.selectAll(nil)
    }

    /// Put the field where the label is under the current fit, at the
    /// label's size, grown to what has been typed — down with every line,
    /// and, unless the label wraps at its width, across with every
    /// character; never smaller than it opened. Then, if the keyboard
    /// would cover it, pan the picture up by the overlap and the field
    /// with it, so what is being typed stays in view. (The picture is left
    /// where it was panned to when the keyboard goes, as after any pan.)
    private func layoutField() {
        guard let (f, edit, userFrame, userFontSize) = field else { return }
        let fontSize = userFontSize * model.viewScale
        f.font = edit.fontName.flatMap { UIFont(name: $0, size: fontSize) } ?? .systemFont(ofSize: fontSize)
        var frame = model.viewRect(userFrame)
        let unbounded: CGFloat = 100_000
        if edit.wraps {
            frame.size.height = max(frame.height, f.sizeThatFits(CGSize(width: frame.width, height: unbounded)).height)
        } else {
            let fits = f.sizeThatFits(CGSize(width: unbounded, height: unbounded))
            frame.size = CGSize(width: max(frame.width, fits.width), height: max(frame.height, fits.height))
        }
        f.frame = frame

        // Below the keyboard's top, or the view's bottom where the host
        // already shrank the view to the keyboard.
        guard let keyboard, let window else { return }
        let covered = convert(window.convert(keyboard, from: nil), from: window)
        let margin: CGFloat = 12
        let overlap = frame.maxY + margin - min(covered.minY, bounds.maxY)
        guard overlap > 0 else { return }
        model.pan(by: CGVector(dx: 0, dy: -overlap))
        f.frame.origin.y -= overlap
    }

    /// Above the software keyboard, which has no Shift-Return: a line
    /// break, and Done.
    private func accessoryBar(for f: UITextView) -> UIView {
        let bar = UIToolbar(frame: CGRect(x: 0, y: 0, width: 320, height: 44))
        bar.items = [
            UIBarButtonItem(title: "New Line", primaryAction: UIAction { [weak f] _ in
                guard let f, let range = f.selectedTextRange else { return }
                f.replace(range, withText: "\n")
            }),
            UIBarButtonItem(systemItem: .flexibleSpace),
            UIBarButtonItem(systemItem: .done, primaryAction: UIAction { [weak self] _ in self?.closeField(commit: true) }),
        ]
        bar.sizeToFit()
        return bar
    }

    private func closeField(commit: Bool) {
        guard let (f, edit, _, _) = field else { return }
        field = nil
        f.delegate = nil
        f.removeFromSuperview()
        if commit { model.commitTextEdit(edit, text: f.text ?? "") }
    }

    /// Return commits; the accessory bar breaks a line.
    public func textView(_ textView: UITextView, shouldChangeTextIn range: NSRange, replacementText text: String) -> Bool {
        guard text == "\n" else { return true }
        closeField(commit: true)
        return false
    }

    public func textViewDidChange(_ textView: UITextView) {
        layoutField()
    }

    public func textViewDidEndEditing(_ textView: UITextView) {
        closeField(commit: true)
    }

    /// Noted, and measured against at the next draw rather than now: a
    /// host that shrinks the view to the keyboard does so after this, and
    /// an overlap measured before that resize pans too far.
    @objc private func keyboardWillShow(_ note: Notification) {
        keyboard = (note.userInfo?[UIResponder.keyboardFrameEndUserInfoKey] as? NSValue)?.cgRectValue
        setNeedsDisplay()
    }

    @objc private func keyboardWillHide(_ note: Notification) {
        keyboard = nil
    }

    @objc private func doubleTapped(_ g: UITapGestureRecognizer) {
        model.doubleClick(at: g.location(in: self))
    }

    public override var canBecomeFirstResponder: Bool { true }

    /// A hardware keyboard's bare key is the model's: a tool's key, the
    /// lock, Escape, and Delete for the selection. A field being typed into
    /// is first responder instead.
    public override func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        for press in presses {
            guard let key = press.key, key.modifierFlags.subtracting([.shift, .alphaShift]).isEmpty else { continue }
            switch key.keyCode {
            case .keyboardDeleteOrBackspace, .keyboardDeleteForward:
                model.deleteSelection()
                return
            default:
                let c: Character? = key.keyCode == .keyboardEscape ? "\u{1B}" : key.charactersIgnoringModifiers.first
                if let c, model.key(c) { return }
            }
        }
        super.pressesBegan(presses, with: event)
    }

    /// The picture, and the field back over its label under the fit it
    /// was just drawn with.
    public override func draw(_ rect: CGRect) {
        guard let context = UIGraphicsGetCurrentContext() else { return }
        model.draw(in: context, rect: bounds, scale: contentScaleFactor)
        layoutField()
    }

    // The canvas follows the view's appearance: light or dark ink, sheet
    // and desk. Read when the view lands in a window and whenever the
    // system, or the app, changes it.
    public override func didMoveToWindow() {
        super.didMoveToWindow()
        followAppearance()
    }

    public override func traitCollectionDidChange(_ previousTraitCollection: UITraitCollection?) {
        super.traitCollectionDidChange(previousTraitCollection)
        if traitCollection.userInterfaceStyle != previousTraitCollection?.userInterfaceStyle {
            followAppearance()
        }
    }

    private func followAppearance() {
        model.appearance = traitCollection.userInterfaceStyle == .dark ? .dark : .light
    }

    /// A touch that lands on the canvas is the canvas's. An ancestor's
    /// recogniser — a navigation stack's swipe back, a scroll view's pan —
    /// would otherwise take a horizontal drag before it drew anything and
    /// cancel the touches, so a rectangle dragged sideways became a page
    /// popped (seen in the Diaryx app on iOS 27, where the pop is full-width).
    /// The view's own — the double-tap, the pinch, the pans — still begin;
    /// the long press only under the select tool, so a stroke that starts
    /// slowly is not taken for one.
    public override func gestureRecognizerShouldBegin(_ gestureRecognizer: UIGestureRecognizer) -> Bool {
        guard gestureRecognizer.view === self else { return false }
        if gestureRecognizer is UILongPressGestureRecognizer { return model.tool == .select }
        return true
    }

    /// The one touch the pointer is: the first down. A second finger is
    /// the pinch's or the pan's, and cancels the drag the first began.
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

    /// Every sample the touch coalesced since the last event, not just the
    /// last: a Pencil reports at 240 Hz and the ink keeps the whole line.
    public override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard let pointer, touches.contains(pointer) else { return }
        let samples = event?.coalescedTouches(for: pointer) ?? [pointer]
        model.pointerDragged(through: samples.map { $0.location(in: self) })
    }

    public override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard let pointer, touches.contains(pointer) else { return }
        self.pointer = nil
        model.pointerUp()
    }

    /// The system took the touches, or a recogniser did — the pinch, a
    /// pan, the long press: nothing lands.
    public override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard let pointer, touches.contains(pointer) else { return }
        self.pointer = nil
        model.cancelPointer()
    }
}
#endif
