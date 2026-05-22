import AppKit
import SwiftUI

struct ScrollArrowView: View {
    let screenFrame: NSRect
    @EnvironmentObject var state: CursorState

    var body: some View {
        Group {
            if let arrow = state.scrollArrow {
                Text(symbol(for: arrow))
                    .font(.system(size: 28, weight: .heavy, design: .rounded))
                    .foregroundStyle(.white)
                    .frame(width: 46, height: 46)
                    .background(
                        Circle()
                            .fill(state.color.opacity(0.86))
                            .shadow(color: state.color.opacity(0.45), radius: 10)
                    )
                    .overlay(
                        Circle()
                            .stroke(Color.white.opacity(0.82), lineWidth: 1.5)
                    )
                    .position(localPoint(arrow.point))
                    .transition(.opacity)
            }
        }
        .animation(.easeOut(duration: 0.6), value: state.scrollArrow)
    }

    private func symbol(for arrow: ScrollArrowState) -> String {
        if abs(arrow.dx) > abs(arrow.dy) {
            if arrow.dx > 0 { return "→" }
            if arrow.dx < 0 { return "←" }
        } else {
            if arrow.dy > 0 { return "↑" }
            if arrow.dy < 0 { return "↓" }
        }
        return "•"
    }

    private func localPoint(_ point: CGPoint) -> CGPoint {
        CGPoint(
            x: point.x - screenFrame.origin.x,
            y: point.y - screenFrame.origin.y
        )
    }
}
