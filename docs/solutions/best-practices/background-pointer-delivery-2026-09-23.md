---
title: Background pointer delivery posts to one window's process, not the screen
date: 2026-09-23
category: best-practices
module: core background_pointer and macos input/mouse_background
problem_type: architecture_pattern
component: tooling
severity: medium
applies_when:
  - "Hovering or clicking inside a window that is hidden, offscreen, or behind other windows"
  - "Revealing hover-only controls (for example VS Code Explorer header actions) without taking the user's cursor"
  - "Debugging a --background command that returned ok but had no visible effect"
tags: [background-pointer, cgeventposttopid, electron, hover, delivery-semantics, macos]
---

# Background pointer delivery posts to one window's process, not the screen

## Context

Some controls exist in the accessibility tree only while hovered. VS Code's
Explorer header (New File, New Folder, ...) is the motivating case: with the
window on a hidden AeroSpace workspace, the buttons never appear, and headed
hover would move the user's cursor and raise the window.

`--background` on `hover`, `mouse-move`, and `mouse-click` posts the event
with `CGEventPostToPid` to the process that owns one exact window. It never
uses the HID tap, `CGWarpMouseCursorPosition`, or activation APIs.

## Guidance

- **Always target an exact window.** A ref supplies pid, process instance, and
  `source_window_id`; raw `--xy` must pass `--window-id`. Core re-verifies the
  window with `resolve_window_strict` under the interaction lease right before
  posting (title left empty because titles change).
- **Tag the event with the window.** A pid-targeted event has no window-server
  hit test, so AppKit routes it by `kCGMouseEventWindowUnderMousePointer` (91)
  and `kCGMouseEventWindowUnderMousePointerThatCanHandleThisEvent` (92), both
  set to the CG window number. Locations stay in global CG coordinates. An
  earlier prototype (c803d040) wrote fields 28/29 and saw events ignored by
  Calculator, TextEdit, System Settings, and YT Music; those are the wrong
  fields.
- **Check geometry against the window only.** The point must lie inside the
  window bounds. Offscreen and covered windows are valid targets; there is no
  occlusion or on-screen requirement.
- **Never claim silent success.** Success is `delivered_unverified`
  (`retry: unsafe`). The adapter samples the frontmost app (live
  `AXFocusedApplication`) before posting and ~100 ms after, and the result
  reports `focus_change: unchanged | changed | unknown` plus a warning on
  change. Hover also reports `unsafe`: a repeat is harmless, but the contract
  has no delivered-and-safe state, and the right follow-up is a snapshot.
- **Skip the cursor overlay.** It would draw a cursor where the real one is
  not, over a window that may be invisible.

## Known limits

- **Chromium/Electron hover.** `RenderWidgetHostViewCocoa`'s
  `shouldIgnoreMouseEvent:` drops `mouseMoved` unless the window is the app's
  main or key window (default `kWhenInActiveWindow`), and hit-tests the content
  view at the event location. Tracking areas are `NSTrackingActiveAlways`, so
  an inactive app still receives moves. Background hover therefore works only
  while the target window is still its app's main window. If not, VS Code's
  `workbench.view.alwaysShowHeaderActions` setting shows the header actions
  without hover.
- **Chromium/Electron clicks.** `acceptsFirstMouse:` is NO by default, so a
  first click into an inactive window may be swallowed unless the app enables
  `acceptFirstMouse`. Prefer a semantic `click @ref` (AXPress) once hover has
  revealed the control.
- **Sandboxed or hardened apps** (Mail, Notes, App Store) may drop
  pid-targeted events silently.
- **Keys are out of scope.** macOS delivers keystrokes to the key window;
  pid-posted keys hit the same wall.
- **Effects are not verified.** Observe with `snapshot` after delivery.

## Verification

Unit tests cover event construction (types, click state, fields 91/92,
modifiers), window derivation from refs, bounds rejection, disposition and
focus reporting, CLI/batch parsing, and dispatch routing. Live behavior on a
real backgrounded app must be checked by observation: snapshot before,
`hover --background`, snapshot after, and confirm the frontmost app and cursor
position are unchanged.
