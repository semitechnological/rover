import RoverCore
import SwiftUI

/// One result row in the picker list.
struct ResultRow: View {
    let item: PickerItem
    let isSelected: Bool

    var body: some View {
        HStack(spacing: 10) {
            ItemIcon(name: item.iconName)
                .foregroundStyle(isSelected ? .white : .primary)

            VStack(alignment: .leading, spacing: 2) {
                Text(item.title)
                    .font(.system(size: RoverGlass.rowFontSize, weight: .medium))
                    .lineLimit(1)
                    .foregroundStyle(isSelected ? .white : .primary)

                if let sub = item.subtitle {
                    Text(sub)
                        .font(.system(size: RoverGlass.rowSubtitleFontSize))
                        .lineLimit(1)
                        .foregroundStyle(isSelected ? .white.opacity(0.65) : .secondary)
                }
            }

            Spacer(minLength: 0)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 7)
        .contentShape(RoundedRectangle(cornerRadius: RoverGlass.rowRadius))
        .background {
            if isSelected {
                Color.clear.selectedRowBackground()
            }
        }
        .padding(.horizontal, 6)
        .animation(.easeInOut(duration: 0.1), value: isSelected)
    }
}
