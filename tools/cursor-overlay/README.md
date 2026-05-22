# cursor-overlay

Companion macOS overlay process for `agent-desktop`. Renders move, click, scroll, key, target, error, and thinking visualizations on top of every screen as the agent drives the desktop.

The overlay is a separate Swift binary that listens on a Unix socket for newline-delimited JSON messages. The full wire format is in [PROTOCOL.md](./PROTOCOL.md).

## Build

```
cd tools/cursor-overlay
swift build -c release
```

The binary is at `.build/release/cursor-overlay`.

## Run

```
./.build/release/cursor-overlay --socket /tmp/cursor-overlay.sock
```

The overlay creates the Unix socket at the given path, accepts one or more clients, and renders every event it receives. Press `Ctrl+C` to stop.

## Environment contract with agent-desktop

`agent-desktop` discovers and (optionally) summons the overlay through two environment variables. Both are optional; if neither is set, agent-desktop runs with no visualization and no overhead.

### `AGENT_CURSOR_SOCKET`

Full path to the Unix socket where the overlay is listening. When this variable is set, every `agent-desktop` invocation emits JSON events for every interesting action (move, click, scroll, type, key combo, target, error, thinking).

```
export AGENT_CURSOR_SOCKET=/tmp/cursor-overlay.sock
```

If the socket is not bound when `agent-desktop` runs, the event is dropped silently and the CLI's normal output is unaffected.

### `AGENT_CURSOR_START_CMD`

Optional shell command that `agent-desktop` runs whenever the socket is not bound. The command is fire-and-forget: `agent-desktop` spawns it through `sh -c`, detaches it via `setsid`, and never waits for it or kills it.

```
export AGENT_CURSOR_START_CMD='cursor-overlay ensure-running --socket $AGENT_CURSOR_SOCKET'
```

After spawning, `agent-desktop` polls the socket for up to 300ms (one connect every 25ms). If the socket appears in that window the queued event is delivered; otherwise the event is dropped and the next invocation retries.

### Three-rule contract for any start command

Whatever you put in `AGENT_CURSOR_START_CMD` MUST satisfy all three of:

1. **Idempotent.** Multiple `agent-desktop` processes can run in parallel and all of them will fire the start command against a cold socket. Only one should actually bind. Implement this idempotency with `flock`, a connect-probe, or a `cursor-overlay ensure-running` subcommand that exits quickly when an overlay is already running.
2. **Detach.** Return to the shell within ~50ms; never block waiting for the overlay's main loop. The overlay must survive the parent `agent-desktop` invocation exiting. The simplest pattern: `nohup cursor-overlay --socket "$AGENT_CURSOR_SOCKET" </dev/null >/var/log/cursor-overlay.log 2>&1 &`.
3. **Return fast.** Even on first cold start, the start command should return well under the 300ms poll budget. If `cursor-overlay` itself takes longer than 300ms to bind the socket on a cold machine, the first event is dropped, but subsequent events will connect normally.

## Lifetime model

`agent-desktop` does NOT manage the overlay's lifetime:

- No PID file is read or written.
- No `kill` is issued on exit.
- No auto-restart loop runs if the overlay crashes.
- The Drop impls on the agent's overlay client only close the socket — they never terminate the child.

This is intentional: `agent-desktop` is a stateless CLI invoked thousands of times per session. It would be wrong for any invocation to own a long-running process. Lifetime management lives entirely in your start command and in whatever supervisor you wrap it in (launchd, systemd, a foreground terminal, etc.).

`agent-desktop` is also responsible for nothing on the socket side: a fire-and-forget JSON write per event. If the overlay dies mid-session, the next event triggers a fresh probe-and-summon cycle.

## Worked example: launchd-managed overlay

```
# ~/Library/LaunchAgents/com.example.cursor-overlay.plist
# (...standard launchd plist that runs cursor-overlay --socket /tmp/cursor-overlay.sock)

launchctl load ~/Library/LaunchAgents/com.example.cursor-overlay.plist

# In your shell rc:
export AGENT_CURSOR_SOCKET=/tmp/cursor-overlay.sock
export AGENT_CURSOR_START_CMD='launchctl start com.example.cursor-overlay'
```

Now every `agent-desktop` invocation renders to the overlay. If launchd has the overlay up: hot path, ~50µs per event. If the overlay is missing: agent-desktop fires the start command, launchd brings it up, the event is delivered.

## Worked example: cold-start a script in a terminal

```
# Terminal A:
cursor-overlay --socket /tmp/cursor-overlay.sock

# Terminal B:
export AGENT_CURSOR_SOCKET=/tmp/cursor-overlay.sock
agent-desktop click --window Finder --ref @e3
```

No `AGENT_CURSOR_START_CMD` needed; you own the overlay's lifetime in Terminal A.

## Protocol

See [PROTOCOL.md](./PROTOCOL.md) for the exact JSON shape of every event, the `target_pid` convention, and the back-compat policy for the deprecated `set_thinking` event.
