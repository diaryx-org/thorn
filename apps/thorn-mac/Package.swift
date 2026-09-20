// swift-tools-version:5.9
//
// A window around `DrawingEditor`, for seeing a change work on the Mac. Its
// own package rather than a target of the root one because it force-loads
// the Rust staticlib with an unsafe linker flag, and a package with an
// unsafe flag anywhere in it cannot be consumed by version — which the
// root package must be. `cargo xtask mac [file.svg]` builds the archive and
// runs this; by hand that is:
//
//   cargo build -p thorn-svg-ffi
//   swift run --package-path apps/thorn-mac thorn-mac [file.svg]
import PackageDescription

let package = Package(
    name: "thorn-mac",
    platforms: [.macOS(.v13)],
    dependencies: [.package(path: "../..")],
    targets: [
        .executableTarget(
            name: "thorn-mac",
            dependencies: [.product(name: "Thorn", package: "thorn")],
            linkerSettings: [
                .unsafeFlags(["-Xlinker", "-force_load", "-Xlinker", "../../target/debug/libthorn_svg_ffi.a"]),
            ]
        ),
    ]
)
