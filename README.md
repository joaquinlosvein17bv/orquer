# orquer

<p align="center">
  <strong>Terminal Workspace Manager with Explicit Mailbox RPC for AI Coding Agents</strong>
</p>

`orquer` evolves multiplexer-based agent coordination by replacing brittle visual screen-scraping with **explicit, voluntary, event-driven inter-agent messaging and blocking RPC**.

---

## The Problem Solved

Standard agent multiplexers (such as legacy `herdr`) attempted to detect whether agents inside terminal panes were `idle`, `working`, or `blocked` by continuously scraping terminal text and running hundreds of regular expressions. This had critical drawbacks:
1. **CPU & Battery Drain:** A background "watcher" had to poll and re-parse pane screen buffers every 50 seconds.
2. **Brittle Detection:** Terminal prompts with interactive multiple-choice menus (arrow keys, curses, inquirer) were frequently misclassified or missed entirely, causing orchestrators to hang without noticing subordinates were waiting for answers.
3. **Implicit Guesswork:** The orchestrator was forced to "guess" what workers were doing rather than receiving direct, structured signals.

## The Orquer Architecture

`orquer` replaces screen scraping with an in-daemon **Mailbox Manager & Messaging Bus**:
- **Zero Polling / Event-Driven Wait:** Orchestrators wait passively using `orquer msg recv --wait`, sleeping on an OS `Condvar` with 0% CPU consumption until an event arrives.
- **Structured Blocking RPC (`ask` / `reply`):** Subordinate agents never render interactive menus to terminal stdout. When a decision is needed, a worker invokes `orquer msg ask --question "..." --option "A" --option "B"`, which cleanly suspends the worker until the orchestrator calls `orquer msg reply --id <id> --choice "A"`.
- **100% Coexistence with Herdr:** All binaries (`orquer`, `orquer-msg`), named pipes, sockets (`ORQUER_SOCKET_PATH`), environment variables (`ORQUER_ENV`, `ORQUER_PANE_ID`), and config paths (`~/.config/orquer`, `%APPDATA%\orquer`) are strictly isolated. You can run both `herdr` and `orquer` side-by-side with zero collisions.

---

## CLI Reference: `orquer msg`

The `orquer msg` command suite (also available via the `orquer-msg` wrapper binary and root shorthands `orquer ask` / `orquer reply`) provides full messaging capabilities:

### 1. `orquer msg send`
Deposit a message in a recipient's queue:
```bash
orquer msg send --to orchestrator --type progress --payload '{"step": "schema migration complete"}'
```
- `--to <target>`: Recipient handle or pane ID (required).
- `--from <origin>`: Sender handle (defaults to `$ORQUER_PANE_ID` or "anonymous").
- `--type <type>`: `task`, `progress`, `ask`, `reply`, `result`, or `error` (default: `progress`).
- `--payload <data>`: JSON or raw string data.
- `--correlation-id <id>`: Optional correlation ID for tracing.

### 2. `orquer msg recv`
Retrieve incoming messages:
```bash
# Non-blocking immediate check
orquer msg recv --recipient orchestrator

# Passive event-driven wait (0% CPU, wakes up immediately when a message arrives)
orquer msg recv --recipient orchestrator --wait

# Passive wait with timeout (in milliseconds)
orquer msg recv --recipient orchestrator --wait --timeout-ms 60000 --json
```

### 3. `orquer msg ask` (Blocking Question RPC)
Subordinates use this when they need a decision:
```bash
# Cleanly blocks until answered, then outputs the chosen option directly to stdout:
CHOSEN=$(orquer msg ask \
  --question "Which database engine should be used?" \
  --option "SQLite" \
  --option "PostgreSQL")

echo "Decision received: $CHOSEN"
```
Or with shorthand:
```bash
orquer ask --question "Run tests?" --option "yes" --option "no"
```

### 4. `orquer msg reply`
Orchestrators resolve an `ask` question:
```bash
orquer msg reply --id "msg_1740000000_a1b2c3" --choice "PostgreSQL"
```
Or with shorthand:
```bash
orquer reply --id "msg_1740000000_a1b2c3" --choice "PostgreSQL"
```

### 5. `orquer msg list`
Inspect queued messages in a mailbox:
```bash
orquer msg list --recipient orchestrator
```

---

## Agent Skills

Orquer includes dedicated agent instructions bundled directly into the binary:

- `orquer --skill`: General workspace and pane manipulation instructions.
- `orquer --skill-orchestrator`: Dedicated orchestrator protocol (passive wait loop, task dispatching, decision resolution).
- `orquer --skill-worker`: Dedicated worker protocol (strict prohibition of interactive menus, mandatory usage of `orquer msg ask`).

Inspect or export them with:
```bash
orquer --skill-orchestrator > .agents/skills/orchestrator/SKILL.md
orquer --skill-worker > .agents/skills/worker/SKILL.md
```

---

## Building from Source

```bash
cargo build --release
```

Produces:
- `target/release/orquer.exe` (or `orquer` on Linux/macOS)
- `target/release/orquer-msg.exe` (or `orquer-msg` on Linux/macOS)

---

## License

Apache-2.0
