import AppKit
import Combine
import SwiftUI

/// Idle-dim timeline. Each phase starts where the previous ends.
enum IdleTimings {
    /// Cursor stays at full color + full opacity for this long after the last move.
    static let holdSeconds: TimeInterval = 3.0
    /// After the hold, color desaturates toward gray over this duration. Opacity still 1.0.
    static let desaturateSeconds: TimeInterval = 3.0
    /// After desaturation finishes, opacity fades from 1.0 to 0.0 over this duration.
    static let fadeSeconds: TimeInterval = 4.0

    static let desaturateStart: TimeInterval = holdSeconds
    static let desaturateEnd: TimeInterval = holdSeconds + desaturateSeconds
    static let fadeStart: TimeInterval = desaturateEnd
    static let fadeEnd: TimeInterval = desaturateEnd + fadeSeconds
}

struct ClickRipple: Identifiable, Equatable {
    let id = UUID()
    let point: CGPoint
    let bornAt: Date
}

struct ScrollArrowState: Equatable {
    var point: CGPoint
    var dx: Double
    var dy: Double
}

struct ErrorFlashState: Equatable {
    var point: CGPoint?
    var code: String
    var message: String
}

final class CursorState: ObservableObject {
    @Published var systemMouse: NSPoint = NSEvent.mouseLocation
    @Published var virtualCursor: CGPoint = NSEvent.mouseLocation
    @Published var hasVirtualCursor: Bool = false
    @Published var visible: Bool = true
    @Published var color: Color = Color(red: 0.247, green: 0.541, blue: 0.988)
    @Published var ripples: [ClickRipple] = []
    @Published var lastMoveAt: Date = Date.distantPast
    @Published var thinking: Bool = false
    @Published var wiggleSeed: Double = 0.0
    @Published var targetPid: Int? = nil
    @Published var targetVisible: Bool = true
    @Published var typingBubble: String? = nil
    @Published var scrollArrow: ScrollArrowState? = nil
    @Published var targetBox: CGRect? = nil
    @Published var errorFlash: ErrorFlashState? = nil

    func applyMove(_ p: CGPoint, targetPid: Int?) {
        virtualCursor = p
        hasVirtualCursor = true
        lastMoveAt = Date()
        self.targetPid = targetPid
    }

    func applyClick(_ p: CGPoint, targetPid: Int?) {
        virtualCursor = p
        hasVirtualCursor = true
        lastMoveAt = Date()
        self.targetPid = targetPid
        let ripple = ClickRipple(point: p, bornAt: Date())
        ripples.append(ripple)
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.6) { [weak self] in
            self?.ripples.removeAll { $0.id == ripple.id }
        }
    }

    func setVisible(_ v: Bool) {
        visible = v
    }

    func setColor(r: Double, g: Double, b: Double) {
        let scale = max(r, g, b) > 1.0 ? 255.0 : 1.0
        color = Color(red: r / scale, green: g / scale, blue: b / scale)
    }

    func setThinking(_ v: Bool) {
        thinking = v
    }

    func setTypingBubble(_ text: String?) {
        typingBubble = text
    }

    func clearTypingBubble() {
        typingBubble = nil
    }

    func setScrollArrow(point: CGPoint, dx: Double, dy: Double, targetPid: Int?) {
        scrollArrow = ScrollArrowState(point: point, dx: dx, dy: dy)
        virtualCursor = point
        hasVirtualCursor = true
        lastMoveAt = Date()
        self.targetPid = targetPid
    }

    func clearScrollArrow() {
        scrollArrow = nil
    }

    func setTargetBox(_ bounds: CGRect, targetPid: Int?) {
        targetBox = bounds
        self.targetPid = targetPid
    }

    func clearTargetBox() {
        targetBox = nil
    }

    func setErrorFlash(point: CGPoint?, code: String, message: String) {
        errorFlash = ErrorFlashState(point: point, code: code, message: message)
    }

    func clearErrorFlash() {
        errorFlash = nil
    }

    func tickWiggle(t: Double) {
        wiggleSeed = t
    }

    func setTargetVisible(_ v: Bool) {
        targetVisible = v
    }

    /// Cursor is rendered when:
    /// - we have a virtual position, AND visible flag is on, AND
    /// - either the event was broadcast (targetPid == nil)
    ///   or the target's window is on-screen and uncovered at the cursor point.
    func shouldRender() -> Bool {
        guard hasVirtualCursor, visible else { return false }
        guard targetPid != nil else { return true }
        return targetVisible
    }

    /// Two-phase idle dim driven by `IdleTimings`. Color desaturates first, then opacity fades.
    func dimFactor(at now: Date = Date()) -> Double {
        let dt = now.timeIntervalSince(lastMoveAt)
        if dt < IdleTimings.fadeStart { return 1.0 }
        if dt < IdleTimings.fadeEnd {
            let t = (dt - IdleTimings.fadeStart) / IdleTimings.fadeSeconds
            return 1.0 - t
        }
        return 0.0
    }

    /// Color saturation factor 0..1. 1 = full color, 0 = fully desaturated (gray).
    func colorSaturation(at now: Date = Date()) -> Double {
        let dt = now.timeIntervalSince(lastMoveAt)
        if dt < IdleTimings.desaturateStart { return 1.0 }
        if dt < IdleTimings.desaturateEnd {
            let t = (dt - IdleTimings.desaturateStart) / IdleTimings.desaturateSeconds
            return 1.0 - t
        }
        return 0.0
    }

    func isIdle(at now: Date = Date()) -> Bool {
        now.timeIntervalSince(lastMoveAt) > IdleTimings.holdSeconds
    }
}
