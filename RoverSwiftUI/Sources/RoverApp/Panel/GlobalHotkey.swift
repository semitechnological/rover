import Carbon
import Foundation

/// Wraps Carbon's `RegisterEventHotKey` for a single global hotkey.
///
/// Default: Cmd+Space (keyCode 49, modifiers: cmdKey).
final class GlobalHotkey {
    private var hotKeyRef: EventHotKeyRef?
    private var handlerRef: EventHandlerRef?
    private let action: @MainActor () -> Void

    /// keyCode: virtual key code (49 = Space). modifier: Carbon modifier mask.
    init(keyCode: UInt32 = 49, modifiers: UInt32 = UInt32(cmdKey), action: @escaping @MainActor () -> Void) {
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

    // MARK: - Private

    private func register(keyCode: UInt32, modifiers: UInt32) {
        let id = EventHotKeyID(signature: OSType(0x524F5652), id: 1) // "ROVR"

        var spec = EventTypeSpec(eventClass: OSType(kEventClassKeyboard),
                                 eventKind: UInt32(kEventHotKeyPressed))

        // Use a C-compatible closure via a retained pointer.
        let selfPtr = Unmanaged.passRetained(self).toOpaque()

        InstallEventHandler(GetApplicationEventTarget(), { _, event, userData -> OSStatus in
            guard let ptr = userData else { return noErr }
            let hotkey = Unmanaged<GlobalHotkey>.fromOpaque(ptr).takeUnretainedValue()
            Task { @MainActor in hotkey.action() }
            return noErr
        }, 1, &spec, selfPtr, &handlerRef)

        RegisterEventHotKey(keyCode, modifiers, id, GetApplicationEventTarget(), 0, &hotKeyRef)
    }
}
