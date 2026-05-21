import SwiftUI

/// One result row in the picker list.
struct ResultRow: View {
    let item: PickerItem
    let isSelected: Bool
    /// Position in the result list (0-based). Used to show ⌘1–⌘9 shortcut badge.
    let index: Int

    @State private var isHovered = false

    var body: some View {
        HStack(spacing: 10) {
            ItemIcon(name: item.iconName)

            VStack(alignment: .leading, spacing: 2) {
                Text(item.title)
                    .font(.system(size: RoverGlass.rowFontSize, weight: .medium))
                    .lineLimit(1)
                    // Use .primary so vibrancy system (not us) controls contrast.
                    .foregroundStyle(.primary)

                if let sub = item.subtitle {
                    Text(sub)
                        .font(.system(size: RoverGlass.rowSubtitleFontSize))
                        .lineLimit(1)
                        .foregroundStyle(.secondary)
                }
            }

            Spacer(minLength: 0)

            // ⌘1–⌘9 shortcut badge for the first 9 results.
            if index < 9 {
                Text("⌘\(index + 1)")
                    .font(.system(size: 10, weight: .medium, design: .monospaced))
                    .foregroundStyle(isSelected ? .secondary : .tertiary)
                    .padding(.horizontal, 5)
                    .padding(.vertical, 2)
                    .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 4))
                    .animation(.easeInOut(duration: 0.1), value: isSelected)
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 7)
        .contentShape(RoundedRectangle(cornerRadius: RoverGlass.rowRadius))
        .background {
            if isSelected {
                SwiftUI.Color.clear.selectedRowBackground()
            } else if isHovered {
                SwiftUI.Color.clear.hoverRowBackground()
            }
        }
        .padding(.horizontal, 6)
        .onHover { isHovered = $0 }
        .animation(.easeInOut(duration: 0.1), value: isSelected)
        .animation(.easeInOut(duration: 0.08), value: isHovered)
    }
}
