---
title: Archive
part_of: '[thorn](/README.md)'
---
# Archive

The Swift frontend thorn no longer builds, archived on 2026-10-28 for the
Dioxus editor, `crates/thorn-dioxus`, drawn by the Blitz fork in a window
(`apps/thorn-dioxus`), as leaf and Diaryx moved before it. What is here
stays readable as a guide to what each Apple platform needs: the Pencil and
a palm, the pinch, the document system, the keyboard that pans a label's
field into view, and the Dioxus path has not done all of them yet.

Each kept its path under `archive/`: `packages/thorn-swift` is
`archive/packages/thorn-swift`. Nothing here builds as it stands. The
workspace, CI, `ship` and the xtask no longer name any of it, and the paths
from here to the live crates (`../../crates/thorn-svg-ffi`) are one level
short. To build one, check out `v0.2.0`, the last release that built it.

| archived | what it was |
| -------- | ----------- |
| [`Package.swift`](Package.swift), [`packages/thorn-swift`](packages/thorn-swift) | `Thorn`, the AppKit and UIKit canvas (`CanvasModel`, `DrawingCanvasView`, `DrawingEditor`), over the UniFFI binding committed in `uniffi-generated/`, as a Swift package resolved from the repository root |
| [`apps/thorn-editor`](apps/thorn-editor) | Thorn for the Mac and iOS (`Thorn.app`, `org.diaryx.thorn`), an xcodegen project over the package; its icon is `App/thorn.icon` |
| [`scripts/`](scripts) | the binding's generator and drift check, the Swift package's tests, and the Homebrew cask's template |
| [`xtask/src`](xtask/src) | `cargo xtask swift`, `package` (the signed, notarised `.dmg`) and `sync-versions` (the Xcode project's marketing version) |
| [`.github/workflows/mac-app.yml`](.github/workflows/mac-app.yml) | the release's `.dmg` and the tap's `thorn-editor` cask |

`crates/thorn-svg-ffi` is not archived: Diaryx's app calls it as a Rust
crate. Its Swift binding is no longer generated or checked.

## What still points here

- **The Homebrew `thorn-editor` cask** stays at the last release that built
  the Swift app, until the Dioxus app has a bundle of its own.
- **SwiftPM.** The repository root has no `Package.swift` from this commit
  on, so a version range that resolves to a later tag fails. A pin to an
  earlier commit or tag still resolves.

## What the Dioxus path does not do yet

[What the Dioxus app does not do yet](../docs/tasks/what-the-dioxus-app-does-not-do-yet.md)
names each, and the archived files that did it.
