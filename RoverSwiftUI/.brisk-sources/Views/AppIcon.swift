import AppKit
import SwiftUI

/// Resolves an icon for a picker item.
///
/// Resolution order:
///   1. SF Symbol (if NSImage confirms the symbol exists)
///   2. App bundle icon via bundle ID
///   3. Generic grid icon fallback
struct ItemIcon: View {
    let name: String?
    var size: CGFloat = 28

    var body: some View {
        Group {
            if let name {
                if isSFSymbol(name) {
                    Image(systemName: name)
                        .imageScale(.large)
                        .fontWeight(.medium)
                } else {
                    AppBundleIcon(bundleID: name, size: size)
                }
            } else {
                Image(systemName: "square.grid.2x2")
                    .imageScale(.large)
            }
        }
        .frame(width: size, height: size)
    }

    /// NSImage can tell us whether a symbol actually exists; Image(systemName:) cannot.
    private func isSFSymbol(_ name: String) -> Bool {
        name.count < 80 && NSImage(systemSymbolName: name, accessibilityDescription: nil) != nil
    }
}

// MARK: - AppBundleIcon

/// Renders the icon from a bundle ID using the modern NSWorkspace URL API.
struct AppBundleIcon: NSViewRepresentable {
    let bundleID: String
    var size: CGFloat = 28

    func makeNSView(context: Context) -> NSImageView {
        let v = NSImageView()
        v.imageScaling = .scaleProportionallyUpOrDown
        return v
    }

    func updateNSView(_ v: NSImageView, context: Context) {
        if let url = NSWorkspace.shared.urlForApplication(withBundleIdentifier: bundleID) {
            v.image = NSWorkspace.shared.icon(forFile: url.path)
        } else {
            v.image = NSImage(systemSymbolName: "app", accessibilityDescription: nil)
        }
    }
}
