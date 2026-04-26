import AppKit
import SwiftUI

/// The top search / text-input bar.
struct SearchBar: View {
    @Binding var text: String
    @FocusState private var isFocused: Bool
    let onSubmit: () -> Void
    /// Called when the user presses ↑ while the text field is focused.
    var onMoveUp: (() -> Void)? = nil
    /// Called when the user presses ↓ while the text field is focused.
    var onMoveDown: (() -> Void)? = nil

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: "magnifyingglass")
                .foregroundStyle(.secondary)
                .imageScale(.large)
                .fontWeight(.medium)

            TextField("Search apps, files, commands…", text: $text)
                .textFieldStyle(.plain)
                .font(.system(size: RoverGlass.searchFontSize))
                .focused($isFocused)
                .onSubmit(onSubmit)
                // Intercept navigation keys directly on the TextField so AppKit
                // never gets a chance to move focus or scroll before we act.
                .onKeyPress(phases: .down) { key in
                    switch key.key {
                    case .upArrow:
                        onMoveUp?(); return .handled
                    case .downArrow:
                        onMoveDown?(); return .handled
                    case .tab:
                        // Shift+Tab → move up; Tab → move down.
                        // Must intercept here before AppKit cycles focus.
                        if key.modifiers.contains(.shift) {
                            onMoveUp?()
                        } else {
                            onMoveDown?()
                        }
                        return .handled
                    case .escape:
                        NotificationCenter.default.post(name: .roverDismiss, object: nil)
                        return .handled
                    default:
                        return .ignored
                    }
                }

            if !text.isEmpty {
                Button {
                    text = ""
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundStyle(.tertiary)
                        .imageScale(.medium)
                }
                .buttonStyle(.plain)
                .transition(.scale.combined(with: .opacity))
            }
        }
        .padding(.horizontal, 18)
        .padding(.vertical, 12)
        .onAppear { isFocused = true }
        .animation(.easeInOut(duration: 0.15), value: text.isEmpty)
    }
}
