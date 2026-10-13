# MCP server

WordCraft speaks the [Model Context Protocol](https://modelcontextprotocol.io) over stdio, so
Claude and other agents can write and edit documents.

```sh
wordcraft-cli mcp                          # headless session (no window)
wordcraft --control 7981 &                 # or: drive the running app…
wordcraft-cli mcp --connect 127.0.0.1:7981 # …including clicks, keys and screenshots
```

Claude Code: `claude mcp add wordcraft -- wordcraft-cli mcp`.

With `--connect`, the bridge sends the app's control key, which it reads from
`WORDCRAFT_CONTROL_KEY` or from the key file the app writes at start. It sends the key only to
loopback addresses; see [Keys](control-protocol.md#keys).

## Tracked changes by an agent

Tracked changes and comments are recorded under the session's user name ("WordCraft User" by
default). Name the agent so reviewers can tell its edits from people's: start the headless server
with `wordcraft-cli mcp --author "Claude (copyedit)"`, or run
`file.setAuthor {"name": "Claude (copyedit)"}`. Turn tracking on explicitly with
`review.trackChanges {"value": true}`; without `value` it toggles, which turns tracking *off* in
a document that was saved with it on. `wordcraft-cli run` takes `--author` too.

## Edits from assistants

Whatever an assistant sends to a running app over the control port (`wordcraft-cli mcp --connect`,
or a raw client) arrives as a suggestion, enforced in the app and not in the client:

- A command that changes the document runs with track changes forced on and the author set to the
  assistant name from Options › Assistant ("Claude" by default); comments it adds carry that name
  too. Your own tracking state and name come back afterwards.
- An assistant cannot accept or reject changes (`review.accept*`, `review.reject*`), turn tracking
  off, change the author or the assistant settings (`review.trackChanges`, `file.setAuthor`,
  `file.options` with `author`/`initials`, `ui.assistant`), undo, redo or repeat (`edit.undo`,
  `edit.redo`, `edit.undoTo`, `edit.repeat`), run macros (`tools.macros`, `tools.recordMacro`),
  restrict editing (`review.restrict`), or change or delete comments it did not write. The reply is
  `not allowed for assistants: <command> (<why>)`. Raw pointer and keyboard input (`ui.click`,
  `ui.key`, `ui.menu`, …) and `app.quit` are refused too, since they could press Accept. Looking
  (`document.inspect`, `ui.inspect`, `ui.screenshot`, `ui.render`, `engine.commands`) and read-only
  commands are allowed.
- Options › Assistant › "Allow assistants outside Margin to connect" (on by default; the port only
  exists when the app was started with `--control`) refuses every request while off and drops the
  connected assistants; "Disconnect" drops them without turning it off.
- The headless `wordcraft-cli mcp` (no app) edits its own session and files directly and is not
  covered. The rules live in `wordcraft_engine::assistant` (`denied`, `Session::run_as_assistant`).

## Confining file access

```sh
wordcraft-cli mcp --automation-read-root /work/project --automation-write-root /work/project/out
```

```json
{"mcpServers": {"wordcraft": {"command": "/abs/path/target/release/wordcraft-cli",
  "args": ["mcp", "--automation-read-root", "/work/project", "--automation-write-root", "/work/project/out"]}}}
```

The two flags (VectorCraft, PhotoCraft and the other Craft apps take the same ones, so one client
config works across the suite) keep the agent to the folders you give it:

- Every file a command reads must lie inside the **read root**: `open_document` and `file.open`,
  `file.newFromTemplate`, `insert.textFromFile` and `insert.object`, `insert.picture` and
  `picture.change` with a `path`, `mailings.recipients` with a `path` (csv, tsv, txt, xlsx, xlsm,
  ods), `review.compare` (`path` and `original`), the templates and documents `tools.updateStyles`,
  `styles.organizer` and `styles.copyFrom` read. The pictures an opened HTML file names are read
  from beside that file, so they are inside the read root too (a link that leads out is not followed).
- Every file a command writes must lie inside the **write root**: `save_document`, `file.save`,
  `file.saveAs`, `file.exportPdf`, `file.exportPng`, `file.printPdf`, `file.saveTemplate`, and `mailings.finish` with
  a `path`.
- Read and write access are separate. Give the same folder twice for both; a root left out grants
  none of its access (with only `--automation-read-root`, nothing can be written to disk). Without
  either flag nothing changes.
- A path is made absolute against the server's working directory, then its links are followed: a
  file (or folder) link that leads out of a root is outside it. For a file that doesn't exist yet,
  its nearest existing folder's links are followed and the rest must be plain names, so `..` below a
  folder that doesn't exist is refused, as is a link that leads nowhere (writing through it would
  create its target). The result is compared with the root by whole folder names: `/work2/x` is not
  inside `/work`. On Windows case is ignored, UNC paths (`\\server\share\...`) are resolved like
  drive-letter ones, and device names (`CON`, `NUL`, `COM1`...) are refused.
- A refused path is an ordinary tool error the agent can read and correct, for example
  `automation path rejected: outside the write root /work/project/out: /etc/x.docx` or
  `automation filesystem access is not granted: write authority is absent: /work/project/out/x.docx`.
- The roots must be existing folders; a missing one stops the server with an error. Each flag is
  given once (`--flag <dir>` or `--flag=<dir>`).

The flags confine the headless server; `--connect` with them is an error, since the server would
otherwise talk to an app that isn't confined. WordCraft's desktop app has no equivalent flags yet,
so a running app (`wordcraft --control`) is not confined.

What the roots do not cover: commands that take no path (`text.insert`, `execute` of anything that
works on the open document) can still read and change that document, and `get_text` and
`inspect_document` return it. The Zotero commands talk to a local Zotero over a socket, not to files.
Anything that reads or writes a path goes through one place in the engine (`io::open_path_with`,
`io::save_path_with`, `io::read_file`, `io::write_file`); a test keeps command modules from
touching the file system on their own.

The checks hold for every thread of the server process. A path is checked, then opened: another
program that swaps a link in between can still win that race, so the roots keep an agent to the
files it was given rather than guarding against other software on the computer.

## Tools

| Tool | What it does |
|---|---|
| `list_commands` | every command (filter with `query`) — over 270 of them |
| `execute` | run one command: `{command: "insert.table", params: {rows: 3, cols: 4}}` |
| `batch` | run several commands in order |
| `new_document` | blank, sample, letter, resume, report |
| `open_document` / `save_document` | docx, doc (open only), odt, rtf, html, md, tex, txt, json; save also pdf, png |
| `type_text` | type at the caret (`paragraphs: true` splits lines into paragraphs) |
| `select_text` | select the n-th occurrence of some text |
| `get_text` / `inspect_document` | read the document back (verify without screenshots) |
| `render_page` | a page as PNG |
| `parity` | feature coverage |
| `screenshot`, `click`, `key`, `ui_inspect` | app only (`--connect`) |

Resources: `wordcraft://document` (inspect) and `wordcraft://commands`.

The acceptance test `crates/mcp/src/tests.rs` writes a formatted document using MCP only.
