//! The UniFFI bindings generator, in-tree so its version tracks the `uniffi`
//! runtime this crate links. Invoked by `scripts/gen-bindings.sh`:
//!
//! ```sh
//! cargo run -p thorn-svg-ffi --bin uniffi-bindgen -- \
//!   generate <libthorn_svg_ffi.dylib> --language swift --out-dir <dir>
//! ```
fn main() {
    uniffi::uniffi_bindgen_main()
}
