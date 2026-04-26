import RoverCore
import SwiftUI

// MARK: - Height preference key

private struct PanelHeightKey: PreferenceKey {
    static let defaultValue: CGFloat = 0
    static func reduce(value: inout CGFloat, nextValue: () -> CGFloat) { value = nextValue() }
}

// MARK: - LauncherView

/// Root launcher view. Thin orchestrator — delegates to focused subviews.
struct LauncherView: View {
    @State private var vm: LauncherViewModel
    /// Called whenever the view's natural height changes so the panel can resize.
    let onResize: ((CGFloat) -> Void)?

    init(vm: LauncherViewModel, onResize: ((CGFloat) -> Void)? = nil) {
        _vm = State(initialValue: vm)
        self.onResize = onResize
    }

    var body: some View {
        GlassEffectContainer {
            VStack(spacing: 0) {
                SearchBar(
                    text: $vm.query,
                    onSubmit: { vm.confirm() },
                    onMoveUp: { vm.moveUp() },
                    onMoveDown: { vm.moveDown() }
                )

                if !vm.cards.isEmpty || !vm.items.isEmpty {
                    Divider().opacity(0.25)
                } else if !vm.query.trimmingCharacters(in: .whitespaces).isEmpty {
                    emptyState
                }

                if !vm.cards.isEmpty {
                    cardSection
                }

                if !vm.items.isEmpty {
                    resultList
                }
            }
        }
        .frame(width: RoverGlass.panelWidth)
        .fixedSize(horizontal: false, vertical: true)
        .panelGlass()
        // Measure natural height and notify the panel.
        .background(
            GeometryReader { geo in
                Color.clear.preference(key: PanelHeightKey.self, value: geo.size.height)
            }
        )
        .onPreferenceChange(PanelHeightKey.self) { height in
            onResize?(height)
        }
        // ── Key handling ──────────────────────────────────────────────────────
        // SearchBar intercepts ↑↓ when TextField is focused; this catch-all
        // handles everything else (Tab, Esc, ⌘↵, ⌘1-9) and acts as a fallback
        // for ↑↓ when focus has moved elsewhere.
        .onKeyPress(phases: .down) { key in
            switch key.key {
            case .upArrow:
                vm.moveUp(); return .handled
            case .downArrow:
                vm.moveDown(); return .handled

            case .tab:
                // Tab / Shift+Tab cycle through results.
                if key.modifiers.contains(.shift) { vm.moveUp() } else { vm.moveDown() }
                return .handled

            case .escape:
                NotificationCenter.default.post(name: .roverDismiss, object: nil)
                return .handled

            case .return:
                if key.modifiers.contains(.command) {
                    vm.confirm(secondary: true)  // ⌘↵ = secondary action
                } else {
                    vm.confirm()
                }
                return .handled

            default:
                // ⌘1–⌘9 quick-pick: select and confirm the nth result.
                if key.modifiers == .command,
                   let digit = key.characters.first?.wholeNumberValue,
                   (1...9).contains(digit)
                {
                    let idx = digit - 1
                    guard idx < vm.items.count else { return .ignored }
                    vm.selectedIndex = idx
                    vm.confirm()
                    return .handled
                }
                return .ignored
            }
        }
    }

    // MARK: - Sections

    private var emptyState: some View {
        VStack(spacing: 8) {
            Image(systemName: "magnifyingglass")
                .font(.system(size: 28))
                .foregroundStyle(.tertiary)
            Text("No results")
                .font(.system(size: 13, weight: .medium))
                .foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 32)
        .transition(.opacity)
    }

    private var cardSection: some View {
        VStack(spacing: 6) {
            ForEach(vm.cards.indices, id: \.self) { i in
                InlineCardView(card: vm.cards[i])
            }
        }
        .padding(.horizontal, 10)
        .padding(.top, 8)
        .padding(.bottom, vm.items.isEmpty ? 10 : 4)
    }

    private var resultList: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(spacing: 0) {
                    ForEach(vm.items.indices, id: \.self) { i in
                        ResultRow(item: vm.items[i], isSelected: i == vm.selectedIndex, index: i)
                            .id(i)
                            .onTapGesture { vm.selectedIndex = i; vm.confirm() }
                    }
                }
                .padding(.vertical, 6)
            }
            .frame(maxHeight: 420)
            .onChange(of: vm.selectedIndex) { _, idx in
                withAnimation(.easeInOut(duration: 0.1)) {
                    proxy.scrollTo(idx, anchor: .center)
                }
            }
        }
    }
}
