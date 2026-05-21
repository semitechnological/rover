import AppKit

// `main.swift` top-level code is always synchronous and nonisolated.
// We bootstrap onto the main actor with `assumeIsolated` — safe because
// NSApplication.run() always executes on the main thread.
@main
struct RoverBriskMain {
    static func main() {
        MainActor.assumeIsolated {
            let app = NSApplication.shared
            let delegate = AppDelegate()
            app.delegate = delegate
            app.setActivationPolicy(.accessory) // no Dock icon
            app.run()
        }
    }
}
