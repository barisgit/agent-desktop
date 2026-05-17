import AppKit
import CoreImage
import SwiftUI

enum WallpaperColor {
    /// Samples the desktop wallpaper of the main screen and returns a complementary cursor color.
    /// Falls back to the default Codex blue on any error.
    static func deriveCursorColor() -> Color? {
        guard let screen = NSScreen.main,
              let url = NSWorkspace.shared.desktopImageURL(for: screen),
              let nsimg = NSImage(contentsOf: url),
              let cg = nsimg.cgImage(forProposedRect: nil, context: nil, hints: nil)
        else {
            return nil
        }
        let ci = CIImage(cgImage: cg)
        let extent = ci.extent
        let filter = CIFilter(name: "CIAreaAverage")
        filter?.setValue(ci, forKey: kCIInputImageKey)
        filter?.setValue(CIVector(cgRect: extent), forKey: kCIInputExtentKey)
        guard let output = filter?.outputImage else { return nil }
        var pixel = [UInt8](repeating: 0, count: 4)
        let ctx = CIContext(options: [.workingColorSpace: NSNull()])
        ctx.render(
            output,
            toBitmap: &pixel,
            rowBytes: 4,
            bounds: CGRect(x: 0, y: 0, width: 1, height: 1),
            format: .RGBA8,
            colorSpace: CGColorSpace(name: CGColorSpace.sRGB)
        )
        let r = Double(pixel[0]) / 255.0
        let g = Double(pixel[1]) / 255.0
        let b = Double(pixel[2]) / 255.0
        return complement(r: r, g: g, b: b)
    }

    /// Builds a vivid, eye-catching color that visually contrasts with the wallpaper.
    /// Strategy: rotate hue 180°, push saturation up, clamp brightness to a readable band.
    private static func complement(r: Double, g: Double, b: Double) -> Color {
        let (h, s, v) = rgbToHsv(r: r, g: g, b: b)
        let rotatedHue = (h + 0.5).truncatingRemainder(dividingBy: 1.0)
        let boostedSat = min(1.0, max(0.65, s + 0.3))
        let bandedVal = min(0.98, max(0.78, v + 0.25))
        let (rr, gg, bb) = hsvToRgb(h: rotatedHue, s: boostedSat, v: bandedVal)
        return Color(red: rr, green: gg, blue: bb)
    }

    private static func rgbToHsv(r: Double, g: Double, b: Double) -> (Double, Double, Double) {
        let maxC = max(r, g, b)
        let minC = min(r, g, b)
        let delta = maxC - minC
        var h = 0.0
        if delta != 0 {
            if maxC == r {
                h = ((g - b) / delta).truncatingRemainder(dividingBy: 6.0)
            } else if maxC == g {
                h = (b - r) / delta + 2.0
            } else {
                h = (r - g) / delta + 4.0
            }
            h /= 6.0
            if h < 0 { h += 1.0 }
        }
        let s = maxC == 0 ? 0 : delta / maxC
        return (h, s, maxC)
    }

    private static func hsvToRgb(h: Double, s: Double, v: Double) -> (Double, Double, Double) {
        let i = floor(h * 6.0)
        let f = h * 6.0 - i
        let p = v * (1.0 - s)
        let q = v * (1.0 - f * s)
        let t = v * (1.0 - (1.0 - f) * s)
        switch Int(i) % 6 {
        case 0: return (v, t, p)
        case 1: return (q, v, p)
        case 2: return (p, v, t)
        case 3: return (p, q, v)
        case 4: return (t, p, v)
        default: return (v, p, q)
        }
    }
}
