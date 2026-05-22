import AppKit
import SwiftUI

struct ErrorFlashView: View {
    let screenFrame: NSRect
    @EnvironmentObject var state: CursorState

    var body: some View {
        Group {
            if let flash = state.errorFlash {
                ZStack {
                    Circle()
                        .fill(Color.red.opacity(0.22))
                        .frame(width: 118, height: 118)
                    Circle()
                        .stroke(Color.red.opacity(0.72), lineWidth: 3)
                        .frame(width: 76, height: 76)
                    Text(flash.code)
                        .font(.system(size: 13, weight: .bold, design: .monospaced))
                        .foregroundStyle(.white)
                        .lineLimit(1)
                        .padding(.horizontal, 10)
                        .padding(.vertical, 5)
                        .background(
                            Capsule()
                                .fill(Color.red.opacity(0.95))
                                .shadow(color: Color.red.opacity(0.45), radius: 8)
                        )
                        .offset(y: 52)
                }
                .position(position(for: flash))
                .transition(.opacity)
            }
        }
        .animation(.easeOut(duration: 0.8), value: state.errorFlash)
    }

    private func position(for flash: ErrorFlashState) -> CGPoint {
        guard let point = flash.point else {
            return CGPoint(x: screenFrame.width / 2.0, y: screenFrame.height / 2.0)
        }
        return CGPoint(
            x: point.x - screenFrame.origin.x,
            y: point.y - screenFrame.origin.y
        )
    }
}
