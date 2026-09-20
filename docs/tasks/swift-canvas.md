---
title: The canvas view, on the Mac and on iOS
description: An AppKit and a UIKit view over DrawingDocument that draws the SVG through resvg-swift and puts selection, handles and the in-flight shape on top
author: adammharris
status: open
created: 2026-09-19
updated: 2026-09-19
part_of: '[Tasks](tasks.md)'
---
# The canvas view, on the Mac and on iOS

`packages/svg-editor-swift` has `DrawingDocument` — the gestures with
Foundation types at the edges — and no view. The canvas is:

- **Display** by handing the SVG to resvg-swift's `SVGPicture` and drawing it
  into the view's `CGContext`; the editor draws no SVG itself. resvg-swift
  is scaffolded and not yet released (its own `docs/tasks/first-release.md`),
  so `Package.swift` does not depend on it yet; when it has a version, add
  `.package(url: "https://github.com/diaryx-org/resvg-swift.git", from:
  …)` and the `SvgEditor` target's dependency on `ResvgCoreGraphics`.
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
