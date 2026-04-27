import AppKit
import SwiftTerm
import SwiftUI

struct SwiftTermView: NSViewRepresentable {
    let initialCommand: String?

    func makeNSView(context: Context) -> LocalProcessTerminalView {
        let tv = LocalProcessTerminalView(frame: .zero)

        // Metal rendering is automatic in LocalProcessTerminalView
        // Apply system colors
        let bg = NSColor.textBackgroundColor
        let fg = NSColor.textColor
        tv.terminal.backgroundColor = bg
        tv.terminal.foregroundColor = fg

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

    func updateNSView(_ nsView: LocalProcessTerminalView, context: Context) {}
}
