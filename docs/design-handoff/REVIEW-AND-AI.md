# Review, comments and AI — addendum to BRIEF.md

Screens in `review/` (1440 × 900 pt at 2×). The scene: Martijn edits with Track Changes on, then
"Claude (co-writer)" edits and comments, then Martijn comments.

## 1. Comments and track changes today

| File | What it shows |
|---|---|
| `01-track-changes-comments-pane-light` | Tracked changes in the page, coloured **per author** (Martijn magenta, Claude blue): insertions underlined, deletions struck through, a bar in the margin. Comments pane on the right: author, time, text, Go to / Reply / Resolve / Delete. Status bar: "Track Changes: On". |
| `05-track-changes-comments-pane-dark` | Same, dark. |
| `02-comment-balloons-light` | Comments as balloons beside the page, joined to their text by a dashed line, in the author's colour. |
| `03-track-changes-options-dialog-light` | Track Changes Options (upstream dialog): what to show, balloons vs inline, colours by author. |
| `04-classic-review-tab-light` | The full Review toolset in the Classic ribbon: spelling, word count, comments, track changes, accept/reject, compare, restrict editing, read aloud. |

**What the engine supports:** an author on every change and comment; accept / reject one or all;
next / previous change; replies and resolved comments; compare two documents; restrict editing to
comments or tracked changes only; everything saved to .docx so Word users see the same.

**Gaps in Margin's interface today (design these):**
- Review has no home in Margin's layout: only the toolbar's Track Changes toggle and Comment button.
  Accept / Reject / Next live in the Review menu only.
- The Comments pane is WordCraft's styling (cramped button row, boxed text). It should match the
  Format inspector.
- No overview of "who changed what": the engine can list all changes (`review.changes`), the UI
  doesn't show it.

## 2. AI integration today (MCP)

Margin speaks **MCP** (Model Context Protocol), the standard way Claude and other AI assistants use
tools. An assistant can open, read, write and save documents and run any of the ~600 commands:

| Tool | What it does |
|---|---|
| `list_commands` / `execute` / `batch` | find and run any command (format, insert table, accept change…) |
| `new_document`, `open_document`, `save_document` | files (docx, odt, rtf, md, html, pdf…) |
| `type_text`, `select_text`, `get_text`, `inspect_document` | write and read the text |
| `render_page`, `screenshot`, `click`, `key` | see the page / drive the open window |

- **Two modes:** headless (the AI works on a file, no window), or **connected to the open app**: the
  AI edits the document you have on screen, live.
- **The AI has its own author name** (e.g. "Claude (co-writer)"). With Track Changes on, every AI
  edit is a suggestion you accept or reject, in its own colour. That already separates AI-written
  from hand-written text — until a change is accepted.
- **Sandboxed:** the AI can be limited to read-only / write-only folders.
- **Today it is set up from outside** (Claude Code / Claude Desktop config). There is no AI panel
  inside Margin yet.

## 3. Direction: AI as a co-worker (to explore in Claude Design)

Goal (Martijn): the AI works **next to you** in the document like a colleague, and you can always
see **what the AI wrote and what a person wrote**.

Principles:
- **Human in the loop.** The AI suggests; it never silently changes your text. Its edits arrive as
  tracked changes or comments under its own name, and you accept or reject.
- **Visible authorship.** A clear, calm visual language for "AI-written": colour, margin mark,
  a toggle to show / hide it, and a per-paragraph summary ("70 % written by Claude").
- **Model choice in Settings** (Claude, Mistral, on-device…), not hard-wired to one vendor.

Ideas to design:
1. **An AI tab or panel** next to Format (a chat/instructions area: "tighten this section",
   "check numbers against …"), with the AI's work listed as reviewable changes.
2. **Inline invocation:** select text → ask the AI (in the floating format bar or a ⌘-shortcut).
3. **Authorship view:** a View option that highlights everything the AI wrote, even after acceptance.
4. **A Review section in the inspector** that shows people and AI side by side: changes by author,
   accept / reject per author ("accept all of Claude's grammar fixes").
5. **Presence:** when the AI is working in the document, show it (like a collaborator's cursor).

What is buildable, and at what cost:

| Feature | Cost | Why |
|---|---|---|
| AI edits as tracked changes under its own name, in its own colour | **exists** | engine + MCP today |
| Review section in the inspector, per-author accept/reject | small–medium | UI over existing commands |
| AI panel inside Margin (prompt box, history) | medium | needs a model connection and API-key setting (Keychain) |
| Authorship that **survives acceptance** | **large** | the engine must keep "who wrote this" on text after a change is accepted, and store it in the file; Word ignores it (it would be Margin-only metadata) |
| Live presence / cursor of the AI | medium | UI over the control channel |

Not possible / out of scope for now: real-time multi-user collaboration (several people at once),
cloud sync. The AI runs on your machine's connection to the model you choose.
