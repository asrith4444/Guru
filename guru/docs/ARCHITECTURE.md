# Guru architecture

Guru is a desktop app (Tauri 2) with a Rust core and a framework-free web interface
rendered by the operating system's web view. AI is used in one place, building a course,
through the Codex CLI. Everything after that is local.

```
 ┌──────────────── ui/ (OS web view: WebKit / WebView2) ────────────────┐
 │ library · new course (level bar) · course · lesson · quiz · cards    │
 │ simulation player ── engine.js (shell, fs, processes, logs, tools)   │
 └───────────────▲──────────────────────────────┬───────────────────────┘
                 │ events: download, login-*    │ invoke(command)
 ┌───────────────┴──── app/ (guru-app, Tauri) ──▼───────────────────────┐
 │ thin command layer: courses, progress, cards, download control,      │
 │ sign-in, optional "Ask AI" (ask_guru, ai_command)                    │
 └───────────────────────────────┬──────────────────────────────────────┘
 ┌──────────────── core/ (guru-core) ▼──────────────────────────────────┐
 │ goal       goal → clarifying questions → professional stage roadmap  │
 │ generator  preflight → plan → research → build (parallel, retried,   │
 │            time-limited, quality-gated) → validate → ready           │
 │ sim        simulation spec + semantic validator                      │
 │ course     package on disk, progress, mastery                        │
 │ scope      level bar → hours        srs  spaced repetition (SM-2)    │
 │ research   Stack Exchange API (curl)  machine  RAM/CPU → worker count│
 │ codex      `codex exec --json --output-schema …` (Sign in with ChatGPT)│
 └──────────────────────────────────────────────────────────────────────┘
```

## Course package

A downloaded course is a plain folder in `~/.guru/courses/<slug>/` (or `$GURU_HOME`):

| File | Contents |
|---|---|
| `course.json` | plan: skills, modules, simulation plans, capstone, level, hours, status |
| `modules/<id>.json` | lesson (Markdown), key points, flashcards, quiz with explanations |
| `sims/<id>.json` | simulations (spec below) |
| `progress.json` | lessons read, best quiz scores, simulation results, mastery per skill |
| `cards.json` | spaced-repetition state |
| `research/` | Stack Exchange material with source links (CC BY-SA attribution) |
| `.guru/download.log` | every step of the download, for diagnosis |

Goal roadmaps live in `goals/<slug>.json`: the goal, the learner's answers, destination,
standards, tools, safety rules, real-world requirements and stages. A stage's course
stores the roadmap context (`goal_context`), so every lesson and simulation knows the
standards in use and what came before and comes after.

## Simulation spec

Defined in `core/src/sim.rs` and played by `ui/js/sim/engine.js`. It is declarative: no
generated code ever runs.

- `world`: nodes (kind, status, map position) and links.
- `hosts`: per-node terminal with `files`, `processes`, `services` and `commands`
  (regex → output + effects) for anything beyond the built-in shell.
- `logs`: timestamped, levelled log streams.
- `apps`: simulated tools made of widgets: `markdown`, `stat`, `timeseries`, `table`,
  `logs`, `query`, `form`, `actions`, `editor`, `chat`.
- `workbench` widget for physical work: `tools` (instruments), `items` placed on a
  bench, each with `readings` per tool and `actions`, plus `links` (wires, pipes). The
  first reading whose `requires` flags are all set is shown, so measurements follow the
  state of the system.
- `actions` and buttons may `require` flags; without them they are refused with a
  `blocked_message`. This is how safe order of work (isolate → lock off → prove dead →
  work) is taught.
- `effects`: set and clear flags, change node or item status, show a message.
- `objectives`: `answer` (multiple choice or regex-checked free text) or `state` (flags
  that the learner's actions must set). Once achieved, an objective stays achieved.
  "Show me how" can close any objective without credit, so there is never a dead end.

Built-in actions set flags automatically (`killed:<pid|name>`, `stopped:<svc>`,
`removed:<path>`, `read:<path>`, `viewed:<stream>`, `inspected:<item>`, …). The validator checks that every
state objective's flags and every action requirement can be produced by some built-in
action or effect, that blocked actions explain themselves, that instruments exist, that every
referenced node and stream exists, that table rows and series lengths line up, and that
every regex compiles. The demo simulations are also checked against the JSON Schema
shape in `core/src/schemas.rs`, so the hand-written reference and what Codex produces
cannot drift apart.

## The Codex contract

| Need | Codex feature |
|---|---|
| AI on the learner's ChatGPT plan | `codex login` (Sign in with ChatGPT), `codex login status` |
| Structured assets | `codex exec --output-schema` (strict; widgets via `anyOf`) + `-o` |
| Fallback when a schema is rejected | the same schema embedded in the prompt, then Guru's own validation |
| Progress display | `codex exec --json` events (web search, commands, reasoning) |
| Safety | `--sandbox read-only`: generation only returns JSON; Codex writes no files |
| Current facts | `-c web_search="live"` for planning and lessons |
| No clutter | `--ephemeral` |

## AI budget

| Step | Codex requests |
|---|---|
| Goal: clarifying questions | 1, low reasoning effort |
| Goal: roadmap | 1 (retried once) |
| Sizing a topic (Quick course level bar) | 1, low reasoning effort |
| Downloading a course (or stage) | 1 plan + 1 per module + 1 per simulation, plus revisions when a quality gate or the validator rejects something |
| Studying | 0 |
| Optional, user-initiated | "Ask Guru" on a lesson: 1 · "Ask AI to simulate this command": 1 |

A 5-hour course (7 modules, about 12 simulations) is roughly 20 requests, run 1-3 at a
time depending on free memory, and resumable.

## Reliability

| Failure | What Guru does |
|---|---|
| Codex missing, not signed in, disk nearly full | Refused before anything is created, with the fix |
| Request hangs | Killed at the time limit (`GURU_CODEX_TIMEOUT_SECS`, default 20 min), retried |
| Network drop, server error, bad output | Up to 4 attempts per asset, waits of 5 s / 20 s / 60 s |
| ChatGPT usage limit | Waits twice, then pauses the download with a message; Resume later |
| Sign-in expired | Stops immediately with instructions; nothing is lost |
| Thin or unplayable asset | Sent back to Codex with the exact problems; unplayable is never accepted |
| One asset fails for good | Everything else is still built and usable; Resume retries just that asset |
| App closed or crashed mid-download | Marked "Interrupted" on next start; Resume continues |

## Security and safety

- AI-generated content is rendered as text nodes only. The Markdown renderer builds DOM
  elements and never uses `innerHTML`. The content security policy allows only Guru's
  own scripts.
- Codex runs with a read-only sandbox for every Guru request.
- Course and asset ids are validated before they touch the filesystem; deletion is
  restricted to course folders.
- Links in lessons open in the system browser, and only `http(s)` links are allowed.
- The improvised-terminal request strips objectives, hints and the debrief from the
  context it sends, so the AI cannot leak the answers.

## Performance

- The app binary is about 5.5 MB (release, Linux). Measured on Linux while playing a
  simulation, the app's total proportional memory (PSS) was about 220 MB across the
  Guru process and WebKit's two helpers. Guru disables WebKitGTK GPU compositing by
  default, which cut the page process from about 250 MB to about 90 MB in measurement.
  Set `WEBKIT_DISABLE_COMPOSITING_MODE=0` to opt back in.
- No background work unless a download is running. Downloads use a worker pool of 1-3
  Codex processes, sized to free RAM, or the "Parallel builders" setting.

## Roadmap

- Voice roleplay for spoken languages (Codex realtime).
- FSRS scheduling once there is enough review history.
- Shareable course packages (export/import a folder or a single `.guru` file).
- Optional "real lab" mode that provisions a container when the machine has room.
