# Guru architecture

Guru is a small Rust program that **orchestrates the Codex CLI**. Codex does three jobs:
it is the AI, it is the system controller that builds practice environments on the
learner's machine inside its sandbox, and it is the live tutor. Guru provides the
pedagogy, the learner model, scheduling, machine awareness and the session flow.

```
                ┌──────────────────────────── guru (≈1.2 MB binary, ≈3 MB RSS idle) ───────────────────────────┐
  learner ⇄ CLI │ learn.rs   intake → plan → pretest → calibrate      session.rs  Recall→Read→Practice→Prove→Reflect │
                │ srs.rs     SM-2 spaced, interleaved retrieval        machine.rs  RAM/CPU/disk/tools → lab budget   │
                │ prompts.rs the method, as instructions               schemas.rs  strict JSON output contracts      │
                │ research.rs Stack Exchange mining (system curl)      config.rs   ~/.guru JSON store (atomic writes) │
                └───────────────┬──────────────────────────────────────────────┬──────────────────────────────────┘
                                │ codex exec --json --output-schema … -        │ codex -C <course> "<prompt>"
                                ▼                                              ▼
                     non-interactive turns                        interactive TUI tutor (AGENTS.md
                     (plan, lesson+lab, check, grade)             = learner model + teaching rules)
                                │
                     Sign in with ChatGPT (codex login) · sandbox (read-only / workspace-write) · web search
```

## The Codex contract

Guru depends only on Codex's stable command-line surface, so it upgrades with Codex:

| Need | Codex feature |
|---|---|
| AI with the learner's ChatGPT plan | `codex login` (Sign in with ChatGPT); `codex login status` |
| Structured results | `codex exec --output-schema <file>` + `-o <file>`; strict schemas in `schemas.rs` |
| Progress display | `codex exec --json` event stream (`item.started` command/file/web-search) |
| Building labs safely | `--sandbox workspace-write -C <course dir>`: Codex can only write inside the course |
| Grading without side effects | `--sandbox read-only` |
| Current facts and versions | `-c web_search="live"` (planning, lessons, research); disabled for grading |
| Live tutoring | interactive `codex -C <course>`; the course `AGENTS.md` is the tutor's instructions |
| No session clutter | `--ephemeral` for one-shot turns |

Set `GURU_CODEX_BIN` (or `guru config codex_bin`) to use a Codex built from this repository.

## AI cost per hour

The token budget is kept small on purpose:

- **1 turn** to prepare the hour (lesson, lab, cards, quiz, hints, all in one structured
  call). It runs in the background during the Recall block, so the learner never waits for it.
- **0..n turns** for help, only when the learner asks. Prepared hints and the automatic
  check script cost nothing.
- **1 turn** for the lab review (only if the check script cannot judge the work, or the
  learner asks for feedback).
- **1 turn** for grading the quiz and explanation together.

Course creation costs 2 turns (plan; calibration plus issue bank). Spaced review, status
and the hint ladder are fully local.

## Learner model

`course.json` holds the plan, the per-skill mastery estimates, misconceptions, remediation
targets and coach notes. `Course::learner_model()` serialises it compactly and every prompt
includes it. That is what makes every lesson, lab, hint and grade adaptive. `AGENTS.md` is
regenerated after each hour so the interactive tutor sees the same model.

## Safety model

- Codex writes only inside the course directory (workspace-write sandbox) and grades
  read-only.
- Paths Codex returns are validated to stay inside the course directory.
- Every script Codex writes (setup, check, teardown) runs with the learner's own permissions,
  outside the sandbox, because setup may need the network to download packages. So each
  script is shown and needs one-time approval tied to its exact contents: any change asks
  again. Check and teardown scripts run with a timeout, and their output is truncated
  before it is sent to the model.
- Lab budgets are enforced through instructions plus the visible RAM/disk estimate the
  learner confirms; teardown is offered at the end of every hour and `guru clean` stops
  everything.
- All learner data is local JSON. Nothing leaves the machine except Codex turns, and the
  Stack Exchange API queries (topic tags only).

## Resilience

Progress inside an hour (`progress.json`) is saved after every step, so a closed laptop
or Ctrl-C resumes exactly where it stopped. All writes are atomic (write + rename).

## Roadmap

- Use the Codex app-server protocol for streaming lessons token-by-token.
- Run check scripts inside `codex sandbox` instead of with plain user permissions.
- Replace SM-2 with FSRS once there is enough review history per learner.
- Voice roleplay for spoken languages via Codex realtime.
- Shareable course templates: the same plan, re-calibrated for each learner.
