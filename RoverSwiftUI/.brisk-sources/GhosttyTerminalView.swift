import AppKit
import SwiftUI

public struct GhosttyTerminalView: NSViewRepresentable {
    let initialCommand: String?

    public init(initialCommand: String?) {
        self.initialCommand = initialCommand
    }

    public func makeNSView(context: Context) -> TerminalView {
        let view = TerminalView(frame: .zero)
        view.configuration = TerminalSurfaceOptions(
            backend: .exec,
            fontSize: 13,
            workingDirectory: FileManager.default.homeDirectoryForCurrentUser.path,
            context: .window
        )
        view.controller = TerminalController()
        if let initialCommand, !initialCommand.isEmpty {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.2) {
                view.sendText(initialCommand + "\n")
            }
        }
        return view
    }

    public func updateNSView(_ nsView: TerminalView, context: Context) {}
}
