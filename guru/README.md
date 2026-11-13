# Guru

**Learn any skill in hours, not weeks, by practising on simulations of the real thing.**

Guru is a desktop app. You tell it what you want to learn and drag a bar to how far you
want to get, from beginner through intermediate to advanced. It suggests how many hours
that takes, then you click **Download course**. AI builds the whole course once: tight
reading-first lessons, quizzes, flashcards, and **interactive simulations** of the tools
and situations you would face at work. Then you study it like a downloaded video course,
except there are no videos, no installs, no tool accounts, and no AI needed from that
point on.

```
 ┌ Guru ───────────────────────────────────────────────────────────────────────────────┐
 │  What do you want to learn?   [ DevOps                                        ]      │
 │                                                                                      │
 │  Intermediate                                                       −   5 h   +      │
 │  Work independently on typical real-world tasks and fix the common failures.         │
 │  ●━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━○──────────────────────────          │
 │  Beginner                     Intermediate                         Advanced          │
 │  7 modules · about 12 simulations · e.g. 1 hour a day for 5 days                     │
 │  Simulated inside Guru: Linux terminal · Grafana · CI/CD pipeline                    │
 │                                                                                      │
 │  [ ⬇ Download course ]                                                                │
 └──────────────────────────────────────────────────────────────────────────────────────┘
```

## Simulations instead of installs

Many people never practise because they can't: the tool is paid, needs an account, needs
a cluster, or their laptop can't run it. Guru closes that gap. Each simulation is a
playable world, written by AI when you download the course and played back offline by
Guru's built-in engine:

- **A visual system map.** Servers, databases, load balancers and services, each with a
  live status. A compromised host pulses red. Click a machine to work on it.
- **Terminals** with a real-feeling shell: files, processes and services, plus `ls`,
  `cat`, `grep`, `awk`, `sort | uniq -c`, `ps`, `top`, `kill`, `systemctl`, `journalctl`,
  `rm`, pipes, redirection, history and tab completion. Tool-specific commands such as
  `ss -tnp`, `iptables`, `kubectl`, `docker` or `crontab` are scripted per scenario.
- **Log explorer** with search, regex and level filters.
- **Simulated tools.** Dashboards with live-looking charts, query editors (PromQL, SQL,
  LogQL…) that answer real queries and return real error messages, configuration forms,
  file editors with checks, and chat counterparts for interviews, negotiations or
  language practice. Grafana, Kubernetes, Jenkins, a SQL console or a Spanish waiter are
  all recreated this way.
- **Objectives** that check what you found (multiple choice or free text) and what you
  did (process killed, persistence removed, alert rule created). Graduated hints and a
  debrief explain how a professional would handle it.

When you type something a scenario did not anticipate, you can optionally press **✦ Ask
AI to simulate this command**. That is one Codex request; nothing else in a simulation
uses AI.

The built-in demo course (**Try the demo course**, no AI or sign-in needed) contains two
full simulations: *3 a.m. page: web-01 is burning CPU* (a crypto-miner incident after an
SSH brute force) and *Build the on-call dashboard and alert in Grafana* (data source,
PromQL, alert rule).

## How a course is built (the only AI-heavy step)

Guru runs on top of the [Codex CLI](../README.md) and uses your **Sign in with ChatGPT**
account (Settings → *Sign in with ChatGPT*).

1. **Size the topic.** One quick request: anchors for beginner / intermediate /
   advanced, with hours and what you can do at each. After that, moving the bar is
   instant arithmetic.
2. **Plan.** Skills ranked by real-world value, modules of about 45 minutes, one or two
   simulations per module, and a capstone scenario.
3. **Research.** The most up-voted Stack Overflow / Stack Exchange questions for the topic
   become the real-world failures that lessons warn about and simulations recreate.
4. **Build.** Lessons, quizzes, flashcards and simulations, in parallel workers sized to
   your free memory. Module 1 comes first, so **you can start studying while the rest
   downloads**. Every simulation is checked by a validator (every objective must be
   achievable, every reference must exist, every regex must compile). If a check fails,
   the exact problems go back to Codex to fix.
5. **Done.** The course is a folder of JSON files. Pause and resume any time; only
   missing pieces are rebuilt.

## Studying

Each module is **Lesson → Quiz → Simulation(s)**, with spaced-repetition flashcards
across the course and a capstone at the end. *Continue* always takes you to the next
step. Progress, mastery per skill and study time are tracked locally. The method and the
research behind it are in [docs/METHOD.md](docs/METHOD.md).

## Light on your machine

Guru uses the operating system's own web view (WebKit on macOS and Linux, WebView2 on
Windows) instead of shipping a copy of Chromium, so the app binary is about **5.5 MB**.
Measured on Linux while playing a simulation, the whole app used about **220 MB** of RAM.
That figure is the proportional set size across Guru's process and WebKit's two helper
processes, with GPU compositing off, which Guru does by default on Linux. Nothing runs in
the background when no course is downloading, and simulations need no containers, VMs or
installs.

## Install and run

Requirements: [Rust](https://rustup.rs), the [Codex CLI](https://developers.openai.com/codex)
(`npm i -g @openai/codex`) for building courses, and on Linux the WebKitGTK development
packages:

```bash
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev   # Debian/Ubuntu only

cd guru
cargo run --release -p guru-app      # starts the desktop app
```

For installers (`.dmg`, `.msi`, `.deb`, `.AppImage`), use the Tauri CLI:
`cargo install tauri-cli --version "^2" && cd app && cargo tauri build`.

To preview the interface in a browser without Rust, serve `ui/` (for example
`python3 -m http.server -d ui`) and open it. It runs against a mock backend that serves
the demo course.

## Project layout

```
guru/
  core/   guru-core: course package, simulation spec + validator, level bar,
          download pipeline (drives `codex exec`), spaced repetition, Stack Exchange mining
  app/    guru-app: the Tauri desktop shell (commands + events)
  ui/     the interface: plain ES modules, no framework, no build step
          ui/js/sim/engine.js plays simulations; ui/demo/ is the built-in demo course
  tools/  generators for the demo course and the app icon
```

## Tests

```bash
cargo test --workspace                     # Rust: unit tests + full download pipeline against a fake Codex
node --test ui/js/sim/engine.test.mjs      # simulation engine: both demo scenarios solved end to end
cargo clippy --workspace --all-targets -- -D warnings
```

Architecture, the Codex contract and the simulation spec: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).
