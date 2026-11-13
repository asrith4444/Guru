# Guru

**Learn any skill to working competence in about seven focused hours.** Read, practice,
recall: one hour a day for a week. No videos, and no roadmap that loses you halfway.

Guru runs on top of the [Codex CLI](../README.md). Codex is the AI (through your
ChatGPT sign-in), the system controller that builds practice environments on your machine,
and your live tutor. Guru brings the learning science, your learner model, the schedule,
and respect for your hardware.

```
$ guru learn "Spring Boot"
What do you want to be able to DO after this course? › build and ship REST APIs at work
What do you already know that is related?            › Java 17, some Express.js
✓ Machine: 8 CPU threads, 9800 MB RAM free → lab budget 4900 MB RAM, 10240 MB disk (Large tier, containers ok).
Deconstructing the skill…
━━ Spring Boot: Production REST APIs in 7 Hours
Target: build, test and containerise a CRUD REST API with validation and JPA, and diagnose
        the 10 most common startup/configuration failures
  Hour 1 Auto-configuration, starters and your first endpoint
  …
▶ Pretest · ~5 min
…
✓ Problem bank: 22 real-world issues.
```

## How a week looks

```
guru learn "<topic>"   # once (~10 min): goal → target performance → pretest → calibrated plan + real-world issue bank
guru next              # each day: one 60-minute session (resumable)
guru review            # any time: 5 minutes of spaced recall, fully offline
guru status            # mastery per skill, calibration, what's being repaired
guru report            # after hour 7: honest report + the next 7-hour plan
```

Each session is the **Guru Loop**:

| Block | | |
|---|---|---|
| Recall | 5 min | spaced, interleaved flashcards from earlier hours (Codex prepares today's lesson and lab in the background meanwhile) |
| Read | 15 min | predict first, then a tight lesson: mental model, worked example, the 20% that matters, how it breaks (with real Stack Overflow cases), cheat sheet |
| Practice | 25 min | a lab sized to your machine: completion task → independent task → break-fix of a real-world failure → capstone slice |
| Prove | 10 min | lab check, exit quiz with confidence ratings, explain-it-simply |
| Reflect | 5 min | scores, calibration, mastery gate, cleanup, next |

The research behind each block is in [docs/METHOD.md](docs/METHOD.md).

## Not only code

The planner chooses the practice arena that fits the topic:

- **Spring Boot, Rust, React…** → `code_lab`: a real project with TODOs and automated checks.
- **DevOps, Kubernetes, databases, networking** → `ops_lab`: broken systems with real faults,
  diagnosed from logs. Simulated when your machine is too small to run them.
- **Spanish, Japanese, negotiation, interviews** → `roleplay`: live scenarios with the tutor
  as your counterpart, corrected after every turn.
- **Statistics, physics, finance** → `problem_set`. **Guitar, drawing, typing** → `drill_protocol`.
  **Management, law, medicine** → `case_study`.

## Practice commands (during the session)

`h` hint ladder (nudge → pointer → near-solution, free) · `a` AI hint that reads your actual
files · `c` run checks · `t` live tutor (interactive Codex) · `r` re-read · `d` done · `q` pause.

Outside a session: `guru ask "…"`, `guru tutor`, `guru check`, `guru incident` (a fresh
break-fix drill built from the issue bank), `guru plan`, `guru clean [--deep]`.

## Light by design

Guru is a ~1.2 MB binary that uses about 3 MB of RAM while it waits for you. It runs no
daemon. Before each session it measures free RAM, CPU, disk (respecting container/cgroup
limits) and installed tools. From those it derives a **hard lab budget**: at most half of
the free RAM and never more than 40% of total RAM, plus a disk cap, and a decision on
whether containers or a single-node local cluster are allowed. Every environment Codex
builds must fit that budget and ship a teardown script, and Guru offers to free everything
at the end of the hour.

```
$ guru doctor
✓ Codex CLI: codex-cli 0.x
✓ Auth: Logged in using ChatGPT
✓ Machine: linux x86_64, 8 threads, RAM 16000 MB total / 9800 MB free, disk free 120000 MB
✓ Lab budget: Large tier · ≤4900 MB RAM · ≤10240 MB disk · ≤4 CPU · containers yes · local cluster no
```

## Install

Requirements: the Codex CLI (`npm i -g @openai/codex`, `brew install --cask codex`, or a
build from this repository) and a Rust toolchain to build Guru. `curl` is optional and
enables Stack Exchange mining.

```bash
cd guru
cargo install --path .
guru login      # Sign in with ChatGPT (runs `codex login`)
guru doctor
guru learn "Spring Boot"
```

To use a Codex built from this repo: `guru config codex_bin /path/to/codex`.

## Configuration

`guru config` shows the current configuration; `guru config <key> <value>` sets a key.

| key | default | |
|---|---|---|
| `codex_bin` | `codex` on PATH | the Codex binary (env `GURU_CODEX_BIN` wins) |
| `model` | Codex default | model passed to Codex |
| `reasoning_effort` | Codex default | e.g. `medium`, `high` |
| `web_search` | `true` | let Codex search the live web while planning and writing lessons |
| `stackexchange` | `true` | mine Stack Exchange for real-world problems |
| `stackexchange_key` | none | optional API key for a higher daily quota |
| `hours` / `minutes_per_hour` | `7` / `60` | default course shape (`guru learn --hours --minutes` overrides) |

Environment: `GURU_HOME` (data directory, default `~/.guru`), `GURU_READER` or `PAGER`
(how lessons are shown; default `less -R`), `NO_COLOR`.

## Your data

Everything is plain files in `~/.guru/courses/<topic>/`: `course.json` (plan + learner
model), `cards.json`, `issues.json` (with source links; Stack Exchange content is CC BY-SA),
`journal.jsonl`, `lessons/`, `lab/`, `capstone/`, and `AGENTS.md`, which turns interactive
Codex into your tutor. When git is installed, the course directory is a git repository, so you can diff your
own progress.

## Development

```bash
cargo test        # unit tests + an end-to-end run against a fake Codex (tests/fake_codex.sh)
cargo clippy --all-targets -- -D warnings
```

Architecture and the Codex contract: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).
