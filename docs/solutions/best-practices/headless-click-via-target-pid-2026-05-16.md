---
title: Headless click via target-pid using CGEventPostToPid
date: 2026-05-16
category: best-practices
module: crates/macos
problem_type: best_practice
component: macos-input
severity: high
applies_when:
  - An agent needs to click an element in a backgrounded macOS app without stealing focus or moving the cursor
  - The element has been resolved via snapshot/find and a ref is available
  - The target process is known by app name or pid for raw coordinate clicks
tags:
  - macos
  - headless
  - mouse
  - cli-parity
  - interaction-policy
---

# Headless click via target-pid using CGEventPostToPid

## Context

`CGEventPost(kCGHIDEventTap, event)` broadcasts mouse events to the OS-wide HID
tap, which moves the real cursor and follows whatever window the front-most app
has under that coordinate. That is wrong for an agent that wants to click inside
a backgrounded app while the user keeps working in the front-most app.

macOS exposes `CGEventPostToPid(pid, event)`. Events posted that way are
delivered to a single process and do not move the user's cursor. The headless
click path uses this primitive to act on a backgrounded target.

## Guidance

For ref-based interactions (`click`, `double-click`, `right-click`, `focus`):

- The default ref-action policy is headless. Resolve the ref, look up the owning
  pid via the platform adapter, and route through the pid path. No extra flag is
  needed.
- The chain inserts a `CGClickToPid` step before the existing `CGClick` step, so
  the physical fallback path is preserved bit-identical when the policy permits
  it.

For raw coordinate commands (`mouse-click`, `mouse-down`, `mouse-up`,
`mouse-move`, `hover`, `drag`):

- Pass `--policy headless` together with `--target-app <name>` or
  `--target-pid <pid>`. The two target flags are mutually exclusive.
- Without either target flag under `--policy headless`, the command returns
  `INVALID_ARGS` and the suggestion field names both `--target-app` and
  `--target-pid`. The CLI never guesses a pid for raw coordinates.
- The default `--policy` for raw commands is `physical`, which preserves the
  previous broadcast behavior. `--policy focus-fallback` is also accepted.

## Limitations

- Sandboxed and hardened-runtime apps (Mail, Notes, App Store, most Mac App
  Store apps) may silently discard `CGEventPostToPid` events. Fall back to
  `--policy focus-fallback` or `--policy physical` for those targets.
- Drag under headless is best-effort. Cross-app drag-and-drop flows that depend
  on a real cursor are not expected to complete.
- Headless mouse delivery does not unlock fully headless text input. `set-value`
  tries a direct AX value write first and succeeds for many text fields; when
  the AX write is rejected, the fallback chain steps are keyboard-based and need
  focus, so they are gated off under headless. `type` is keyboard-only and also
  requires focus today. Use `--policy focus-fallback` or `--policy physical`
  when keyboard-driven flows are required, or perform discovery with brief
  focus.
- Electron apps may collapse their AX tree while backgrounded. Discovery
  (snapshot/find) may need brief focus to populate the tree; the click itself
  does not.
- Enter-submit via `press --app <name> return` is app-specific. Some web and
  Electron apps accept the keystroke but do not fire the in-page Enter handler,
  so the field stays focused without submitting. Prefer clicking the submit
  control directly when Enter-submit cannot be confirmed.

## Review Rule

Any change to `PlatformAdapter::mouse_event` or `PlatformAdapter::drag` must
keep `target_pid: Option<i32>` in the signature, must keep `None` as a valid
broadcast variant for backwards compatibility, and must preserve the
`CGClickToPid`-before-`CGClick` ordering inside macOS chains.

## Appendix: SkyLight private-API probe (negative result)

Before settling on `CGEventPostToPid`, we probed SkyLight private symbols to
see if any path improved on the sandboxed-app gap. None did. This appendix
captures the matrix so a future investigation does not have to rediscover the
symbol set when Apple changes routing on a new macOS release.

Framework: `/System/Library/PrivateFrameworks/SkyLight.framework/SkyLight`.
All symbols below resolve via `dlopen` + `dlsym` on macOS 14.x.

| Variant         | API                                          | Outcome on macOS 14.x                                                                  |
| --------------- | -------------------------------------------- | -------------------------------------------------------------------------------------- |
| `sl-bare`       | `SLEventPostToPid(pid, CGEvent)`             | Indistinguishable from `CGEventPostToPid`; sandboxed apps still discard the event.     |
| `sl-winfields`  | `SLEventPostToPid` + event fields 28, 29     | Setting `windowUnderMouse` / `windowUnderMouseHandler` to a CGWindowID does not change sandbox acceptance. |
| `sl-combined`   | `SLEventPostToPid` w/ `combinedSessionState` | Same routing as `sl-bare`.                                                             |
| `sl-private`    | `SLEventPostToPid` w/ `privateState`         | Same routing as `sl-bare`.                                                             |
| `slps-conn`     | `SLPSPostEventRecordTo(conn, CGEvent)`       | Posts via the agent's own CGS connection (`CGSMainConnectionID()`); does not cross pids. |
| `cgs-mouse`     | `CGSPostMouseEvent(conn, type, point, btn)`  | Ignores pid; behaves like a coarser broadcast (close to the HID tap). Not useful.      |

`CGEventField` raw values `28` and `29` correspond to
`kCGMouseEventWindowUnderMousePerWindow` and
`kCGMouseEventWindowUnderMousePerWindowHandler`. They are not exposed in any
public SDK header and originate from leaked OpenStep-era enums.

Verdict: use `CGEventPostToPid`. Fall back to `--policy focus-fallback` or
`--policy physical` for sandboxed targets.

### Regenerating the probe

The probe is short enough that any coding agent can regenerate it from this
matrix in a few minutes. Hand it the table above plus this skeleton:

- Swift command-line tool, single `main.swift`.
- `dlopen` `SkyLight.framework`, `dlsym` each candidate symbol.
- `unsafeBitCast` resolved symbols through `@convention(c)` function types
  with these signatures:
  - `SLEventPostToPid: (pid_t, OpaquePointer) -> Int32`
  - `SLPSPostEventRecordTo: (Int32, OpaquePointer) -> Int32`
  - `CGSPostMouseEvent: (Int32, Int32, CGPoint, Int32) -> Int32`
  - `CGSMainConnectionID: () -> Int32`
- For each variant build a `CGEvent(mouseEventSource:mouseType:mouseCursorPosition:mouseButton:)`,
  set `mouseEventClickState = 1`, set fields 28/29 when applicable, post
  down + 50 ms sleep + up, and report return codes as JSON.
- CLI: `--list`, `--pid`, `--xy x,y`, `--variant <name|all>`, `--window <cgwindowid>`.

Rebuild with `swiftc -O main.swift -o skylight-probe` and run against a known
target pid. If any variant starts delivering clicks to a previously-discarding
sandboxed app on a new macOS version, that is the moment to revisit the
macOS adapter's headless primitive.
