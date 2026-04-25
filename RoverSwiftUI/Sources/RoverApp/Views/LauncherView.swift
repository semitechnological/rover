import RoverCore
import SwiftUI

/// Root launcher view. Thin orchestrator — delegates to focused subviews.
struct LauncherView: View {
    @State private var vm: LauncherViewModel

    init(vm: LauncherViewModel) {
        _vm = State(initialValue: vm)
    }

    var body: some View {
        GlassEffectContainer {
            VStack(spacing: 0) {
                SearchBar(text: $vm.query, onSubmit: { vm.confirm() })

                if !vm.cards.isEmpty || !vm.items.isEmpty {
                    Divider().opacity(0.25)
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
        .onKeyPress(.downArrow)  { vm.moveDown(); return .handled }
        .onKeyPress(.upArrow)    { vm.moveUp();   return .handled }
        .onKeyPress(.escape) {
            NotificationCenter.default.post(name: .roverDismiss, object: nil)
            return .handled
        }
    }

    // MARK: - Sections

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
                        ResultRow(item: vm.items[i], isSelected: i == vm.selectedIndex)
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
