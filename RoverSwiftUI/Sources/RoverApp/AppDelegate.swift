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
        // Build plugin registry with all native Swift plugins.
        registry = PluginRegistry()
        registry.register(AppLauncherPlugin())
        registry.register(CalculatorPlugin())
        registry.register(ClipboardPlugin())
        registry.register(FileSearchPlugin())
        registry.register(TerminalPlugin())
        registry.register(EmojiPlugin())

        // Create the launcher panel.
        vm = LauncherViewModel(registry: registry)
        panel = LauncherPanel(vm: vm)

        // Menu bar status item.
        setupStatusItem()

        // Global hotkey: Cmd+Space.
        hotkey = GlobalHotkey { [weak self] in self?.panel.toggle() }

        // Notification observers.
        NotificationCenter.default.addObserver(
            forName: .roverDismiss, object: nil, queue: .main
        ) { [weak self] _ in self?.panel.hide() }

        NotificationCenter.default.addObserver(
            forName: .roverOpenSettings, object: nil, queue: .main
        ) { [weak self] _ in self?.openSettings() }
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
        // Placeholder — implement SettingsView in a separate window.
        NSApp.sendAction(Selector(("showSettingsWindow:")), to: nil, from: nil)
    }
}
