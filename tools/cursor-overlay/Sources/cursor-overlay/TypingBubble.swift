import AppKit
import SwiftUI

struct TypingBubble: View {
    let screenFrame: NSRect
    @EnvironmentObject var state: CursorState

    var body: some View {
        Group {
            if let text = state.typingBubble {
                Text(displayText(text))
                    .font(.system(size: 14, weight: .semibold, design: .rounded))
                    .foregroundStyle(.white)
                    .lineLimit(1)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 7)
                    .background(
                        RoundedRectangle(cornerRadius: 13, style: .continuous)
                            .fill(state.color.opacity(0.88))
                            .shadow(color: state.color.opacity(0.35), radius: 8)
                    )
                    .position(bubblePosition())
                    .transition(.opacity)
            }
        }
        .animation(.easeOut(duration: 0.4), value: state.typingBubble)
    }

    private func displayText(_ text: String) -> String {
        guard text.count > 40 else { return text }
        let end = text.index(text.startIndex, offsetBy: 39)
        return String(text[..<end]) + "…"
    }

    private func bubblePosition() -> CGPoint {
        let point = state.hasVirtualCursor ? state.virtualCursor : state.systemMouse
        let local = CGPoint(
            x: point.x - screenFrame.origin.x,
            y: point.y - screenFrame.origin.y
        )
        return CGPoint(x: local.x + 58, y: local.y - 30)
    }
}
