---
title: What the Dioxus app does not do yet
part_of: '[Tasks](/docs/tasks/tasks.md)'
status: open
author: adammharris
created: 2026-10-28
description: What the archived Swift frontend did that thorn-dioxus and its app do not, and where in the archive each was done
---
# What the Dioxus app does not do yet

**Where.** `crates/thorn-dioxus` and `apps/thorn-dioxus`; the archived
Swift frontend (`archive/`) is where each of these was done.

**What.** The Dioxus editor has every tool and gesture the Swift one had —
the core makes them all — and the toolbar, the options strip and the zoom.
What it does not have yet:

- **A Mac document.** Thorn opens, saves and saves as through rfd's dialogs
  and a path; the Swift app was an `NSDocument` (`DocumentGroup`): autosave,
  Versions, Finder's Open With, recents, the close-with-changes question
  (`archive/apps/thorn-editor/App/ThornDocument.swift`). Leaf's
  `apps/leaf-dioxus/src/platform/macos/document.rs` is the pattern to take.
  Closing a window with unsaved changes asks nothing yet.
- **A bundle and a release.** No `Thorn.app` is built, signed or notarised,
  and no `.dmg` is attached to a release; the tap's `thorn-editor` cask stays
  where the last Swift release left it (`archive/xtask/src/package.rs`,
  `archive/.github/workflows/mac-app.yml`). The icon is
  `archive/apps/thorn-editor/App/thorn.icon`.
- **iOS.** Pencil-only drawing once a Pencil has touched the canvas, the
  Pencil's double-tap and squeeze for the eraser, coalesced samples into the
  ink, a finger's tolerance and handles, the two-finger pan, the long-press
  menu, and the keyboard that pans a label's field into view
  (`archive/packages/thorn-swift/Sources/Thorn/DrawingCanvasView.swift`).
  Diaryx's Dioxus app runs on iOS and is where the touch half belongs first.
- **The phone's bar.** The toolbar is the Mac's and an iPad's strip at
  every width; the Swift editor moved it to one bar along the bottom when
  the strip did not fit (`DrawingEditor.swift`'s `bar`).
- **A context menu.** The Swift canvas's long-press menu of layers, group,
  delete and edit; unit's `ContextMenu` is what to draw it with.
- **Tooltips.** Each tile's name and keys are its accessible label, which
  Blitz shows nowhere; the Swift toolbar's `.help` did.
- **The drag outside the canvas.** A drag that leaves the canvas lands
  there, as Blitz captures no pointer; AppKit's did, to the window's edge.
- **Pinch about the pointer on a touchscreen.** A trackpad's pinch zooms
  (winit's `PinchGesture`, about the last pointer position); a touchscreen's
  two fingers do nothing yet.
