# svg-editor-ffi

The UniFFI binding over [`svg-editor-core`](../svg-editor-core): one object,
`Drawing`, with the core's gestures as methods and its shapes and findings as
records. The committed Swift binding under
`packages/svg-editor-swift/uniffi-generated/` is generated from this crate by
`scripts/gen-bindings.sh`, and CI holds the two together.

A host links this crate's staticlib — or, when it already has a Rust FFI
crate of its own, depends on this one from there so the scaffolding lands in
its single archive, the way leaf-ffi carries resvg-uniffi.
