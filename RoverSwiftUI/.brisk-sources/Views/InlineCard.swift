import SwiftUI

/// A card shown above results: calculator result, AI response, terminal output.
struct InlineCardView: View {
    let card: InlineCard

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            Image(systemName: iconName)
                .foregroundStyle(.secondary)
                .imageScale(.medium)
                .padding(.top, 1)

            Text(card.content)
                .font(.system(size: 15, weight: .regular, design: .monospaced))
                .lineLimit(6)
                .textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
        .padding(12)
        .cardGlass()
    }

    private var iconName: String {
        switch card.kind {
        case .calculator: "equal.circle"
        case .ai:         "sparkles"
        case .terminal:   "terminal"
        case .info:       "info.circle"
        }
    }
}
