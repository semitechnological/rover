import SwiftUI

/// The top search / text-input bar.
struct SearchBar: View {
    @Binding var text: String
    @FocusState private var isFocused: Bool
    let onSubmit: () -> Void

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
