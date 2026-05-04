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

/// Wraps Carbon's `RegisterEventHotKey` for a single global hotkey.
///
/// Default: Cmd+Space (keyCode 49, modifiers: cmdKey).
final class GlobalHotkey {
    private static let signature = OSType(0x524F5652)

    private var hotKeyRef: EventHotKeyRef?
    private var handlerRef: EventHandlerRef?
    private let id: UInt32
    private let action: @MainActor () -> Void

    /// keyCode: virtual key code (49 = Space). modifier: Carbon modifier mask.
    init(id: UInt32 = 1, keyCode: UInt32 = 49, modifiers: UInt32 = UInt32(cmdKey), action: @escaping @MainActor () -> Void) {
        self.id = id
        self.action = action
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

    // MARK: - Private

    private func register(keyCode: UInt32, modifiers: UInt32) {
        let hotkeyID = EventHotKeyID(signature: Self.signature, id: id)

        var spec = EventTypeSpec(eventClass: OSType(kEventClassKeyboard),
                                 eventKind: UInt32(kEventHotKeyPressed))

        let selfPtr = Unmanaged.passUnretained(self).toOpaque()

        InstallEventHandler(GetApplicationEventTarget(), { _, event, userData -> OSStatus in
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

        RegisterEventHotKey(keyCode, modifiers, hotkeyID, GetApplicationEventTarget(), 0, &hotKeyRef)
    }
}
