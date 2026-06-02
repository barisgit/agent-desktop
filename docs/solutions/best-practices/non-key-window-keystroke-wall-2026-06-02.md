---
title: Background keystroke delivery is bounded by the key-window wall (macOS 26 Tahoe)
date: 2026-06-02
category: best-practices
module: crates/macos
problem_type: best_practice
component: macos-input
severity: high
applies_when:
  - An agent wants to deliver a committing keystroke (Return, Cmd+T, Cmd+L) to a backgrounded app without stealing focus
  - The target app has multiple windows and the target is NOT the app's current key window
  - Building or reasoning about the headless press / press --window-id path
tags:
  - macos
  - headless
  - keyboard
  - window-server
  - interaction-policy
  - key-window
---

# Background keystroke delivery is bounded by the key-window wall

## Context

Headless control (`--policy headless`) delivers synthesized keystrokes with
`CGEventPostToPid(pid, event)`. That call enqueues the event into the target
**process** event stream; the process's AppKit then routes it to whichever
window is its **key window** (first responder). macOS only lets the **frontmost
app** own a key window. This bounds what headless keystroke delivery can do.

## What works (proven, shipped)

- **Single-window backgrounded app**: `set-value` the field (AX value write),
  then `press <combo> --window-id w-N --policy headless`. Commits, zero focus
  steal. The omnibox is a native Cocoa `NSTextField`, so `CGEventPostToPid`
  reaches it (the Chromium renderer-IPC drop only affects *web-content*
  keystrokes, not native chrome).
- **Multi-window, target IS the key window**: same recipe, zero focus steal.
- **Per-element AX on ANY window** (incl. non-key): `set-value`, `click`,
  `focus`, `select`, `toggle` reach the element via
  `AXUIElementSetAttributeValue` / `AXUIElementPerformAction`, flash-free. These
  are not real keystrokes, so they cannot *commit* an omnibox Return — but they
  set values and press buttons anywhere.

## The wall (empirically proven on macOS 26.2 Tahoe)

Delivering a **committing keystroke to a non-key sibling window** of a
backgrounded app is not achievable from outside that app without a visible
focus change. Tested live with a real Apple Development-signed, hardened,
AX-trusted process (same privilege profile as OpenAI Codex's CUA service):

| Lever | Result |
|-------|--------|
| `AXMain=true` on target window | Takes (rc=0, readback true), but main ≠ key — keys still route elsewhere |
| `AXFocused=true` on target window | rc=0 but readback **reverts to false** — OS rejects it for a non-frontmost app |
| `app.AXFocusedWindow = target` | rc=0 but readback **stays the genuinely-key window** — silent no-op |
| `CGEventPostToPid` after any forgery | Routes to the real key window, never the forged target |
| SLPS focus byte-records (`SLPSPostEventRecordTo`, yabai-style) | rc=0 but **inert** — moves nothing; signing does not change this |
| `_SLPSSetFrontProcessWithOptions` (0x200 / 0x400) | Flips key window **but activates the app = visible focus steal** |
| AX `AXConfirm`/`AXPress` on omnibox | Reaches non-key window but does **not** commit Chromium omnibox navigation |

Corroboration: OpenAI Codex's own Computer Use, given the identical two-window
setup, could only target the key window; it fell back to window-cycling, the
Window menu, Bring All to Front, App Exposé, and a Space switch, then refused.
BCU (background-computer-use) ships an `.effectNotVerified` path that admits the
same limitation. A Perplexity Deep Research report claimed the cua "focus
forgery" recipe works with public APIs — true on macOS 14/15, but its own
"Tahoe speculative" caveat held: Apple closed the `AXFocusedWindow`-write hole
on 26.

## Decision

Ship the **refuse-guard** as the honest boundary. In
`crates/macos/src/system/key_dispatch.rs::press_for_pid_with_window_impl`, when
`focused_cg_window(pid) != requested_window_number`, return `ACTION_FAILED` with
a suggestion to use ref-based AX actions (`set-value` / `click` / `press
<REF>`) or focus the window first. The check short-circuits **before** the SLPS
preflight so no inert record is posted on the refuse path.

## Two correctness fixes that ship alongside

1. **Tahoe OOB crash**: `target_only_focus_record` must back the 0xF8-length
   SLPS record with a `[u8; 0x100]` buffer. `SLPSPostEventRecordTo` over-reads
   up to 8 bytes past the declared length on 14.2.1+/Tahoe (NSKeyedArchiver),
   crashing the process. `key_window_records` already used 0x100.
2. **Guard ordering**: the non-key short-circuit runs before
   `skylight::preflight_window`, so the refuse path never posts the inert
   (and previously crash-prone) SLPS record.

## The only remaining non-key door is app-specific IPC

Bypassing the window server entirely: CDP (`--remote-debugging-port` relaunch),
`chrome.debugger` (extension + consent), or AppleScript `execute javascript`
(gated behind *View > Developer > Allow JavaScript from Apple Events*, off by
default, not programmatically settable). Each costs a relaunch flag, an
extension, or a one-time user toggle — none are generic, so they stay out of the
generic CLI surface.

## Open lead (unconfirmed)

Codex's `SkyComputerUseService` ships a Swift `AccessibilitySupport` module
(`SyntheticAppFocusEnforcer`, `SystemFocusStealPreventer`, `KeyWindowTracker`)
that replays CPS key-focus notifications (`kCPSNotifyKeyFocusTaken/Returned`)
and suppresses focus-steal side effects. This is the likely mechanism for any
zero-flash background key routing, but it is deep, version-fragile private-API
work. Binary RE could not confirm a direct `SLEventPostToPid` call. Not pursued
for v1.
