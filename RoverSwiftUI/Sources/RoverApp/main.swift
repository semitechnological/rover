import AppKit

// Entry point — no @main so we can use NSApplicationMain with a delegate.
let app = NSApplication.shared
let delegate = AppDelegate()
app.delegate = delegate
app.setActivationPolicy(.accessory)  // No Dock icon (LSUIElement equivalent).
app.run()
