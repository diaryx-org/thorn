---
title: Text on iOS waits on resvg-swift's font fix
description: Every `<text>` is dropped on iOS until thorn-svg-ffi pins the resvg-uniffi that loads /System/Library/Fonts there
author: adammharris
status: open
created: 2026-09-21
updated: 2026-09-21
part_of: '[Tasks](tasks.md)'
---
# Text on iOS waits on resvg-swift's font fix

On iOS the canvas drew no text at all — a label typed into a note landed
in the file and rendered as nothing. `fontdb::load_system_fonts` has no
iOS branch: under `target_os = "ios"` it takes its Linux path and scans
`/usr/share/fonts`, which is empty, so usvg finds no face for `sans-serif`
and drops every `<text>`. resvg-swift's `resvg-uniffi` now loads
`/System/Library/Fonts` on iOS as well (its commit that closes the
matching change there); verified in the simulator against this checkout
with the crate patched in.

What is left is the pin: `crates/thorn-svg-ffi/Cargo.toml` says
`resvg-uniffi = "0.1.1"`, and the fix reaches the app only once
resvg-swift is released with it and that requirement moves to the
version that carries it. Then a fresh Diaryx build on a phone shows the
label. leaf-ffi carries resvg-uniffi on the same terms, so leaf's SVG
rendering on iOS has the same gap and the same pin to move.

Done when: the pin is at a released resvg-uniffi with the iOS branch, and
`cargo xtask ci` is green against crates.io with no patch on.
