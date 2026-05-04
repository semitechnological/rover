/// RustBuilder — thin Swift executable invoked by RustBuildPlugin.
///
/// Argument 0 (implicit): path to this executable
/// Argument 1: repo root (parent of RoverSwiftUI/)
///
/// Performs the same steps as `build-swiftui.sh`:
///   1. cargo build -p rover_swift
///   2. Copy librover_swift.dylib → RoverSwiftUI/lib/
///   3. Run uniffi-bindgen (via eqswift) → generate Swift bindings
///   4. Copy generated files into the SPM source tree

import Foundation

// MARK: - Helpers

@discardableResult
func run(_ command: String, cwd: String? = nil, env: [String: String]? = nil) -> Int32 {
    let process = Process()
    process.executableURL = URL(fileURLWithPath: "/bin/bash")
    process.arguments = ["-c", command]
    if let cwd {
        process.currentDirectoryURL = URL(fileURLWithPath: cwd)
    }
    if let env {
        var combined = ProcessInfo.processInfo.environment
        combined.merge(env) { _, new in new }
        process.environment = combined
    }
    do {
        try process.run()
    } catch {
        fputs("RustBuilder: failed to launch: \(error)\n", stderr)
        return 1
    }
    process.waitUntilExit()
    return process.terminationStatus
}

func fail(_ message: String) -> Never {
    fputs("RustBuilder error: \(message)\n", stderr)
    exit(1)
}

// MARK: - Main

let args = CommandLine.arguments
guard args.count >= 2 else {
    fail("expected argument: <repoRoot>")
}

let repoRoot = args[1]
let profile = ProcessInfo.processInfo.environment["RELEASE"] != nil ? "release" : "debug"
let profileFlag = profile == "release" ? "--release" : ""

// Resolve eqswift (sibling of the rover checkout inside projects/)
// Repo is typically at: ~/projects/rover/.claude/worktrees/<name>/
// eqswift is at:        ~/projects/eqswift/
let repoURL = URL(fileURLWithPath: repoRoot)

// Walk up from the worktree to the shared projects dir.
// Structure: .../projects/rover/.claude/worktrees/<name>
// We go up 4 levels:  <name> → worktrees → .claude → rover → projects
var projectsURL = repoURL
for _ in 0..<4 { projectsURL = projectsURL.deletingLastPathComponent() }
let eqswift = projectsURL.appendingPathComponent("eqswift").path
let eqswiftCargo = "\(eqswift)/eq-swift/Cargo.toml"

guard FileManager.default.fileExists(atPath: eqswiftCargo) else {
    fail("""
    eqswift not found at \(eqswift)
    Clone it with:
      git clone https://github.com/semitechnological/eqswift \(eqswift)
    """)
}

// ── 1. Build Rust cdylib ────────────────────────────────────────────────────
print("▶ cargo build -p rover_swift (\(profile))…")
let cargoStatus = run("cargo build -p rover_swift \(profileFlag)", cwd: repoRoot)
guard cargoStatus == 0 else { fail("cargo build failed (exit \(cargoStatus))") }

let dylib = "\(repoRoot)/target/\(profile)/librover_swift.dylib"
guard FileManager.default.fileExists(atPath: dylib) else {
    fail("dylib not found at \(dylib)")
}

// ── 2. Copy dylib → RoverSwiftUI/lib/ ──────────────────────────────────────
let libDir = "\(repoRoot)/RoverSwiftUI/lib"
let dest   = "\(libDir)/librover_swift.dylib"
try! FileManager.default.createDirectory(atPath: libDir, withIntermediateDirectories: true)
if FileManager.default.fileExists(atPath: dest) { try! FileManager.default.removeItem(atPath: dest) }
try! FileManager.default.copyItem(atPath: dylib, toPath: dest)
print("  Copied dylib → RoverSwiftUI/lib/librover_swift.dylib")

// ── 3. Generate UniFFI Swift bindings ───────────────────────────────────────
let tmp = FileManager.default.temporaryDirectory
    .appendingPathComponent("rover_bindgen_\(Int.random(in: 1_000_000..<9_999_999))").path
try! FileManager.default.createDirectory(atPath: tmp, withIntermediateDirectories: true)
defer { try? FileManager.default.removeItem(atPath: tmp) }

print("▶ Generating Swift bindings (uniffi-bindgen)…")
let bindgenStatus = run("""
cargo run \
  --manifest-path '\(eqswiftCargo)' \
  --bin uniffi-bindgen \
  -- generate \
  --library '\(dylib)' \
  --language swift \
  --out-dir '\(tmp)'
""", cwd: repoRoot)
guard bindgenStatus == 0 else { fail("uniffi-bindgen failed (exit \(bindgenStatus))") }

// ── 4. Copy generated files into SPM source tree ─────────────────────────────
let ffiDir  = "\(repoRoot)/RoverSwiftUI/Sources/rover_swiftFFI"
let coreDir = "\(repoRoot)/RoverSwiftUI/Sources/RoverCore"
try! FileManager.default.createDirectory(atPath: ffiDir,  withIntermediateDirectories: true)
try! FileManager.default.createDirectory(atPath: coreDir, withIntermediateDirectories: true)

func cp(from src: String, to dst: String) {
    if FileManager.default.fileExists(atPath: dst) { try! FileManager.default.removeItem(atPath: dst) }
    try! FileManager.default.copyItem(atPath: src, toPath: dst)
}
cp(from: "\(tmp)/rover_swiftFFI.h",         to: "\(ffiDir)/rover_swiftFFI.h")
cp(from: "\(tmp)/rover_swiftFFI.modulemap", to: "\(ffiDir)/module.modulemap")
cp(from: "\(tmp)/rover_swift.swift",        to: "\(coreDir)/rover_swift.swift")

print("✓ Done. Build with: swift build --package-path RoverSwiftUI")
