---
title: The canvas view, on the Mac and on iOS
description: An AppKit and a UIKit view over DrawingDocument that draws the SVG through resvg-swift and puts selection, handles and the in-flight shape on top
author: adammharris
status: done
created: 2026-09-19
updated: 2026-09-20
part_of: '[Closed tasks](/docs/tasks/closed/closed.md)'
---
# The canvas view, on the Mac and on iOS

**Done 2026-09-20**, in the commit that closes this. `CanvasModel` is the
canvas with no view in it — tool, selection, the drag in flight, the
view↔user fit, and drawing into any `CGContext`; `DrawingCanvasView` is an
`NSView` or a `UIView` over it; `DrawingEditor` is the SwiftUI toolbar and
canvas. `Package.swift` depends on resvg-swift 0.1.2 for the picture:
the canvas found that 0.1.1 placed a `<marker>` wrongly at any fit but 1:1
(fixed there in `b1ccb73`), and 0.1.2's `SVGPicture.fitTransform(in:)` is
the fit the canvas maps clicks through, so it cannot disagree with what was
drawn. `apps/thorn-mac` is a window around it for seeing a change work.
What was still open moved to `hit-testing.md` (a `<path>`/`<g>` and measured
text bounds — since done there) and `style-template.md`.

`packages/thorn-swift` has `DrawingDocument` — the gestures with
Foundation types at the edges — and no view. The canvas is:

- **Display** by handing the SVG to resvg-swift's `SVGPicture` and drawing it
  into the view's `CGContext`; the editor draws no SVG itself. resvg-swift
  is scaffolded and not yet released (its own `docs/tasks/closed/first-release.md`),
  so `Package.swift` does not depend on it yet; when it has a version, add
  `.package(url: "https://github.com/diaryx-org/resvg-swift.git", from:
  …)` and the `Thorn` target's dependency on `ResvgCoreGraphics`.
- **Hit-testing and handles**, which are the core's
  (`docs/tasks/closed/hit-testing.md`) and reach here through the binding; the view
  only draws what the core says is selected and where its handles are.
- **A toolbar** on each platform: rectangle, ellipse, line, arrow, text,
  select; forward and back; group and ungroup; delete; undo and redo.
- **In-flight geometry.** During a drag the view keeps the numbers and draws
  the shape itself over the picture; the gesture lands as one splice on
  pointer-up. *(Revised: every drag — a move, a resize, an endpoint, a
  shape being created — is applied to the document as the pointer moves,
  each application undone before the next, so the picture follows the
  hand and the gesture is still one undo step.)*

The app's `AttachmentDetail` opens this as its own document; nothing here
embeds in leaf's editing surface.
