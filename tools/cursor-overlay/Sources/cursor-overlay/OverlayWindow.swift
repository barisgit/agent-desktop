import AppKit
import SwiftUI

final class OverlayWindow: NSPanel {
    let screenFrame: NSRect

    init(screen: NSScreen, state: CursorState) {
        self.screenFrame = screen.frame
        super.init(
            contentRect: screen.frame,
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )

        self.isOpaque = false
        self.backgroundColor = .clear
        self.level = .screenSaver
        self.ignoresMouseEvents = true
        self.collectionBehavior = [.canJoinAllSpaces, .stationary, .fullScreenAuxiliary]
        self.isReleasedWhenClosed = false
        self.hasShadow = false
        self.hidesOnDeactivate = false
        self.isMovable = false
        self.isFloatingPanel = true
        self.becomesKeyOnlyIfNeeded = true

        let host = NSHostingView(rootView: BlueCursorView(screenFrame: screen.frame).environmentObject(state))
        host.frame = NSRect(origin: .zero, size: screen.frame.size)
        host.autoresizingMask = [.width, .height]
        self.contentView = host

        self.setFrame(screen.frame, display: true)
        self.setFrameOrigin(screen.frame.origin)
    }

    override var canBecomeKey: Bool { false }
    override var canBecomeMain: Bool { false }
}
