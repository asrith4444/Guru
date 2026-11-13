# Guru

**Learn any skill in hours, not weeks, by practising on simulations of the real thing.**

Guru is a desktop app. It starts with one question: **What's your goal?** Anything: a trade
(plumbing, electrical work, HVAC), a profession (DevOps, chip design, nursing, law), a tool,
an exam, a language. Guru asks the 3–5 questions a senior practitioner would ask (your
experience, the purpose, the country and its codes, the specialisation, your tools). It then
builds a **professional roadmap** with:

- the destination, described the way the profession describes it;
- the standards and codes it follows (BS 7671, NEC, IEEE 1800…);
- the tools of the trade;
- the safety rules;
- honest notes on what still needs the real world (licences, exams, supervised hours);
- 3–8 **stages**, each with measurable outcomes, hands-on simulations and an assessment.

Each stage is one click: **Download this stage**. AI builds a complete course once:
reading-first lessons, quizzes, flashcards and **interactive simulations** of the tools and
situations you would face at work. Then you study it like a downloaded video course. There
are no videos, no installs, no tool accounts, and no AI needed from that point on.

For a single topic there is also a **Quick course**: type the topic, drag a bar from
beginner through intermediate to advanced, see the suggested hours, and download.

```
 What's your goal?  [ Become an electrician's mate on UK homes                 ]  Plan my path →
   → Experience?  None · DIY · Apprentice      → Which regulations?  UK (BS 7671) · US (NEC)
   → Roadmap: 1 Safe isolation (2 h) → 2 Fault finding (3 h) → 3 Testing & inspection (4 h)
              Standards: BS 7671, GS38 · Safety rules · What still needs the real world
              [ ⬇ Download this stage ]
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
- **Workbenches for physical work.** Consumer units, plumbing systems, HVAC units, engines
  or circuit boards. You pick a real instrument (two-pole tester, insulation resistance
  tester, pressure gauge, oscilloscope…), click a component, and get realistic readings
  with real units that change as you work: 230 V before isolation, 0 V after. Unsafe
  actions are **refused and explained**, for example opening a socket before you have
  locked off the circuit and proved it dead with a proved tester.
- **Objectives** that check what you found (multiple choice or free text) and what you
  did (process killed, persistence removed, alert rule created, appliance withdrawn).
  Graduated hints and a debrief explain how a professional would handle it.
- **No dead ends.** After the hints run out, or after two wrong answers, **Show me how**
  reveals that objective's answer and method, so you can always finish. Revealed
  objectives earn no credit, and the debrief tells you which ones to replay.

When you type something a scenario did not anticipate, you can optionally press **✦ Ask
AI to simulate this command**. That is one Codex request; nothing else in a simulation
uses AI.

Two built-in demo courses work with no AI and no sign-in (links on the home screen):

- **Incident response:** *3 a.m. page: web-01 is burning CPU* (a crypto-miner after an SSH
  brute force) and *Build the on-call dashboard and alert in Grafana* (data source,
  PromQL, alert rule).
- **Electrical fault-finding:** *Kitchen sockets dead: find the fault, safely*. This is a
  30 mA RCD tripped by a water-damaged kettle. You use a two-pole tester, a proving unit
  and a 500 V insulation resistance tester, and safe isolation is enforced (BS 7671 /
  GS38, with notes on the North American GFCI and NEC equivalents).

## Built not to fail half-way, and to be worth your time

- **Checked before it starts.** Codex installed, signed in, enough disk; otherwise Guru
  says exactly what to fix *before* creating anything.
- **Every AI request has a time limit** (20 minutes by default). A hung request is killed
  and retried.
- **Every asset is retried** up to 4 times with growing waits after dropped connections or
  bad output.
- **Usage limits pause rather than fail.** When your ChatGPT usage limit is hit, Guru
  waits, then pauses the download with a clear message; press Resume later.
- **A lost sign-in stops at once** with instructions. Retrying could not help, so Guru
  doesn't waste attempts.
- **Nothing built is ever lost.** Every finished lesson and simulation is a file, so
  Resume only builds what's missing. A download interrupted by a crash or shutdown shows
  up as "Interrupted, press Resume", never as stuck.
- **Quality gates.** Thin lessons (too short, missing the required structure, too few
  questions or flashcards) and thin simulations (too few objectives, nothing to *do*,
  sparse logs, a short debrief) are sent back to Codex with the exact problems.
  Unplayable simulations are never accepted.
- **Fully logged.** Every step is written to `.guru/download.log` inside the course.


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
Measured on Linux while playing a simulation, the whole app used about **215–220 MB** of RAM.
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
  core/   guru-core: goal roadmaps, course package, simulation spec + validator, level bar,
          download pipeline (drives `codex exec`), spaced repetition, Stack Exchange mining
  app/    guru-app: the Tauri desktop shell (commands + events)
  ui/     the interface: plain ES modules, no framework, no build step
          ui/js/sim/engine.js plays simulations; ui/demo/ holds the built-in demo courses
  tools/  generators for the demo course and the app icon
```

## Tests

```bash
cargo test --workspace                     # Rust: unit tests + goal → roadmap → stage download against a fake
                                           # Codex that drops a connection, hangs, and returns thin and broken assets
node --test ui/js/sim/engine.test.mjs      # simulation engine: all three demo scenarios solved end to end
cargo clippy --workspace --all-targets -- -D warnings
```

Architecture, the Codex contract and the simulation spec: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).
