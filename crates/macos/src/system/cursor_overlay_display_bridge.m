#import <AppKit/AppKit.h>
#import <CoreGraphics/CoreGraphics.h>
#import <stdbool.h>

static uint32_t ADTargetPid = 0;
static uint32_t ADTargetWindow = 0;
static CGPoint ADTargetPoint;

static bool ADTargetVisibleInWindows(NSArray *windows, uint32_t pid, uint32_t number,
                                     CGPoint point, bool (^isRenderer)(uint32_t)) {
    for (NSDictionary *window in windows) {
        uint32_t owner = [window[(id)kCGWindowOwnerPID] unsignedIntValue];
        if ([window[(id)kCGWindowAlpha] doubleValue] <= 0) {
            continue;
        }
        CGRect bounds;
        if (!CGRectMakeWithDictionaryRepresentation((__bridge CFDictionaryRef)window[(id)kCGWindowBounds], &bounds)
            || !CGRectContainsPoint(bounds, point)) {
            continue;
        }
        if (isRenderer(owner)) {
            continue;
        }
        return owner == pid && [window[(id)kCGWindowNumber] unsignedIntValue] == number;
    }
    return false;
}

void agent_desktop_cursor_overlay_target(uint32_t pid, uint32_t window, double x, double y) {
    ADTargetPid = pid;
    ADTargetWindow = window;
    ADTargetPoint = CGPointMake(x, y);
}

bool agent_desktop_cursor_overlay_target_visible(void) {
    if (ADTargetPid == 0) {
        return true;
    }
    @autoreleasepool {
        NSRunningApplication *app = [NSRunningApplication runningApplicationWithProcessIdentifier:(pid_t)ADTargetPid];
        if (app == nil || app.hidden || app.terminated || ADTargetWindow == 0) {
            return false;
        }
        NSArray *windows = CFBridgingRelease(CGWindowListCopyWindowInfo(
            kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements, kCGNullWindowID));
        NSRunningApplication *renderer = NSRunningApplication.currentApplication;
        return ADTargetVisibleInWindows(windows, ADTargetPid, ADTargetWindow, ADTargetPoint,
            ^bool(uint32_t owner) {
                if (owner == (uint32_t)renderer.processIdentifier) {
                    return true;
                }
                NSRunningApplication *peer = [NSRunningApplication runningApplicationWithProcessIdentifier:(pid_t)owner];
                return renderer.executableURL != nil && [renderer.executableURL isEqual:peer.executableURL];
            });
    }
}

static NSRect ADTopLeftRectAtHeight(NSRect frame, double mainHeight) {
    return NSMakeRect(frame.origin.x,
                      mainHeight - NSMaxY(frame),
                      frame.size.width,
                      frame.size.height);
}

static NSRect ADTopLeftRect(NSRect frame) {
    return ADTopLeftRectAtHeight(frame, CGDisplayBounds(CGMainDisplayID()).size.height);
}

static NSScreen *ADScreenAt(double x, double y) {
    NSPoint point = NSMakePoint(x, y);
    for (NSScreen *screen in NSScreen.screens) {
        if (NSPointInRect(point, ADTopLeftRect(screen.frame))) {
            return screen;
        }
    }
    return nil;
}

bool agent_desktop_cursor_overlay_initial_point(double *output) {
    if (output == NULL) {
        return false;
    }
    @autoreleasepool {
        NSScreen *screen = NSScreen.mainScreen;
        if (screen == nil) {
            return false;
        }
        NSRect frame = ADTopLeftRect(screen.visibleFrame);
        output[0] = NSMidX(frame);
        output[1] = NSMidY(frame);
        return true;
    }
}

bool agent_desktop_cursor_overlay_screen(double x,
                                         double y,
                                         double *output) {
    if (output == NULL) {
        return false;
    }
    @try {
        @autoreleasepool {
            NSScreen *screen = ADScreenAt(x, y);
            if (screen == nil) {
                return false;
            }
            NSRect frame = ADTopLeftRect(screen.visibleFrame);
            output[0] = frame.origin.x;
            output[1] = frame.origin.y;
            output[2] = frame.size.width;
            output[3] = frame.size.height;
            double refreshRate = 60.0;
            NSNumber *screenNumber = screen.deviceDescription[@"NSScreenNumber"];
            if (screenNumber != nil) {
                CGDisplayModeRef mode = CGDisplayCopyDisplayMode(screenNumber.unsignedIntValue);
                if (mode != NULL) {
                    double reportedRate = CGDisplayModeGetRefreshRate(mode);
                    if (reportedRate > 0.0) {
                        refreshRate = reportedRate;
                    }
                    CGDisplayModeRelease(mode);
                }
            }
            output[4] = MAX(60.0, MIN(120.0, refreshRate));
            output[5] = NSWorkspace.sharedWorkspace.accessibilityDisplayShouldReduceMotion ? 1.0 : 0.0;
            return true;
        }
    } @catch (NSException *exception) {
        (void)exception;
        return false;
    }
}
