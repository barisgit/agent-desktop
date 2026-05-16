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
- Headless mouse delivery does not unlock headless text input. The `set-value`
  and `type` chains require focus-acquiring steps that are explicitly gated by
  the interaction policy, so they will not work against a backgrounded target
  under `--policy headless`. Use `--policy focus-fallback` or `--policy physical`
  for keyboard-driven flows, or perform discovery with brief focus.
- Electron apps may collapse their AX tree while backgrounded. Discovery
  (snapshot/find) may need brief focus to populate the tree; the click itself
  does not.

## Review Rule

Any change to `PlatformAdapter::mouse_event` or `PlatformAdapter::drag` must
keep `target_pid: Option<i32>` in the signature, must keep `None` as a valid
broadcast variant for backwards compatibility, and must preserve the
`CGClickToPid`-before-`CGClick` ordering inside macOS chains.
