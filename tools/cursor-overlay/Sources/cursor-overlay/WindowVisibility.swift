import AppKit
import CoreGraphics

/// Determines whether a given target_pid has a visible window at a given screen point.
///
/// Uses CGWindowListCopyWindowInfo to walk on-screen windows front-to-back. The cursor
/// for target_pid is considered visible if the topmost normal-layer window covering
/// the point belongs to that pid.
///
/// Aerospace-friendly: windows on hidden workspaces are moved off-screen by Aerospace,
/// so they fail the bounds check naturally.
enum WindowVisibility {
    /// Returns true when the cursor at `point` (CG global top-left coords) should be
    /// rendered for `targetPid`. If `targetPid` is nil, always returns true.
    static func isVisible(targetPid: Int?, at point: CGPoint) -> Bool {
        guard let pid = targetPid else { return true }
        let frontPid = Int(NSWorkspace.shared.frontmostApplication?.processIdentifier ?? -1)
        if frontPid == pid { return true }
        return targetHasWindowOnOtherScreen(pid: pid, frontPid: frontPid, cursorPoint: point)
    }

    /// Returns true if target has an on-screen window AND that window does not share a screen
    /// with the frontmost app's primary window. This covers the multi-monitor case where the
    /// agent's target app is visible on screen B while the user works on screen A.
    private static func targetHasWindowOnOtherScreen(pid: Int, frontPid: Int, cursorPoint: CGPoint)
        -> Bool
    {
        let opts: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
        guard let raw = CGWindowListCopyWindowInfo(opts, kCGNullWindowID) as? [[String: Any]] else {
            return false
        }
        var targetRects: [CGRect] = []
        var frontRects: [CGRect] = []
        for info in raw {
            guard let layer = info[kCGWindowLayer as String] as? Int, layer == 0 else { continue }
            guard let boundsDict = info[kCGWindowBounds as String] as? [String: Any],
                  let rect = CGRect(dictionaryRepresentation: boundsDict as CFDictionary)
            else { continue }
            if rect.width < 1 || rect.height < 1 { continue }
            let owner = (info[kCGWindowOwnerPID as String] as? Int) ?? -1
            if owner == pid { targetRects.append(rect) }
            if owner == frontPid { frontRects.append(rect) }
        }
        guard let targetAt = targetRects.first(where: { $0.contains(cursorPoint) }) else {
            return false
        }
        for fr in frontRects where fr.intersects(targetAt) { return false }
        return true
    }
}
