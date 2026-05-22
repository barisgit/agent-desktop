# cursor-overlay socket protocol

## Transport

- Unix domain socket, path from `--socket PATH` flag or `AGENT_CURSOR_SOCKET` env var.
- Single client at a time. Connecting a second client disconnects the first.
- Wire format: newline-delimited JSON. One object per line, terminated with `\n`.
- Every event is one-shot and fire-and-forget: there is no ACK, retry, or response payload. If the socket disconnects, agent-desktop drops that event and reconnects on the next event.

## Coordinates and targeting

- All x/y values are global CG screen coordinates with top-left origin, matching the agent-desktop `mouse-click --xy X,Y` semantics.
- On a multi-display setup, the overlay converts to per-screen AppKit coords internally.
- `target_pid` is an optional field on `move`, `click`, `scroll`, and target-set messages. When present, it identifies the target application process for visibility gating; when omitted, the event is treated as broadcast overlay feedback.

## Messages

### move

Move the virtual cursor to a global screen point. The renderer glides the cursor toward the point and may gate visibility by `target_pid`.

Shape: `{"kind":"move","x":N,"y":N,"target_pid":N?}`

Optional fields: `target_pid` is omitted for broadcast moves.

Example:

```json
{"kind":"move","x":1192,"y":369,"target_pid":12345}
```

### click

Move the cursor to the click point and emit a click ripple. The renderer uses `button` and `count` to choose the ripple style; unsupported button labels should be treated as left click by callers.

Shape: `{"kind":"click","x":N,"y":N,"button":"left|right|middle","count":N,"target_pid":N?}`

Optional fields: `target_pid` is omitted for broadcast clicks. Emitters should always send both `button` and `count`.

Example:

```json
{"kind":"click","x":1192,"y":369,"button":"left","count":1}
```

### scroll

Show scroll feedback at a target point. `dx` and `dy` are logical scroll deltas, not per-frame animation values.

Shape: `{"kind":"scroll","x":N,"y":N,"dx":N,"dy":N,"target_pid":N?}`

Optional fields: `target_pid` is omitted when the scroll is not tied to one application process.

Example:

```json
{"kind":"scroll","x":500,"y":400,"dx":0,"dy":-3,"target_pid":12345}
```

### key

Show keyboard feedback for a text insertion or a key combo. Exactly one of `text` or `combo` is present; messages containing both or neither are invalid.

Shape: `{"kind":"key","text":"..."}` or `{"kind":"key","combo":"cmd+s"}`

Optional fields: none; `text` and `combo` are mutually exclusive alternatives.

Examples:

```json
{"kind":"key","text":"hello world"}
{"kind":"key","combo":"cmd+s"}
```

### target

Set or clear a target highlight rectangle. A set message displays a box around the target bounds; a clear message removes the current box.

Shape: `{"kind":"target","x":N,"y":N,"w":N,"h":N,"target_pid":N?}` or `{"kind":"target","clear":true}`

Optional fields: `target_pid` is omitted when the highlighted target is not tied to one application process. `clear:true` messages contain no geometry fields.

Examples:

```json
{"kind":"target","x":100,"y":200,"w":300,"h":40,"target_pid":12345}
{"kind":"target","clear":true}
```

### error

Show transient error feedback with a machine-readable code and human-readable message. If a point is present, the renderer may place the flash near that point; otherwise it uses its current cursor context.

Shape: `{"kind":"error","x":N?,"y":N?,"code":"STALE_REF","message":"..."}`

Optional fields: `x` and `y` are omitted when no point is known. `code` and `message` are always required.

Example:

```json
{"kind":"error","x":500,"y":400,"code":"STALE_REF","message":"Run snapshot again"}
```

### thinking

Toggle the renderer's busy visual state. Payload field: `thinking: bool`.

Shape: `{"kind":"thinking","thinking":true}` or `{"kind":"thinking","thinking":false}`

Optional fields: none.

Example:

```json
{"kind":"thinking","thinking":true}
```

### set_visible

Show or hide the virtual cursor and related overlay effects without tearing down the overlay window. This is a persistent renderer setting until the next `set_visible` message.

Shape: `{"kind":"set_visible","visible":true|false}`

Optional fields: none.

Example:

```json
{"kind":"set_visible","visible":false}
```

### set_color

Set the overlay cursor color. Components are `u8` values in the inclusive range 0-255.

Shape: `{"kind":"set_color","r":N,"g":N,"b":N}`

Optional fields: none. Values outside 0-255 are invalid for v2 emitters.

Example:

```json
{"kind":"set_color","r":63,"g":138,"b":252}
```

### bye

Ask the overlay process to terminate gracefully after processing prior messages from the same connection. This is a one-way shutdown request and has no reply.

Shape: `{"kind":"bye"}`

Optional fields: none.

Example:

```json
{"kind":"bye"}
```

## Manual smoke test

```bash
./.build/release/cursor-overlay --socket /tmp/co.sock &
printf '{"kind":"move","x":500,"y":400}\n' | socat - UNIX-CONNECT:/tmp/co.sock
printf '{"kind":"click","x":500,"y":400,"button":"left","count":1}\n' | socat - UNIX-CONNECT:/tmp/co.sock
printf '{"kind":"thinking","thinking":true}\n' | socat - UNIX-CONNECT:/tmp/co.sock
```
