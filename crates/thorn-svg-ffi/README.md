# thorn-svg-ffi

The UniFFI binding over [`thorn-svg-core`](../thorn-svg-core): one object,
`Drawing`, with the core's gestures as methods and its shapes and findings as
records. The Diaryx app calls it as a Rust crate. The Swift binding that was
generated from it went with the Swift frontend into the archive
(`archive/packages/thorn-swift/uniffi-generated/`), and nothing generates or
checks it now.

A host links this crate's staticlib — or, when it already has a Rust FFI
crate of its own, depends on this one from there so the scaffolding lands in
its single archive, the way leaf-ffi carries resvg-uniffi.
