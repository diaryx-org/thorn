---
title: The canvas view, on the Mac and on iOS
description: An AppKit and a UIKit view over DrawingDocument that draws the SVG through resvg-swift and puts selection, handles and the in-flight shape on top
author: adammharris
status: done
created: 2026-09-19
updated: 2026-09-20
part_of: '[Tasks](tasks.md)'
---
# The canvas view, on the Mac and on iOS

**Done 2026-09-20**, in the commit that closes this. `CanvasModel` is the
canvas with no view in it — tool, selection, the drag in flight, the
view↔user fit, and drawing into any `CGContext`; `DrawingCanvasView` is an
`NSView` or a `UIView` over it; `DrawingEditor` is the SwiftUI toolbar and
canvas. `Package.swift` depends on resvg-swift 0.1.1 for the picture.
`apps/thorn-mac` is a window around it for seeing a change work.
What is still open moved to `hit-testing.md` (a `<path>`/`<g>` and measured
text bounds) and `style-template.md`. resvg-swift 0.1.1 places a
`<marker>` wrongly at any fit but 1:1; fixed there in `b1ccb73`, released
as its next version.

`packages/thorn-swift` has `DrawingDocument` — the gestures with
Foundation types at the edges — and no view. The canvas is:

- **Display** by handing the SVG to resvg-swift's `SVGPicture` and drawing it
  into the view's `CGContext`; the editor draws no SVG itself. resvg-swift
  is scaffolded and not yet released (its own `docs/tasks/first-release.md`),
  so `Package.swift` does not depend on it yet; when it has a version, add
  `.package(url: "https://github.com/diaryx-org/resvg-swift.git", from:
  …)` and the `Thorn` target's dependency on `ResvgCoreGraphics`.
- **Hit-testing and handles**, which are the core's
  (`docs/tasks/hit-testing.md`) and reach here through the binding; the view
  only draws what the core says is selected and where its handles are.
- **A toolbar** on each platform: rectangle, ellipse, line, arrow, text,
  select; forward and back; group and ungroup; delete; undo and redo.
- **In-flight geometry.** During a drag the view keeps the numbers and draws
  the shape itself over the picture; the gesture lands as one splice on
  pointer-up.

The app's `AttachmentDetail` opens this as its own document; nothing here
embeds in leaf's editing surface.
