// swift-tools-version:5.9
//
// The Swift package an AppKit/UIKit app links to edit a drawing through the
// Rust core. The manifest lives at the repository root — not in
// packages/thorn-swift/ — because SwiftPM resolves git dependencies only
// from a root Package.swift, and by-version resolution is how a consumer is
// meant to take this package:
//
//   .package(url: "https://github.com/diaryx-org/thorn.git", from: "X.Y.Z")
//
// It builds the UniFFI binding + the Thorn wrapper **from source**; the
// Rust staticlib itself is linked by the consuming app. Two Rust staticlibs
// cannot share one executable, so an app that already links a Rust FFI crate
// of its own makes `thorn-svg-ffi` a Cargo dependency of that crate and
// force-loads the one archive; an app with no Rust of its own builds
// crates/thorn-svg-ffi's staticlib and force-loads that. README.md → Linking.
//
// The two `uniffi-generated/` inputs below are committed — a version-resolved
// clone runs no generators, so they must build as-is. `scripts/gen-bindings.sh`
// writes them from crates/thorn-svg-ffi and CI holds them to it (`--check`).
//
// `Thorn` draws the picture through resvg-swift's `ResvgCoreGraphics`,
// whose `resvg_uniffi` symbols ride inside this repository's staticlib
// (crates/thorn-svg-ffi depends on resvg-uniffi for exactly that), so the
// host still force-loads one archive.
import PackageDescription

let package = Package(
    name: "Thorn",
    platforms: [.macOS(.v13), .iOS(.v16)],
    products: [
        // The low-level binding: `Drawing` and the value types.
        .library(name: "ThornFFI", targets: ["ThornFFI"]),
        // The Swift-shaped layer over it.
        .library(name: "Thorn", targets: ["Thorn"]),
    ],
    dependencies: [
        .package(url: "https://github.com/diaryx-org/resvg-swift.git", from: "0.1.4"),
    ],
    targets: [
        // The C ABI as a clang module (`import thorn_svg_ffiFFI`). No library
        // to link here — the app force-loads the Rust `.a`, so the symbols the
        // generated Swift references stay undefined until the final link.
        .systemLibrary(
            name: "thorn_svg_ffiFFI",
            path: "packages/thorn-swift/uniffi-generated/headers"
        ),
        // The generated Swift, compiled against that C module.
        .target(
            name: "ThornFFI",
            dependencies: ["thorn_svg_ffiFFI"],
            path: "packages/thorn-swift/uniffi-generated/Sources/ThornFFI"
        ),
        // The document wrapper, the canvas view (AppKit / UIKit) and the
        // SwiftUI editor with its toolbar (committed source).
        .target(
            name: "Thorn",
            dependencies: [
                "ThornFFI",
                .product(name: "ResvgCoreGraphics", package: "resvg-swift"),
            ],
            path: "packages/thorn-swift/Sources/Thorn"
        ),
        // Drives a real drawing, so it needs the Rust staticlib:
        // `scripts/test-swift.sh` force-loads it (plain `swift test` won't
        // find the `.a`).
        .testTarget(
            name: "ThornTests",
            dependencies: ["Thorn"],
            path: "packages/thorn-swift/Tests/ThornTests"
        ),
    ]
)
