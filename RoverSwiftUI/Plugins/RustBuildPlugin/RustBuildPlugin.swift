import PackagePlugin
import Foundation

/// SPM CommandPlugin — compiles the Rust `rover_swift` crate and regenerates
/// UniFFI Swift bindings before a Swift build.
///
/// Usage (from repo root or RoverSwiftUI/):
///   swift package --package-path RoverSwiftUI \
///     --allow-writing-to-package-directory \
///     plugin rust-build
///
/// Xcode / xcodebuild: handled automatically via the XcodeGen pre-build
/// script phase (see project.yml at the repo root).
@main struct RustBuildPlugin: CommandPlugin {
    func performCommand(context: PluginContext, arguments: [String]) async throws {
        let tool = try context.tool(named: "RustBuilder")
        let repoRoot = context.package.directoryURL
            .deletingLastPathComponent()
            .path(percentEncoded: false)

        print("▶ RustBuildPlugin: invoking RustBuilder…")
        let process = Process()
        process.executableURL = tool.url
        process.arguments = [repoRoot]
        try process.run()
        process.waitUntilExit()

        guard process.terminationStatus == 0 else {
            throw PluginError.toolFailed(exitCode: process.terminationStatus)
        }
        print("✓ RustBuildPlugin: done.")
    }
}

enum PluginError: Error, CustomStringConvertible {
    case toolFailed(exitCode: Int32)
    var description: String {
        switch self {
        case .toolFailed(let code): return "RustBuilder exited with code \(code)"
        }
    }
}
