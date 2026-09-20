//! The UniFFI bindings generator, in-tree so its version tracks the `uniffi`
//! runtime this crate links. Invoked by `scripts/gen-bindings.sh`:
//!
//! ```sh
//! cargo run -p svg-editor-ffi --bin uniffi-bindgen -- \
//!   generate --library <libsvg_editor_ffi.dylib> --language swift --out-dir <dir>
//! ```
fn main() {
    uniffi::uniffi_bindgen_main()
}
