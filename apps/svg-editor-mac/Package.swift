// swift-tools-version:5.9
//
// A window around `DrawingEditor`, for seeing a change work on the Mac. Its
// own package rather than a target of the root one because it force-loads
// the Rust staticlib with an unsafe linker flag, and a package with an
// unsafe flag anywhere in it cannot be consumed by version — which the
// root package must be. Build the archive first:
//
//   cargo build -p svg-editor-ffi
//   swift run --package-path apps/svg-editor-mac svg-editor-mac [file.svg]
import PackageDescription

let package = Package(
    name: "svg-editor-mac",
    platforms: [.macOS(.v13)],
    dependencies: [.package(path: "../..")],
    targets: [
        .executableTarget(
            name: "svg-editor-mac",
            dependencies: [.product(name: "SvgEditor", package: "svg-editor")],
            linkerSettings: [
                .unsafeFlags(["-Xlinker", "-force_load", "-Xlinker", "../../target/debug/libsvg_editor_ffi.a"]),
            ]
        ),
    ]
)
