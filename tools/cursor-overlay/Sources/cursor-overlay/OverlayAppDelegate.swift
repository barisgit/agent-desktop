import AppKit

final class OverlayAppDelegate: NSObject, NSApplicationDelegate {
    let args: OverlayArgs
    let cursorState = CursorState()
    private var windows: [OverlayWindow] = []
    private var screenChangeObserver: NSObjectProtocol?
    private var activateObserver: NSObjectProtocol?
    private var deactivateObserver: NSObjectProtocol?
    private var hideObserver: NSObjectProtocol?
    private var socketServer: SocketServer?

    init(args: OverlayArgs) {
        self.args = args
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        applyWallpaperColor()
        rebuildWindows()
        screenChangeObserver = NotificationCenter.default.addObserver(
            forName: NSApplication.didChangeScreenParametersNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            self?.applyWallpaperColor()
            self?.rebuildWindows()
        }
        let nc = NSWorkspace.shared.notificationCenter
        activateObserver = nc.addObserver(
            forName: NSWorkspace.didActivateApplicationNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            self?.refreshVisibility()
        }
        deactivateObserver = nc.addObserver(
            forName: NSWorkspace.didDeactivateApplicationNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            self?.refreshVisibility()
        }
        hideObserver = nc.addObserver(
            forName: NSWorkspace.didHideApplicationNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            self?.refreshVisibility()
        }
        startMouseTracker()
        startWiggleTicker()
        startVisibilityTicker()
        startSocketIfRequested()
        fputs("cursor-overlay: ready; socket=\(args.socketPath ?? "<none>")\n", stderr)
    }

    func applicationWillTerminate(_ notification: Notification) {
        if let obs = screenChangeObserver {
            NotificationCenter.default.removeObserver(obs)
        }
        let nc = NSWorkspace.shared.notificationCenter
        if let obs = activateObserver { nc.removeObserver(obs) }
        if let obs = deactivateObserver { nc.removeObserver(obs) }
        if let obs = hideObserver { nc.removeObserver(obs) }
        socketServer?.stop()
    }

    private func rebuildWindows() {
        for w in windows { w.orderOut(nil) }
        windows.removeAll()
        for screen in NSScreen.screens {
            let win = OverlayWindow(screen: screen, state: cursorState)
            win.orderFrontRegardless()
            windows.append(win)
        }
    }

    private func startMouseTracker() {
        Timer.scheduledTimer(withTimeInterval: 1.0 / 60.0, repeats: true) { [weak self] _ in
            let loc = NSEvent.mouseLocation
            DispatchQueue.main.async {
                self?.cursorState.systemMouse = loc
            }
        }
    }

    private func startWiggleTicker() {
        let start = Date()
        Timer.scheduledTimer(withTimeInterval: 1.0 / 30.0, repeats: true) { [weak self] _ in
            let t = Date().timeIntervalSince(start)
            DispatchQueue.main.async {
                self?.cursorState.tickWiggle(t: t)
            }
        }
    }

    private func startVisibilityTicker() {
        Timer.scheduledTimer(withTimeInterval: 0.08, repeats: true) { [weak self] _ in
            self?.refreshVisibility()
        }
    }

    private func refreshVisibility() {
        let pid = cursorState.targetPid
        let point = cursorState.virtualCursor
        let visible = WindowVisibility.isVisible(targetPid: pid, at: point)
        cursorState.setTargetVisible(visible)
    }

    private func applyWallpaperColor() {
        guard args.useWallpaperColor, let c = WallpaperColor.deriveCursorColor() else { return }
        cursorState.color = c
    }

    private func startSocketIfRequested() {
        guard let path = args.socketPath else {
            fputs("cursor-overlay: no socket configured (set --socket PATH or AGENT_CURSOR_SOCKET)\n", stderr)
            return
        }
        let server = SocketServer(path: path) { [weak self] msg in
            DispatchQueue.main.async {
                self?.handleMessage(msg)
            }
        }
        do {
            try server.start()
            socketServer = server
        } catch {
            fputs("cursor-overlay: socket start failed: \(error)\n", stderr)
        }
    }

    private func handleMessage(_ msg: OverlayMessage) {
        switch msg {
        case .move(let x, let y, let pid):
            let p = CGPoint(x: x, y: y)
            cursorState.applyMove(p, targetPid: pid)
            refreshVisibilityNow(pid: pid, point: p)
        case .click(let x, let y, _, _, let pid):
            let p = CGPoint(x: x, y: y)
            cursorState.applyClick(p, targetPid: pid)
            refreshVisibilityNow(pid: pid, point: p)
        case .scroll(let x, let y, let dx, let dy, let pid):
            let p = CGPoint(x: x, y: y)
            cursorState.setScrollIndicator(point: p, dx: dx, dy: dy, targetPid: pid)
            refreshVisibilityNow(pid: pid, point: p)
        case .key(let text, let combo):
            cursorState.setTypingText(text ?? combo)
        case .targetSet(let x, let y, let w, let h, let pid):
            cursorState.setTargetBounds(CGRect(x: x, y: y, width: w, height: h), targetPid: pid)
        case .targetClear:
            cursorState.clearTargetBounds()
        case .error(let x, let y, let code, let message):
            let point = x.flatMap { xValue in y.map { CGPoint(x: xValue, y: $0) } }
            cursorState.setErrorFlash(point: point, code: code, message: message)
        case .thinking(let v):
            cursorState.setThinking(v)
        case .setVisible(let v):
            cursorState.setVisible(v)
        case .setColor(let r, let g, let b):
            cursorState.setColor(r: r, g: g, b: b)
        case .bye:
            NSApp.terminate(nil)
        }
    }

    private func refreshVisibilityNow(pid: Int?, point: CGPoint) {
        let visible = WindowVisibility.isVisible(targetPid: pid, at: point)
        cursorState.setTargetVisible(visible)
    }
}
