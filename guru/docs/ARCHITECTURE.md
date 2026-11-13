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
 │ generator  plan → research → build (parallel) → validate → ready     │
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

## Simulation spec

Defined in `core/src/sim.rs` and played by `ui/js/sim/engine.js`. It is declarative: no
generated code ever runs.

- `world`: nodes (kind, status, map position) and links.
- `hosts`: per-node terminal with `files`, `processes`, `services` and `commands`
  (regex → output + effects) for anything beyond the built-in shell.
- `logs`: timestamped, levelled log streams.
- `apps`: simulated tools made of widgets: `markdown`, `stat`, `timeseries`, `table`,
  `logs`, `query`, `form`, `actions`, `editor`, `chat`.
- `effects`: set flags, change node status, show a message.
- `objectives`: `answer` (multiple choice or regex-checked free text) or `state` (flags
  that the learner's actions must set).

Built-in actions set flags automatically (`killed:<pid|name>`, `stopped:<svc>`,
`removed:<path>`, `read:<path>`, `viewed:<stream>`, …). The validator checks that every
state objective's flags can be produced by some built-in action or effect, that every
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
| Sizing a topic (level bar) | 1, low reasoning effort |
| Downloading a course | 1 plan + 1 per module + 1 per simulation (+1 retry when validation fails) |
| Studying | 0 |
| Optional, user-initiated | "Ask Guru" on a lesson: 1 · "Ask AI to simulate this command": 1 |

A 5-hour course (7 modules, about 12 simulations) is roughly 20 requests, run 1-3 at a
time depending on free memory, and resumable.

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
