import AppKit
import RoverCore
import SwiftUI

/// Root launcher view — search bar + inline cards + result list.
struct LauncherView: View {
    @State private var vm: LauncherViewModel

    init(vm: LauncherViewModel) {
        _vm = State(initialValue: vm)
    }

    var body: some View {
        VStack(spacing: 0) {
            // ── Search bar ────────────────────────────────────────────────
            SearchBar(text: $vm.query, onSubmit: { vm.confirm() })
                .padding(.horizontal, 16)
                .padding(.vertical, 10)

            if !vm.cards.isEmpty || !vm.items.isEmpty {
                Divider()
            }

            // ── Inline cards ─────────────────────────────────────────────
            if !vm.cards.isEmpty {
                VStack(spacing: 6) {
                    ForEach(vm.cards.indices, id: \.self) { i in
                        InlineCardView(card: vm.cards[i])
                    }
                }
                .padding(.horizontal, 12)
                .padding(.top, 8)

                if !vm.items.isEmpty { Divider().padding(.top, 6) }
            }

            // ── Result list ───────────────────────────────────────────────
            if !vm.items.isEmpty {
                ScrollViewReader { proxy in
                    ScrollView {
                        LazyVStack(spacing: 0) {
                            ForEach(vm.items.indices, id: \.self) { i in
                                ResultRowView(
                                    item: vm.items[i],
                                    isSelected: i == vm.selectedIndex
                                )
                                .id(i)
                                .onTapGesture { vm.selectedIndex = i; vm.confirm() }
                            }
                        }
                    }
                    .onChange(of: vm.selectedIndex) { _, newIndex in
                        withAnimation(.easeInOut(duration: 0.1)) {
                            proxy.scrollTo(newIndex, anchor: .center)
                        }
                    }
                }
            }
        }
        .frame(width: 640)
        .fixedSize(horizontal: false, vertical: true)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .onKeyPress(.downArrow) { vm.moveDown(); return .handled }
        .onKeyPress(.upArrow) { vm.moveUp(); return .handled }
        .onKeyPress(.escape) {
            NotificationCenter.default.post(name: .roverDismiss, object: nil)
            return .handled
        }
    }
}

// MARK: - SearchBar

private struct SearchBar: View {
    @Binding var text: String
    let onSubmit: () -> Void

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: "magnifyingglass")
                .foregroundStyle(.secondary)
                .imageScale(.large)

            TextField("Search apps, files, commands…", text: $text)
                .textFieldStyle(.plain)
                .font(.system(size: 18, weight: .regular))
                .onSubmit(onSubmit)

            if !text.isEmpty {
                Button { text = "" } label: {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundStyle(.secondary)
                }
                .buttonStyle(.plain)
            }
        }
    }
}

// MARK: - ResultRowView

private struct ResultRowView: View {
    let item: PickerItem
    let isSelected: Bool

    var body: some View {
        HStack(spacing: 10) {
            ItemIconView(iconName: item.iconName)
                .foregroundStyle(isSelected ? .white : .primary)

            VStack(alignment: .leading, spacing: 1) {
                Text(item.title)
                    .font(.system(size: 14, weight: .medium))
                    .lineLimit(1)
                    .foregroundStyle(isSelected ? .white : .primary)

                if let sub = item.subtitle {
                    Text(sub)
                        .font(.system(size: 12))
                        .lineLimit(1)
                        .foregroundStyle(isSelected ? .white.opacity(0.7) : .secondary)
                }
            }

            Spacer()
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 7)
        .background(isSelected ? Color.accentColor : .clear, in: RoundedRectangle(cornerRadius: 6))
        .padding(.horizontal, 4)
    }
}

// MARK: - ItemIconView

/// Resolves icon: SF Symbol if NSImage finds a match, otherwise app bundle icon.
private struct ItemIconView: View {
    let iconName: String?

    var body: some View {
        Group {
            if let name = iconName {
                if isSFSymbol(name) {
                    Image(systemName: name)
                        .imageScale(.large)
                        .frame(width: 28, height: 28)
                } else {
                    AppIconView(bundleID: name)
                        .frame(width: 28, height: 28)
                }
            } else {
                Image(systemName: "square.grid.2x2")
                    .imageScale(.large)
                    .frame(width: 28, height: 28)
            }
        }
    }

    /// NSImage can reliably tell us whether a symbol exists.
    private func isSFSymbol(_ name: String) -> Bool {
        guard name.count < 60 else { return false }
        return NSImage(systemSymbolName: name, accessibilityDescription: nil) != nil
    }
}

// MARK: - InlineCardView

private struct InlineCardView: View {
    let card: InlineCard

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: iconName)
                .foregroundStyle(.secondary)
            Text(card.content)
                .font(.system(size: 15, weight: .medium, design: .monospaced))
                .lineLimit(4)
            Spacer()
        }
        .padding(10)
        .background(.quaternary, in: RoundedRectangle(cornerRadius: 8))
    }

    private var iconName: String {
        switch card.kind {
        case .calculator: "equal"
        case .ai: "sparkles"
        case .terminal: "terminal"
        case .info: "info.circle"
        }
    }
}

// MARK: - AppIconView

/// Renders the icon for a given bundle ID using the modern NSWorkspace API.
private struct AppIconView: NSViewRepresentable {
    let bundleID: String

    func makeNSView(context: Context) -> NSImageView {
        let view = NSImageView()
        view.imageScaling = .scaleProportionallyUpOrDown
        return view
    }

    func updateNSView(_ view: NSImageView, context: Context) {
        if let url = NSWorkspace.shared.urlForApplication(withBundleIdentifier: bundleID) {
            view.image = NSWorkspace.shared.icon(forFile: url.path)
        } else {
            view.image = NSImage(systemSymbolName: "app", accessibilityDescription: nil)
        }
    }
}
