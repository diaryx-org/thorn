# thorn-svg-ffi

The UniFFI binding over [`thorn-svg-core`](../thorn-svg-core): one object,
`Drawing`, with the core's gestures as methods and its shapes and findings as
records. The committed Swift binding under
`packages/thorn-swift/uniffi-generated/` is generated from this crate by
`scripts/gen-bindings.sh`, and CI holds the two together.

A host links this crate's staticlib — or, when it already has a Rust FFI
crate of its own, depends on this one from there so the scaffolding lands in
its single archive, the way leaf-ffi carries resvg-uniffi.
