import AppKit
import SwiftTerm
import SwiftUI

public struct SwiftTermView: NSViewRepresentable {
    let initialCommand: String?

    public init(initialCommand: String?) {
        self.initialCommand = initialCommand
    }

    public func makeNSView(context: Context) -> LocalProcessTerminalView {
        let tv = LocalProcessTerminalView(frame: .zero)

        // Metal rendering is automatic in LocalProcessTerminalView
        // Apply system colors
        tv.nativeBackgroundColor = .textBackgroundColor
        tv.nativeForegroundColor = .textColor

        // Start the shell
        let shell = ProcessInfo.processInfo.environment["SHELL"] ?? "/bin/zsh"
        tv.startProcess(executable: shell, args: [], environment: nil, execName: nil)

        // Send initial command after a brief yield
        if let cmd = initialCommand {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
                tv.send(txt: cmd + "\n")
            }
        }
        return tv
    }

    public func updateNSView(_ nsView: LocalProcessTerminalView, context: Context) {}
}
