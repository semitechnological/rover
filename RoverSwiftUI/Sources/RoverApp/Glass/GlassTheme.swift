import SwiftUI

// MARK: - Liquid Glass helpers (macOS 26 / Tahoe)
//
// All glass styling flows through this file so there's one place to update
// when Apple iterates the Liquid Glass API.

extension View {
    /// Full-panel glass — the primary launcher surface.
    func panelGlass() -> some View {
        self.glassEffect(in: RoundedRectangle(cornerRadius: RoverGlass.panelRadius, style: .continuous))
    }

    /// Card glass — inline calculator / AI / terminal cards.
    func cardGlass() -> some View {
        self.glassEffect(in: RoundedRectangle(cornerRadius: RoverGlass.cardRadius, style: .continuous))
    }

    /// Row selection highlight.
    ///
    /// Uses a solid fill only — no nested glassEffect. Stacking glassEffect inside
    /// GlassEffectContainer causes vibrancy conflicts that wash out text.
    func selectedRowBackground() -> some View {
        self.background(
            RoundedRectangle(cornerRadius: RoverGlass.rowRadius, style: .continuous)
                .fill(SwiftUI.Color.accentColor.opacity(0.22))
        )
    }

    /// Hover highlight for unselected rows.
    func hoverRowBackground() -> some View {
        self.background(
            RoundedRectangle(cornerRadius: RoverGlass.rowRadius, style: .continuous)
                .fill(SwiftUI.Color.primary.opacity(0.06))
        )
    }
}

// MARK: - Design tokens

enum RoverGlass {
    static let panelRadius: CGFloat = 16
    static let cardRadius: CGFloat = 10
    static let rowRadius: CGFloat = 8
    static let panelWidth: CGFloat = 660
    static let searchFontSize: CGFloat = 18
    static let rowFontSize: CGFloat = 14
    static let rowSubtitleFontSize: CGFloat = 12
}
