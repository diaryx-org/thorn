---
title: The wasm binding, for the composer
description: svg-editor-wasm, reaching the web composer the way leaf-wasm reaches leaf-web; nothing in the first milestone needs it
author: adammharris
status: open
created: 2026-09-19
updated: 2026-09-19
part_of: '[Tasks](tasks.md)'
---
# The wasm binding, for the composer

The composer is the second consumer and the reason the core is Rust.
`crates/svg-editor-wasm` over `svg-editor-core` through `wasm-bindgen`, the
shape leaf-wasm has, with the same surface as the FFI crate. twig-sys has a
`wasm32` payload, so the core already compiles there.

Nothing in the first milestone needs it; opened so the layout is stated.
