import AppKit
import RoverCore
import RoverPlugins
import SwiftUI

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    // MARK: - State

    private var panel: LauncherPanel!
    private var hotkey: GlobalHotkey!
    private var statusItem: NSStatusItem!
    private var registry: PluginRegistry!
    private var vm: LauncherViewModel!

    // MARK: - NSApplicationDelegate

    func applicationDidFinishLaunching(_ notification: Notification) {
        registry = PluginRegistry()
        // Order matters: higher-priority plugins are queried first.
        registry.register(BuiltinCommandsPlugin())   // always-visible rover commands
        registry.register(AppLauncherPlugin())
        registry.register(CalculatorPlugin())
        registry.register(ClipboardPlugin())
        registry.register(FileSearchPlugin())
        registry.register(TerminalPlugin())
        registry.register(BrowserPlugin())
        registry.register(EmojiPlugin())
        registry.register(AIPlugin())                // on-device Apple Intelligence
        registry.register(SemanticPlugin())          // fastembed semantic search

        vm = LauncherViewModel(registry: registry)
        panel = LauncherPanel(vm: vm)

        setupStatusItem()

        // Carbon hotkey: Cmd+Space.
        hotkey = GlobalHotkey { [weak self] in self?.panel.toggle() }

        // Use `.main` queue so the closure runs on the main thread; then
        // assert main-actor isolation to safely touch @MainActor properties.
        NotificationCenter.default.addObserver(
            forName: .roverDismiss, object: nil, queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated { self?.panel.hide() }
        }

        NotificationCenter.default.addObserver(
            forName: .roverOpenSettings, object: nil, queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated { self?.openSettings() }
        }

        NotificationCenter.default.addObserver(
            forName: .roverOpenBrowser, object: nil, queue: .main
        ) { [weak self] note in
            let url = note.object as? URL
            MainActor.assumeIsolated { self?.openBrowser(url: url) }
        }
    }

    // MARK: - Status item

    private func setupStatusItem() {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        let button = statusItem.button!
        button.image = NSImage(systemSymbolName: "magnifyingglass", accessibilityDescription: "Rover")
        button.action = #selector(statusItemClicked)
        button.target = self
    }

    @objc private func statusItemClicked() {
        panel.toggle()
    }

    // MARK: - Settings

    private func openSettings() {
        SettingsWindowController.shared.show()
    }

    // MARK: - Browser

    private func openBrowser(url: URL?) {
        BrowserWindowController.shared.navigate(to: url)
    }
}
