# cursor-overlay socket protocol

## Transport

- Unix domain socket, path from `--socket PATH` flag or `AGENT_CURSOR_SOCKET` env var.
- Single client at a time. Connecting a second client disconnects the first.
- Wire format: newline-delimited JSON. One object per line, terminated with `\n`.

## Coordinates

- All x/y values are global CG screen coordinates with **top-left** origin, matching the agent-desktop `mouse-click --xy X,Y` semantics.
- On a multi-display setup, the overlay converts to per-screen AppKit coords internally.

## Messages

### move
Update the virtual cursor's target position. Cursor animates to the new point.
```json
{"kind":"move","x":1192,"y":369}
```

### click
Move the cursor and emit a ripple at the click point.
```json
{"kind":"click","x":1192,"y":369,"button":"left","count":1}
```
Fields: `button` ∈ `"left"|"right"|"middle"` (default `"left"`), `count` integer (default `1`).

### set_visible
Show or hide the virtual cursor and ripples without tearing down the window.
```json
{"kind":"set_visible","visible":false}
```

### set_color
Override the cursor and ripple color. Components are 0.0–1.0 doubles.
```json
{"kind":"set_color","r":0.247,"g":0.541,"b":0.988}
```

### bye
Cleanly terminate the overlay process.
```json
{"kind":"bye"}
```

## Manual smoke test

```bash
# Start the overlay
./.build/release/cursor-overlay --socket /tmp/co.sock &

# Send a move + click
printf '{"kind":"move","x":500,"y":400}\n' | socat - UNIX-CONNECT:/tmp/co.sock
printf '{"kind":"click","x":500,"y":400,"button":"left","count":1}\n' | socat - UNIX-CONNECT:/tmp/co.sock
```
