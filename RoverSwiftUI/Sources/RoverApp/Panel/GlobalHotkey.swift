import AppKit
import Carbon
import Foundation

enum HotkeyPreset: String, CaseIterable, Identifiable {
    case commandSpace
    case optionSpace
    case controlSpace
    case commandOptionSpace

    var id: String { rawValue }

    var display: String {
        switch self {
        case .commandSpace: "⌘ Space"
        case .optionSpace: "⌥ Space"
        case .controlSpace: "⌃ Space"
        case .commandOptionSpace: "⌘⌥ Space"
        }
    }

    var carbonModifiers: UInt32 {
        switch self {
        case .commandSpace: UInt32(cmdKey)
        case .optionSpace: UInt32(optionKey)
        case .controlSpace: UInt32(controlKey)
        case .commandOptionSpace: UInt32(cmdKey | optionKey)
        }
    }
}

struct HotkeyConfiguration: Codable, Equatable {
    let keyCode: UInt32
    let modifiers: UInt32
    let display: String

    static let defaultHotkey = HotkeyConfiguration(keyCode: 49, modifiers: UInt32(cmdKey), display: "⌘ Space")
    static let defaultEmoji = HotkeyConfiguration(keyCode: 49, modifiers: UInt32(cmdKey | optionKey), display: "⌘⌥ Space")

    static func current() -> HotkeyConfiguration {
        let defaults = UserDefaults.standard
        if defaults.object(forKey: "rover.hotkey.keyCode") != nil {
            let keyCode = UInt32(defaults.integer(forKey: "rover.hotkey.keyCode"))
            let modifiers = UInt32(defaults.integer(forKey: "rover.hotkey.modifiers"))
            let display = defaults.string(forKey: "rover.hotkey.display") ?? Self.display(keyCode: keyCode, modifiers: modifiers)
            return HotkeyConfiguration(keyCode: keyCode, modifiers: modifiers, display: display)
        }
        if let preset = HotkeyPreset(rawValue: defaults.string(forKey: "rover.hotkeyPreset") ?? "") {
            return HotkeyConfiguration(keyCode: 49, modifiers: preset.carbonModifiers, display: preset.display)
        }
        return .defaultHotkey
    }

    func save() {
        let defaults = UserDefaults.standard
        defaults.set(Int(keyCode), forKey: "rover.hotkey.keyCode")
        defaults.set(Int(modifiers), forKey: "rover.hotkey.modifiers")
        defaults.set(display, forKey: "rover.hotkey.display")
    }

    static func from(event: NSEvent) -> HotkeyConfiguration? {
        let modifiers = carbonModifiers(from: event.modifierFlags)
        guard modifiers != 0 else { return nil }
        let keyCode = UInt32(event.keyCode)
        return HotkeyConfiguration(
            keyCode: keyCode,
            modifiers: modifiers,
            display: display(keyCode: keyCode, modifiers: modifiers, event: event)
        )
    }

    private static func carbonModifiers(from flags: NSEvent.ModifierFlags) -> UInt32 {
        var modifiers: UInt32 = 0
        if flags.contains(.command) { modifiers |= UInt32(cmdKey) }
        if flags.contains(.option) { modifiers |= UInt32(optionKey) }
        if flags.contains(.control) { modifiers |= UInt32(controlKey) }
        if flags.contains(.shift) { modifiers |= UInt32(shiftKey) }
        return modifiers
    }

    private static func display(keyCode: UInt32, modifiers: UInt32, event: NSEvent? = nil) -> String {
        var parts = ""
        if modifiers & UInt32(controlKey) != 0 { parts += "⌃" }
        if modifiers & UInt32(optionKey) != 0 { parts += "⌥" }
        if modifiers & UInt32(shiftKey) != 0 { parts += "⇧" }
        if modifiers & UInt32(cmdKey) != 0 { parts += "⌘" }
        return parts + keyName(keyCode: keyCode, event: event)
    }

    private static func keyName(keyCode: UInt32, event: NSEvent?) -> String {
        switch keyCode {
        case 36: "↩"
        case 48: "Tab"
        case 49: " Space"
        case 51: "Delete"
        case 53: "Esc"
        case 123: "←"
        case 124: "→"
        case 125: "↓"
        case 126: "↑"
        default:
            if let chars = event?.charactersIgnoringModifiers, !chars.isEmpty {
                chars.uppercased()
            } else {
                "Key \(keyCode)"
            }
        }
    }
}

/// Wraps Carbon's `RegisterEventHotKey` for a single global hotkey.
///
/// Default: Cmd+Space (keyCode 49, modifiers: cmdKey).
final class GlobalHotkey {
    private static let signature = OSType(0x524F5652)

    private var hotKeyRef: EventHotKeyRef?
    private var handlerRef: EventHandlerRef?
    private let id: UInt32
    private let action: @MainActor () -> Void
    private(set) var lastError: OSStatus = noErr

    /// keyCode: virtual key code (49 = Space). modifier: Carbon modifier mask.
    init(id: UInt32 = 1, keyCode: UInt32 = 49, modifiers: UInt32 = UInt32(cmdKey), action: @escaping @MainActor () -> Void) {
        self.id = id
        self.action = action
        installHandler()
        register(keyCode: keyCode, modifiers: modifiers)
    }

    deinit {
        if let ref = hotKeyRef {
            UnregisterEventHotKey(ref)
        }
        if let ref = handlerRef {
            RemoveEventHandler(ref)
        }
    }

    func update(keyCode: UInt32 = 49, modifiers: UInt32) {
        if let ref = hotKeyRef {
            UnregisterEventHotKey(ref)
            hotKeyRef = nil
        }
        register(keyCode: keyCode, modifiers: modifiers)
    }

    func update(_ configuration: HotkeyConfiguration) {
        update(keyCode: configuration.keyCode, modifiers: configuration.modifiers)
    }

    // MARK: - Private

    private func installHandler() {
        guard handlerRef == nil else { return }
        var spec = EventTypeSpec(eventClass: OSType(kEventClassKeyboard),
                                 eventKind: UInt32(kEventHotKeyPressed))

        let selfPtr = Unmanaged.passUnretained(self).toOpaque()

        lastError = InstallEventHandler(GetApplicationEventTarget(), { _, event, userData -> OSStatus in
            guard let ptr = userData else { return noErr }
            guard let event else { return noErr }
            let hotkey = Unmanaged<GlobalHotkey>.fromOpaque(ptr).takeUnretainedValue()
            var eventID = EventHotKeyID()
            let status = GetEventParameter(
                event,
                EventParamName(kEventParamDirectObject),
                EventParamType(typeEventHotKeyID),
                nil,
                MemoryLayout<EventHotKeyID>.size,
                nil,
                &eventID
            )
            guard status == noErr,
                  eventID.signature == GlobalHotkey.signature,
                  eventID.id == hotkey.id
            else { return noErr }
            Task { @MainActor in hotkey.action() }
            return noErr
        }, 1, &spec, selfPtr, &handlerRef)
    }

    private func register(keyCode: UInt32, modifiers: UInt32) {
        let hotkeyID = EventHotKeyID(signature: Self.signature, id: id)
        lastError = RegisterEventHotKey(keyCode, modifiers, hotkeyID, GetApplicationEventTarget(), 0, &hotKeyRef)
    }
}
