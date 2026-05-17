import AppKit

let args = OverlayArgs.parse(CommandLine.arguments)

let app = NSApplication.shared
app.setActivationPolicy(.accessory)

let delegate = OverlayAppDelegate(args: args)
app.delegate = delegate
app.run()
