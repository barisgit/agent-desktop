import AppKit
import SwiftUI

private let cursorBaseSize: CGFloat = 22
private let wiggleIdleThresholdSeconds: TimeInterval = IdleTimings.holdSeconds

struct BlueCursorView: View {
    let screenFrame: NSRect
    @EnvironmentObject var state: CursorState

    var body: some View {
        TimelineView(.animation(minimumInterval: 1.0 / 30.0)) { ctx in
            content(now: ctx.date)
        }
        .ignoresSafeArea()
    }

    @ViewBuilder
    private func content(now: Date) -> some View {
        GeometryReader { _ in
            ZStack {
                Color.clear
                TargetBoxView(screenFrame: screenFrame)

                let opacity = effectiveOpacity(now: now)
                let sat = state.thinking ? 1.0 : state.colorSaturation(at: now)
                let tinted = desaturatedColor(state.color, saturation: sat)
                if state.shouldRender() && opacity > 0.01 {
                    let target = virtualCursorToLocal(state.virtualCursor)
                    let wiggle = wiggleOffset(now: state.wiggleSeed,
                                              thinking: state.thinking,
                                              idle: isIdle(now: now))
                    let pos = CGPoint(x: target.x + wiggle.width, y: target.y + wiggle.height)

                    ForEach(state.ripples) { ripple in
                        RippleView(ripple: ripple, color: state.color, screenFrame: screenFrame)
                            .opacity(opacity)
                    }

                    cursorBodyView(tint: tinted)
                        .opacity(opacity)
                        .position(x: pos.x, y: pos.y)
                        .animation(.spring(response: 0.32, dampingFraction: 0.72), value: target)
                }

                TypingBubble(screenFrame: screenFrame)
                ScrollArrowView(screenFrame: screenFrame)
                ErrorFlashView(screenFrame: screenFrame)
            }
        }
    }

    private func cursorBodyView(tint: Color) -> some View {
        let pulseScale = state.thinking ? thinkingPulse(now: state.wiggleSeed) : 1.0
        return ZStack {
            ArrowCursorShape()
                .fill(tint.opacity(0.22))
                .frame(width: cursorBaseSize * 1.7, height: cursorBaseSize * 1.7)
                .scaleEffect(pulseScale)
                .blur(radius: 5)
                .offset(x: cursorBaseSize * 0.35, y: cursorBaseSize * 0.35)
            ArrowCursorShape()
                .fill(tint)
                .overlay(
                    ArrowCursorShape()
                        .stroke(Color.white.opacity(0.92), lineWidth: 1.5)
                )
                .frame(width: cursorBaseSize, height: cursorBaseSize)
                .shadow(color: tint.opacity(0.55), radius: 8)
                .offset(x: cursorBaseSize * 0.35, y: cursorBaseSize * 0.35)
        }
    }

    private func desaturatedColor(_ c: Color, saturation: Double) -> Color {
        let s = max(0.0, min(1.0, saturation))
        if s >= 0.999 { return c }
        let ns = NSColor(c).usingColorSpace(.deviceRGB) ?? .gray
        let r = Double(ns.redComponent)
        let g = Double(ns.greenComponent)
        let b = Double(ns.blueComponent)
        let luminance = 0.299 * r + 0.587 * g + 0.114 * b
        let mr = r * s + luminance * (1.0 - s)
        let mg = g * s + luminance * (1.0 - s)
        let mb = b * s + luminance * (1.0 - s)
        return Color(red: mr, green: mg, blue: mb)
    }

    private func effectiveOpacity(now: Date) -> Double {
        guard state.hasVirtualCursor else { return 0.0 }
        if state.thinking { return 1.0 }
        return state.dimFactor(at: now)
    }

    private func isIdle(now: Date) -> Bool {
        now.timeIntervalSince(state.lastMoveAt) > wiggleIdleThresholdSeconds
    }

    private func wiggleOffset(now: Double, thinking: Bool, idle: Bool) -> CGSize {
        guard thinking else { return .zero }
        let amp: Double = 3.0
        let freq: Double = 2.4
        let dx = sin(now * 2.0 * .pi * freq) * amp
        let dy = cos(now * 2.0 * .pi * freq * 0.7) * amp
        return CGSize(width: dx, height: dy)
    }

    private func thinkingPulse(now: Double) -> CGFloat {
        let t = sin(now * 2.0 * .pi * 1.2)
        return 1.0 + CGFloat(t) * 0.18
    }

    private func virtualCursorToLocal(_ cgGlobal: CGPoint) -> CGPoint {
        let localX = cgGlobal.x - screenFrame.origin.x
        let localY = cgGlobal.y - screenFrame.origin.y
        return CGPoint(x: localX, y: localY)
    }
}

/// Classic macOS-style arrow cursor (hotspot at top-left tip).
/// Path designed for a unit square; SwiftUI scales it into the .frame.
struct ArrowCursorShape: Shape {
    func path(in rect: CGRect) -> Path {
        var p = Path()
        let w = rect.width
        let h = rect.height
        p.move(to: CGPoint(x: 0.02 * w, y: 0.02 * h))
        p.addLine(to: CGPoint(x: 0.02 * w, y: 0.80 * h))
        p.addLine(to: CGPoint(x: 0.24 * w, y: 0.62 * h))
        p.addLine(to: CGPoint(x: 0.38 * w, y: 0.94 * h))
        p.addLine(to: CGPoint(x: 0.50 * w, y: 0.88 * h))
        p.addLine(to: CGPoint(x: 0.36 * w, y: 0.56 * h))
        p.addLine(to: CGPoint(x: 0.62 * w, y: 0.56 * h))
        p.closeSubpath()
        return p
    }
}

struct RippleView: View {
    let ripple: ClickRipple
    let color: Color
    let screenFrame: NSRect

    @State private var outerScale: CGFloat = 0.25
    @State private var outerOpacity: Double = 0.95
    @State private var innerScale: CGFloat = 0.1
    @State private var innerOpacity: Double = 0.7

    var body: some View {
        let local = CGPoint(
            x: ripple.point.x - screenFrame.origin.x,
            y: ripple.point.y - screenFrame.origin.y
        )
        ZStack {
            Circle()
                .stroke(color, lineWidth: 2.5)
                .frame(width: 56, height: 56)
                .scaleEffect(outerScale)
                .opacity(outerOpacity)
            Circle()
                .fill(color.opacity(0.4))
                .frame(width: 28, height: 28)
                .scaleEffect(innerScale)
                .opacity(innerOpacity)
        }
        .position(x: local.x, y: local.y)
        .onAppear {
            withAnimation(.easeOut(duration: 0.55)) {
                outerScale = 2.1
                outerOpacity = 0.0
            }
            withAnimation(.easeOut(duration: 0.35)) {
                innerScale = 1.4
                innerOpacity = 0.0
            }
        }
    }
}
