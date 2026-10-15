# desktopd: session-first desktop control process

Status: draft, parked (2026-10). Nothing here is implemented. Revisit after
upstream lands more of its Windows adapter and Phase 4/5 work.

## Why

agent-desktop today is a CLI that starts, reads the accessibility tree, acts,
and exits. That shape causes the problems we hit while building on it:

- Refs, snapshots, and traces round-trip through files, and the global
  "latest snapshot" lets one agent consume another agent's refs.
- Held input (`key-down`, `mouse-down`) fails closed because no owner outlives
  a command to guarantee the release.
- The focus guard only watches for 400 ms after each background action.
- Permissions (Accessibility, Screen Recording) are granted to whichever
  terminal runs the binary.
- 59 commands with headless, `--background`, and `--headed` variants are too
  many controls for an agent; most work uses about six.
- Upstream will not maintain SkyLight background input (#227-#229 closed), so
  our routing, focus guard, and overlay work needs its own home.

## Goals

1. One long-running process per logged-in user owns all desktop state.
2. Every interaction belongs to a session.
3. A small agent API: open, observe, act, wait, close.
4. The process picks the least intrusive input route and verifies results.
5. MCP, CLI, and SDK are thin clients of the same API.
6. Decision models (jev or others) plug in as clients, never inside the core.
7. Platform code stays behind the existing `PlatformAdapter` trait.

Non-goals: embedding a model, planning, browser automation, GUI beyond the
cursor overlay.

## Process model

```
 Agents                          Deciders (jev, any model)
 MCP / codemode / CLI / SDK      observation in -> action out (ref or bbox)
        \                         /
         v                       v
   +----------------------------------------------+
   | desktopd (one per logged-in user)            |
   |  sessions, API, route selection, guards,     |
   |  in-memory refs, traces, overlay             |
   +----------------------------------------------+
         |  PlatformAdapter
   macOS (AX + SkyLight)    Windows (UIA)    Linux (AT-SPI)
```

- Scope is the GUI login session, not the machine: macOS TCC grants and the
  accessibility server are per user session.
- macOS: signed `.app` bundle started by a LaunchAgent, listening on a Unix
  socket under the state root (mode 0700). Permissions attach to the app.
- Clients auto-start the process when the socket is missing.
- Crash safety: on restart, sessions are gone and held input is released by
  the OS when the process exits; clients get `SESSION_NOT_FOUND`.

## Sessions

Nothing runs outside a session.

| Owned by a session | Purpose |
|---|---|
| Refs and snapshots (memory) | No global "latest"; refs are session-scoped |
| Policy | Allowed apps, allowed routes, may it take focus or move the pointer, lock behavior |
| Identity | Agent name, cursor color and label; every action is attributed and shown |
| Trace | Always on; evidence for replay and debugging |
| Held input, leases | Released on close, disconnect, or idle timeout |
| Decider (optional) | Model attached to the session, not wired per command |

Lifecycle:

- `open(policy) -> session_id`; ends on `close`, client disconnect (socket and
  MCP), or idle timeout.
- MCP: one connection is one implicit session.
- CLI: `desktop session open` prints an ID for the environment; a command with
  no session runs in an ephemeral session closed after that command.
- Child sessions share the parent's refs and policy and get their own cursor.
  This replaces `--multi-agent` and `--agent-id`.

Across sessions:

- One machine-wide input lease serializes input delivery.
- Two sessions in the same window get a conflict warning; stale refs fail with
  `STALE_REF` rather than acting on the wrong element.
- `sessions` lists active sessions, their app, and their cursor.

## Agent API

| Call | Behavior |
|---|---|
| `observe(target, mode)` | `target` is app or window; `mode` is `tree`, `image`, or `both`. Tree is compact with refs (skeleton first, drill on request). Image can carry ref marks |
| `act(target, action, text?)` | `target` is a ref, a role+name locator, or a point/box. Actions: click, type, set, press, scroll, hover, drag, select, check, expand |
| `wait(condition)` | Element, text, window, or idle; also used as `expect` |
| `apps`, `windows` | Inventory |
| `open`, `close`, `sessions` | Session lifecycle |

`act` returns delivery (`verified`, `unverified`, `not_delivered`), the route
used, and what changed since the last observation.

"Smart, not too smart" means: auto-wait for actionability, route selection,
verification by re-observation, change summaries, and recovery hints. It never
plans multi-step work or calls a model.

## Input routes

Ordered from least to most intrusive; the session policy caps the ceiling.

1. Headless: accessibility actions and attribute writes (upstream behavior,
   including the Electron `type` value fallback from our fork).
2. Background: events posted to the target pid/window with the SkyLight layers
   (route fields, `SLEventPostToPid`, target-only activation record, key-window
   pair). Pointer, wheel, keyboard. Best effort, reported unverified.
3. Headed: real pointer and keyboard; requires policy permission to take focus.

Selection is per action: try the lowest allowed route that the target
supports, escalate only when it reports not delivered or a known gap (hover
menus in Chromium, ScrollTo-only panes, Electron text fields).

## Guards

- Focus guard: continuous, not per action. Restores the user's frontmost app
  when a session's target steals focus; yields when the user switches apps
  themselves.
- Input lease and held-input release (see sessions).
- Lock screen:
  - Phase 1: power assertion to keep the display awake while a session is
    active; on lock, actions fail fast with `SCREEN_LOCKED` and sessions resume
    after unlock.
  - Later, opt-in only: auto-unlock. Codex Computer Use ships this
    (`LockScreenAutoUnlockCoordinator`, `LockScreenLoginAuthorizationBroker`,
    `SystemLockScreenPhysicalInputMonitor`, overlay presenter; inferred from
    symbol names, not run). It requires storing the login credential and must
    stop on any physical input.

## Deciders

A decider is a client. It receives an observation (tree, image, or both) and
returns an action whose target is a ref or a bounding box. desktopd grounds a
box to the element under it (accessibility hit test) so the action can stay
headless, and falls back to a background point action. jev (TypeSafe; now with
image input and box output) becomes one decider adapter instead of a script
that shells out per step.

## Surfaces

- MCP (stdio shim to the socket, plus local streamable HTTP): the agent API as
  a few coarse tools with typed results; suits codemode chaining.
- CLI: thin client, same calls.
- SDK/FFI: same protocol; the existing C ABI can wrap the socket client.

## Platform layer

Keep `agent-desktop-core` and the platform crates. Add trait capabilities for
input route, focus guard, and lock state. The macOS crate keeps the AX
workarounds that are the main value of the codebase.

## What carries over from the fork

From `merged`: web ref allocation fix, overlay improvements (5 s hold, 1 s
fade, window outline, one window lookup per action, retire timeout), Electron
type fallback, background pointer/keyboard/wheel with SkyLight layers and the
target-surface check, background overlay cues.

## Open decisions

1. Fork strategy: restructure `merged` in place and cherry-pick upstream fixes
   (preferred), or a new repository reusing the crates.
2. Name, and whether it is published or personal.
3. Lock screen: keep-awake plus pause only, or also opt-in auto-unlock.
4. Platforms: macOS first with the trait kept, or pull upstream's Windows
   adapter early.
5. Protocol: JSON-RPC over the socket (shared with MCP) or a separate binary
   protocol.
