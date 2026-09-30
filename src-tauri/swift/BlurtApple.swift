// Blurt's bridge to Apple frameworks, exposed to Rust through a small C ABI.
//
// Microphone permission (AVFoundation): ask once, with Apple's own prompt.
// Apple Intelligence (FoundationModels): the on-device model answers "Ask AI" for free,
// privately, with no setup, on Macs that have Apple Intelligence turned on (macOS 26+).

import Foundation
import AVFoundation
#if canImport(FoundationModels)
import FoundationModels
#endif

private final class Box: @unchecked Sendable {
    var text = ""
    var ok: Int32 = 0
}

/// 1 when the on-device model is ready to use, 0 otherwise.
@_cdecl("blurt_ai_available")
public func blurt_ai_available() -> Int32 {
    #if canImport(FoundationModels)
    if #available(macOS 26.0, *) {
        if case .available = SystemLanguageModel.default.availability { return 1 }
    }
    #endif
    return 0
}

/// Why the on-device model can't be used, in plain words (empty when it can).
@_cdecl("blurt_ai_status")
public func blurt_ai_status() -> UnsafeMutablePointer<CChar>? {
    var reason = "This Mac needs macOS 26 or later for Apple Intelligence."
    #if canImport(FoundationModels)
    if #available(macOS 26.0, *) {
        switch SystemLanguageModel.default.availability {
        case .available:
            reason = ""
        case .unavailable(.appleIntelligenceNotEnabled):
            reason = "Turn on Apple Intelligence in System Settings to use it with Blurt."
        case .unavailable(.deviceNotEligible):
            reason = "This Mac doesn't support Apple Intelligence."
        case .unavailable(.modelNotReady):
            reason = "Apple Intelligence is still downloading. Try again in a few minutes."
        default:
            reason = "Apple Intelligence isn't available right now."
        }
    }
    #endif
    return strdup(reason)
}

/// Runs one prompt against the on-device model. Blocks the calling thread (Rust calls this
/// from a worker thread). `ok` is set to 1 on success; the returned string is the reply or
/// the error message. Free it with `blurt_free`.
@_cdecl("blurt_ai_complete")
public func blurt_ai_complete(
    _ system: UnsafePointer<CChar>,
    _ user: UnsafePointer<CChar>,
    _ ok: UnsafeMutablePointer<Int32>
) -> UnsafeMutablePointer<CChar>? {
    let instructions = String(cString: system)
    let prompt = String(cString: user)
    let box = Box()
    #if canImport(FoundationModels)
    if #available(macOS 26.0, *) {
        let done = DispatchSemaphore(value: 0)
        Task.detached {
            do {
                let session = LanguageModelSession(instructions: instructions)
                let response = try await session.respond(to: prompt)
                box.text = response.content
                box.ok = 1
            } catch let error as LanguageModelSession.GenerationError {
                switch error {
                case .exceededContextWindowSize:
                    box.text = "That's too much text for Apple Intelligence. Try a shorter selection."
                case .guardrailViolation, .refusal:
                    box.text = "Apple Intelligence declined that request."
                default:
                    box.text = "Apple Intelligence couldn't do that: \(error.localizedDescription)"
                }
            } catch {
                box.text = "Apple Intelligence couldn't do that: \(error.localizedDescription)"
            }
            done.signal()
        }
        done.wait()
    } else {
        box.text = "Apple Intelligence needs macOS 26 or later."
    }
    #else
    box.text = "This build of Blurt doesn't include Apple Intelligence."
    #endif
    ok.pointee = box.ok
    return strdup(box.text)
}

@_cdecl("blurt_free")
public func blurt_free(_ ptr: UnsafeMutablePointer<CChar>?) {
    free(ptr)
}

/// 0 = not asked yet, 1 = restricted, 2 = denied, 3 = allowed.
@_cdecl("blurt_mic_status")
public func blurt_mic_status() -> Int32 {
    switch AVCaptureDevice.authorizationStatus(for: .audio) {
    case .notDetermined: return 0
    case .restricted: return 1
    case .denied: return 2
    case .authorized: return 3
    @unknown default: return 0
    }
}

/// Shows the system microphone prompt (first time only) and waits for the answer.
@_cdecl("blurt_mic_request")
public func blurt_mic_request() -> Int32 {
    let done = DispatchSemaphore(value: 0)
    let box = Box()
    AVCaptureDevice.requestAccess(for: .audio) { granted in
        box.ok = granted ? 1 : 0
        done.signal()
    }
    done.wait()
    return box.ok
}
