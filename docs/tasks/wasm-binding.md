---
title: The wasm binding, for the composer
description: thorn-svg-wasm, reaching the web composer the way leaf-wasm reaches leaf-web; nothing in the first milestone needs it
author: adammharris
status: open
created: 2026-09-19
updated: 2026-09-19
part_of: '[Tasks](tasks.md)'
---
# The wasm binding, for the composer

The composer is the second consumer and the reason the core is Rust.
`crates/thorn-svg-wasm` over `thorn-svg-core` through `wasm-bindgen`, the
shape leaf-wasm has, with the same surface as the FFI crate. twig-sys has a
`wasm32` payload, so the core already compiles there.

Nothing in the first milestone needs it; opened so the layout is stated.
