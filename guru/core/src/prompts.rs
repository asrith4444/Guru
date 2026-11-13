//! Every instruction Guru gives Codex. The learning science and the quality bar
//! for simulations live here, so every downloaded course gets them.
//! See docs/METHOD.md for the research behind the rules.

use crate::course::Course;
use crate::course::ModuleInfo;
use crate::course::SimPlan;

pub const METHOD: &str = r#"You are the course author behind Guru, a desktop app that turns any topic into a short,
intense, offline course: reading-first lessons, quizzes, flashcards and interactive SIMULATIONS that
recreate real tools and real incidents, so learners practise without installing anything or owning
an account. The whole course is generated now; afterwards it is studied with no AI at all, so
everything must be complete, correct and self-contained.

Principles (apply them, do not lecture about them):
1. Pace aggressively. Focused learners with good material move far faster than traditional courses
   assume: what is usually sold as "two weeks" fits into 2-4 hours here. Cut filler, history,
   motivational text and repetition. Every paragraph must earn its place.
2. Pareto: teach the 20% of concepts, commands, workflows and failure modes that cover 80% of real
   use. State what is out of scope.
3. Performance over topics: every module ends with something the learner can DO in a simulation.
4. Worked examples first, then practice with less and less support.
5. Retrieval and spacing: flashcards and quizzes demand recall, mix in earlier material.
6. Real-world problems: use the genuine failures practitioners hit (Stack Overflow, incident write-ups,
   issue trackers) as practice. Cite sources in lessons.
7. Accuracy: current stable versions, real command syntax, real tool concepts and terminology.
   Never invent APIs, flags or facts. Use web search when available to verify.
8. Professional standard, any field: teach the way the profession itself trains and assesses people.
   Use the field's real competency frameworks, codes and standards (for example NEC / IEC 60364 / BS 7671
   for electrical work, IPC/UPC plumbing codes, IEEE 1800 SystemVerilog and sign-off flows for chip design,
   ICAO for aviation), its real tools and instruments with realistic readings and units, its real job tasks,
   and its vocabulary. Where rules differ by country or jurisdiction, say which one you use and how others differ.
   Be specific: numbers, tolerances, part names, commands, procedures. Never vague advice.
9. Safety first where the work can hurt people or property (electricity, gas, heights, chemicals, machinery,
   medicine, structural work): teach the safe order of work (isolate, lock out, prove dead / depressurise,
   PPE), make simulations refuse unsafe actions and explain the consequence, and state plainly which tasks
   legally require a licence, certification or supervised hours that a course cannot replace.
"#;

fn outline(course: &Course) -> String {
    let mut s = format!(
        "Course: {}\nTopic: {}\nLearner goal: {}\nLevel: {} ({}/100) in {} hours\nTarget performance: {}\nSkills: {}\nModules:\n",
        course.title,
        course.topic,
        course.goal,
        course.level_label,
        course.level,
        course.hours,
        course.target_performance,
        course
            .skills
            .iter()
            .map(|s| format!("{} ({})", s.id, s.name))
            .collect::<Vec<_>>()
            .join(", ")
    );
    if !course.goal_context.trim().is_empty() {
        s.push_str(&format!(
            "This course is one stage of a larger roadmap:\n{}\n",
            course.goal_context
        ));
    }
    for m in &course.modules {
        s.push_str(&format!(
            "- {} {} ({} min): {}\n",
            m.id,
            m.title,
            m.minutes,
            m.objectives.join("; ")
        ));
    }
    s
}

pub fn scope(topic: &str, goal: &str) -> String {
    format!(
        r#"{METHOD}
TASK: size this topic for Guru's level bar (beginner → intermediate → advanced).

Topic: {topic}
Learner goal: {goal}

For each level give the focused study hours Guru needs (in 0.5 h steps) and one sentence of what the
learner can DO at that level. Calibrate aggressively:
- a single tool (Grafana, Git, Docker, Postman, Excel pivot tables): beginner 0.5-1, intermediate 1.5-3, advanced 4-6
- a framework or language (Spring Boot, React, Python, SQL): beginner 1-1.5, intermediate 2.5-4, advanced 6-8
- a broad discipline (DevOps, cloud architecture, cybersecurity incident response, data engineering,
  conversational Spanish): beginner 1.5-2.5, intermediate 4-7, advanced 9-14
Also: breadth 1-10, the real tools whose experience Guru should SIMULATE in the app, a short course
title, and a one-line note.
"#
    )
}

/// Plan prompt for a freshly created course (topic, goal, level and hours set).
pub fn plan(course: &Course, can_do: &str, modules: usize) -> String {
    let (topic, goal, level, level_label, hours) = (
        &course.topic,
        &course.goal,
        course.level,
        &course.level_label,
        course.hours,
    );
    let goal_context = &course.goal_context;
    let goal_context = if goal_context.trim().is_empty() {
        String::new()
    } else {
        format!("\nThis course is one stage of the learner's roadmap. Stay inside this stage, build on earlier stages and\nprepare for later ones:\n{goal_context}\n")
    };
    format!(
        r#"{METHOD}
TASK: plan a {hours}-hour Guru course with exactly {modules} modules.
{goal_context}
Topic: {topic}
Learner goal: {goal}
Target level: {level_label} ({level}/100). At this level the learner can: {can_do}

Return:
- title, a 2-3 sentence summary, target_performance (one concrete, checkable description of what the
  learner can do at the end), out_of_scope.
- skills: 6-14 sub-skills (snake_case ids), weight 1-10 by real-world value.
- modules: exactly {modules}, ids m1..m{modules}, minutes summing to about {total} minutes, 2-4 objectives each,
  skill_ids, and 1-2 simulations per module (ids like m1s1). Simulations are the heart of Guru:
  * "tool": recreate the real tool's UI and workflow (e.g. Grafana dashboards, PromQL explore, alert rules;
    Kubernetes via kubectl; Jenkins pipelines; Excel-like tables) so the learner practises without installing it;
  * "incident": a broken or compromised system to investigate from logs, terminals and dashboards, then fix;
  * "guided": a step-by-step build or configuration task with checks;
  * "roleplay": a conversation (customer, interviewer, native speaker, stakeholder) for language and soft skills;
  * "workbench": hands-on physical work on a simulated bench: an electrical panel, a plumbing system, an HVAC
    unit, an engine, a circuit board. Learners pick instruments (multimeter, pressure gauge, oscilloscope…),
    read realistic measurements, and act in a safe order (unsafe actions are refused and explained).
  Pick what gives the most realistic practice for this topic. Premises must be specific and realistic.
- capstone: one final simulation (id "capstone") that integrates the whole course under realistic pressure.
- research: the best Stack Exchange site for real problems in this topic ("" if none), 1-3 tags, 2-4 queries.
"#,
        total = (hours * 60.0).round() as u32,
    )
}

pub fn module(course: &Course, m: &ModuleInfo, issues: &str) -> String {
    let sims: Vec<String> = m
        .sims
        .iter()
        .map(|s| format!("{} ({}, {}): {}", s.title, s.kind, s.tool, s.premise))
        .collect();
    format!(
        r#"{METHOD}
TASK: write module {id} "{title}" ({minutes} min) of this course.

{outline}
This module: {summary}
Objectives: {objectives}
Skills: {skills}
Simulations that follow this lesson (the lesson must prepare the learner for them): {sims}

Real-world problems collected for this topic (votes = how many people hit them):
{issues}

Return:
- lesson: Markdown, reading time about {read} minutes, dense and concrete. Structure:
  ## Why this matters (2-3 lines)  ## The mental model (plain words; an ASCII diagram if useful)
  ## Worked example (step by step, real commands/config/UI steps, with "> Pause and predict:" prompts)
  ## The 20% you will use constantly  ## How it breaks (2-4 real failures: symptom → cause → fix, cite links)
  ## Don't confuse it with (contrast with the similar concept)  ## Cheat sheet (compact table or list)
  Use fenced code blocks with a language tag. No HTML.
- key_points: 4-6 one-line takeaways.
- cards: 6-10 flashcards; front is a question that demands recall (not a cloze of the lesson),
  back is short and exact. Include 1-2 cards on earlier modules' skills when this is not module m1.
- quiz: 5-7 questions: mostly "single" (4 plausible options, misconceptions as distractors),
  some "multi", and 1-2 "text" (short exact answers like a command, metric or term, with accept regexes
  covering reasonable variants, case-insensitive). Every question has an explanation of WHY.
"#,
        id = m.id,
        title = m.title,
        minutes = m.minutes,
        outline = outline(course),
        summary = m.summary,
        objectives = m.objectives.join("; "),
        skills = m.skill_ids.join(", "),
        sims = if sims.is_empty() {
            "none".into()
        } else {
            sims.join(" | ")
        },
        read = (m.minutes / 3).clamp(5, 15),
    )
}

pub const SIM_SPEC: &str = r#"HOW GURU PLAYS A SIMULATION (design for this engine; it runs offline, no AI):
- world: nodes drawn on a map (x,y 0-100; spread them out, left-to-right by traffic flow) with a status
  colour; links between them. Clicking a node opens its terminal, logs and apps.
- hosts: a terminal per node with a virtual filesystem (files), processes and services. Built-in commands
  work automatically on that data: ls, cd, pwd, cat, head, tail, grep (-i -v -n -c -r -E), find -name, wc -l,
  sort, uniq -c, pipes (|), ps, top, kill, systemctl status/start/stop/restart/enable/disable, journalctl -u,
  rm, whoami, hostname, uname, echo, history, clear. Anything else (docker, kubectl, crontab, netstat/ss,
  iptables, curl, git, mysql, apt, the tool's own CLI...) needs a command rule: a regex over the whole command
  line plus its realistic output. Write rules for every command a competent person would reasonably try,
  including common variants (e.g. "^(sudo )?netstat -[a-z]*p" and "^(sudo )?ss -[a-z]*p"). Rules are tried
  in order, before built-ins.
- logs: named streams with timestamped lines; the learner searches and filters them. Make them realistic:
  realistic timestamps in order, normal background noise, the signal hidden in it, one or two red herrings.
  Use documentation IP ranges (192.0.2.x, 198.51.100.x, 203.0.113.x) and example.com domains.
- apps: simulated tools built from widgets: markdown, stat, timeseries, table, logs (embeds a stream),
  query (an editor: regex-matched responses returning table/timeseries/text/error, plus a fallback error for
  anything else — make the fallback teach, e.g. the real error message), form (fields + rules on submitted
  values), actions (buttons), editor (the learner edits a file; checks are regexes that must / must not match;
  effects fire when all checks pass), chat (a counterpart who replies to learner messages matching `expect`),
  workbench (physical work: `tools` the learner can pick — instruments such as "Two-pole voltage tester",
  "Multimeter (Ω)", "Pressure gauge", "Thermal camera", "Oscilloscope" — and `items` placed at x,y 0-100 with a
  status (ok | fault | warning | off | on | unknown), `links` (wires, pipes, traces) between items, `readings`
  per tool, and `actions`). Readings are tried in order and the first whose `requires` flags are all set is
  shown, so measurements change as the learner works (e.g. 230 V before isolation, 0 V after). Use "Look" as
  the tool for visual inspection. Give readings real units and realistic values with tolerances.
- actions and buttons may list `requires` flags; if any is missing the action is refused and `blocked_message`
  is shown. Use this to enforce professional order of work and safety (isolate → lock off → prove dead →
  work), and make the blocked message say what would happen in real life.
  Recreate the real tool's screens, terminology, query language and workflow faithfully (name it like
  "Grafana (simulated)"; no logos or trademarks beyond the name).
- effects: set_flags, node_status changes and a toast message. Built-in actions set flags automatically:
  kill → "killed:<pid>" and "killed:<process name>"; systemctl stop/start/restart/disable/enable →
  "stopped:<svc>" etc.; rm → "removed:<path>" (also for any parent directory removed); cat/head/tail/grep on a
  file → "read:<path>"; opening a log stream → "viewed:<stream id>"; any reading on a workbench item →
  "inspected:<item id>". Effects' node_status may also target workbench items (to change their status).
- objectives: "answer" (a question with options+correct indices, or free text with accept regexes) or "state"
  (all listed flags must be set by the learner's actions). Every state flag MUST be producible by a built-in
  action or by an effect somewhere in the simulation. Mix finding (answer) and fixing (state) objectives.
- hints: 3 graduated hints (nudge, pointer, near-answer). debrief: Markdown explaining what really happened,
  how a professional would approach it, and how to prevent it.
Regexes: simple, case-insensitive, no lookaround or backreferences."#;

pub fn simulation(
    course: &Course,
    module: Option<&ModuleInfo>,
    plan: &SimPlan,
    issues: &str,
    minutes: u32,
) -> String {
    let context = match module {
        Some(m) => format!(
            "It follows module {} \"{}\" (objectives: {}; skills: {}).",
            m.id,
            m.title,
            m.objectives.join("; "),
            m.skill_ids.join(", ")
        ),
        None => "It is the CAPSTONE: integrate the most important skills of the whole course in one realistic, \
                 multi-step scenario with 5-8 objectives."
            .into(),
    };
    format!(
        r#"{METHOD}
TASK: author the simulation "{title}" (id "{id}", kind {kind}, simulating: {tool}), about {minutes} minutes of
hands-on practice.
Premise: {premise}
{context}

{outline}
Real-world problems collected for this topic:
{issues}

{SIM_SPEC}

Quality bar: it must feel like the real thing to a professional. Provide enough files, processes, log lines
(typically 40-150 per stream), dashboard data and command rules that exploring is rewarding and the answer must
be found, not guessed. 3-6 objectives (capstone: 5-8). skill_ids from the course skills. Keep the JSON under
about 60 KB.
"#,
        title = plan.title,
        id = plan.id,
        kind = plan.kind,
        tool = plan.tool,
        premise = plan.premise,
        outline = outline(course),
    )
}

pub fn fix_simulation(errors: &[String]) -> String {
    format!(
        "\n\nYOUR PREVIOUS ATTEMPT FAILED VALIDATION. Return the complete simulation again with these problems fixed:\n- {}\n",
        errors.join("\n- ")
    )
}

pub fn clarify(goal: &str) -> String {
    format!(
        r#"{METHOD}
TASK: a learner told Guru their goal. Before planning, ask the 3-5 questions a senior practitioner and training
manager in this field would ask to plan the right path. Any field is possible (software, trades like plumbing or
electrical work, chip design, medicine, law, languages, music, business…).

Goal: {goal}

Ask only what changes the plan, for example: current experience and adjacent skills; the purpose (job, licence or
certification, own home or project, curiosity); country or jurisdiction when codes and licensing differ; the
specialisation or context (e.g. residential vs industrial, analog vs digital, FPGA vs ASIC); the tools or
equipment they have; how deep they want to go. Each question: short, concrete, with a one-line "why" and 3-5
typical answers as options. Also restate the goal precisely and name the professional field.
"#
    )
}

pub fn goal_plan(goal: &str, answers: &[crate::goal::Answer]) -> String {
    let answers: String = answers
        .iter()
        .filter(|a| !a.answer.trim().is_empty())
        .map(|a| format!("- {}: {}\n", a.question, a.answer))
        .collect();
    format!(
        r#"{METHOD}
TASK: design a professional roadmap for this learner, the way an experienced practitioner and training manager
would design an apprenticeship or a structured upskilling programme.

Goal: {goal}
Learner's answers:
{answers}
Return:
- title, field (the professional domain, specific), destination: what the learner will be able to do at the end,
  described concretely as a professional would (tasks, quality bar, tools), not vaguely.
- standards: the competency frameworks, codes, standards and certifications this roadmap follows (name them
  exactly, with the jurisdiction when relevant).
- tools: the real tools, instruments and software of the trade (Guru will simulate them).
- safety: the field's safety-critical rules (empty if not applicable).
- beyond_guru: what honestly still needs the real world to reach the destination: licences, exams, supervised or
  logged hours, physical practice with real equipment. Be specific (which exam, how many hours where defined).
- stages: 3-8 stages in order, each one downloadable as an intense Guru course of 1-12 focused hours (aggressive
  pacing: no filler). For each: title, why it comes here, level 0-100 (non-decreasing: beginner ~25, intermediate
  ~60, advanced ~90), hours, 3-5 measurable outcomes ("can …"), competencies, the hands-on simulations it will
  contain (specific scenarios with the real tools), and how mastery is assessed at the end.
Use web search (when available) to confirm current standards, editions and licensing rules.
"#
    )
}

pub fn quality_feedback(problems: &[String]) -> String {
    format!(
        "\n\nYOUR PREVIOUS ATTEMPT WAS NOT GOOD ENOUGH TO BE WORTH THE LEARNER'S TIME. Return the complete result again, fixing:\n- {}\n",
        problems.join("\n- ")
    )
}

pub fn ask(course: &Course, context: &str, question: &str) -> String {
    format!(
        r#"{METHOD}
The learner is studying this course offline and asks a question. Answer like a great tutor: short, precise,
concrete, with a minimal example. If it is about a simulation they are solving, give a hint or a question back
rather than the answer, unless they explicitly ask for the answer.

{outline}
What they are looking at: {context}
Question: {question}
"#,
        outline = outline(course)
    )
}

pub fn simulate_command(sim_json: &str, hostname: &str, history: &str, command: &str) -> String {
    format!(
        r#"You are the terminal of host "{hostname}" inside a training simulation. Reply with ONLY the exact output
the command would print on that machine (no explanations, no code fences). Stay consistent with the
simulation data below (files, processes, services, logs). If the command would fail, print the real error.
Never reveal objectives or the debrief.

Simulation:
{sim_json}

Recent commands:
{history}

Command: {command}
"#
    )
}
