// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "RoverSwiftUI",
    platforms: [.macOS(.v14)],
    products: [
        .executable(name: "RoverApp", targets: ["RoverApp"]),
    ],
    targets: [
        // ── Rust FFI bridge (pre-built dylib + generated UniFFI bindings) ──
        //
        // Run `cargo build -p rover_swift` then
        // `cargo run --bin uniffi-bindgen -- generate --library target/debug/librover_swift.dylib --language swift --out-dir RoverSwiftUI/Sources/Generated`
        // to populate Sources/Generated before building this package.
        .target(
            name: "RoverCore",
            path: "Sources/RoverCore",
            linkerSettings: [
                .unsafeFlags([
                    "-L", "../../target/debug",
                    "-lrover_swift",
                    "-rpath", "@executable_path/../Frameworks",
                ]),
            ]
        ),

        // ── Native Swift plugins ─────────────────────────────────────────
        .target(
            name: "RoverPlugins",
            dependencies: ["RoverCore"],
            path: "Sources/RoverPlugins"
        ),

        // ── SwiftUI app ──────────────────────────────────────────────────
        .executableTarget(
            name: "RoverApp",
            dependencies: ["RoverCore", "RoverPlugins"],
            path: "Sources/RoverApp"
        ),
    ]
)
