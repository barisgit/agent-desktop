import Foundation

struct OverlayArgs {
    let socketPath: String?
    let useWallpaperColor: Bool

    static func parse(_ argv: [String]) -> OverlayArgs {
        var socket: String? = ProcessInfo.processInfo.environment["AGENT_CURSOR_SOCKET"]
        var wallpaper = true
        if let env = ProcessInfo.processInfo.environment["AGENT_CURSOR_WALLPAPER_COLOR"] {
            wallpaper = !(env == "0" || env.lowercased() == "false")
        }
        var i = 1
        while i < argv.count {
            let a = argv[i]
            if a == "--socket", i + 1 < argv.count {
                socket = argv[i + 1]
                i += 2
                continue
            }
            if a.hasPrefix("--socket=") {
                socket = String(a.dropFirst("--socket=".count))
                i += 1
                continue
            }
            if a == "--no-wallpaper-color" {
                wallpaper = false
                i += 1
                continue
            }
            i += 1
        }
        return OverlayArgs(socketPath: socket, useWallpaperColor: wallpaper)
    }
}
