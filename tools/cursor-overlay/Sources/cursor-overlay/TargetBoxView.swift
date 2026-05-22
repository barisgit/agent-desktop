import AppKit
import SwiftUI

struct TargetBoxView: View {
    let screenFrame: NSRect
    @EnvironmentObject var state: CursorState

    var body: some View {
        Group {
            if let box = state.targetBox {
                Rectangle()
                    .stroke(state.color, lineWidth: 2)
                    .frame(width: box.width, height: box.height)
                    .position(localCenter(of: box))
            }
        }
    }

    private func localCenter(of box: CGRect) -> CGPoint {
        CGPoint(
            x: box.midX - screenFrame.origin.x,
            y: box.midY - screenFrame.origin.y
        )
    }
}
