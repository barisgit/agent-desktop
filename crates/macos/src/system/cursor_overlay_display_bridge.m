#import <AppKit/AppKit.h>
#import <CoreGraphics/CoreGraphics.h>
#import <stdbool.h>

static uint32_t ADTargetPid = 0;
static uint32_t ADTargetWindow = 0;
static CGPoint ADTargetPoint;
static CGRect ADTargetBounds;
static bool ADTargetBoundsKnown = false;
static CGPoint ADVisibleTargetPoint;

static bool ADWindowBoundsInWindows(NSArray *windows, uint32_t pid, uint32_t number,
                                    CGRect *output) {
    for (NSDictionary *window in windows) {
        if ([window[(id)kCGWindowOwnerPID] unsignedIntValue] != pid
            || [window[(id)kCGWindowNumber] unsignedIntValue] != number) {
            continue;
        }
        return CGRectMakeWithDictionaryRepresentation(
            (__bridge CFDictionaryRef)window[(id)kCGWindowBounds], output);
    }
    return false;
}

static CGPoint ADTranslatedTargetPoint(CGPoint point, CGRect from, CGRect to) {
    return CGPointMake(point.x + to.origin.x - from.origin.x,
                       point.y + to.origin.y - from.origin.y);
}

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
    ADVisibleTargetPoint = ADTargetPoint;
    ADTargetBoundsKnown = false;
    if (pid == 0 || window == 0) {
        return;
    }
    NSArray *windows = CFBridgingRelease(CGWindowListCopyWindowInfo(
        kCGWindowListOptionAll | kCGWindowListExcludeDesktopElements, kCGNullWindowID));
    ADTargetBoundsKnown = ADWindowBoundsInWindows(windows, pid, window, &ADTargetBounds);
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
        CGRect currentBounds;
        if (!ADWindowBoundsInWindows(windows, ADTargetPid, ADTargetWindow, &currentBounds)) {
            return false;
        }
        ADVisibleTargetPoint = ADTargetBoundsKnown
            ? ADTranslatedTargetPoint(ADTargetPoint, ADTargetBounds, currentBounds)
            : ADTargetPoint;
        NSRunningApplication *renderer = NSRunningApplication.currentApplication;
        return ADTargetVisibleInWindows(windows, ADTargetPid, ADTargetWindow, ADVisibleTargetPoint,
            ^bool(uint32_t owner) {
                if (owner == (uint32_t)renderer.processIdentifier) {
                    return true;
                }
                NSRunningApplication *peer = [NSRunningApplication runningApplicationWithProcessIdentifier:(pid_t)owner];
                return renderer.executableURL != nil && [renderer.executableURL isEqual:peer.executableURL];
            });
    }
}

bool agent_desktop_cursor_overlay_target_point(double *output) {
    if (output == NULL) {
        return false;
    }
    output[0] = ADVisibleTargetPoint.x;
    output[1] = ADVisibleTargetPoint.y;
    return true;
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

static CGPoint ADLabelPositionInFrame(double x, double y, double width, double height,
                                          NSRect frame) {
    double right = NSMaxX(frame);
    double bottom = NSMaxY(frame);
    double placedX = x + 18.0 + width <= right ? x + 18.0 : x - width - 18.0;
    double placedY = y + 18.0 + height <= bottom ? y + 18.0 : y - height - 18.0;
    return CGPointMake(MAX(frame.origin.x, MIN(placedX, MAX(right - width, frame.origin.x))),
                       MAX(frame.origin.y, MIN(placedY, MAX(bottom - height, frame.origin.y))));
}

bool agent_desktop_cursor_overlay_label_position(double x, double y, double width,
                                                 double height, double *output) {
    if (output == NULL) {
        return false;
    }
    NSScreen *screen = ADScreenAt(x, y);
    if (screen == nil) {
        return false;
    }
    CGPoint position = ADLabelPositionInFrame(x, y, width, height,
                                              ADTopLeftRect(screen.visibleFrame));
    output[0] = position.x;
    output[1] = position.y;
    return true;
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
