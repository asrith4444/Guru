//! Guru simulations: AI-authored, offline-playable practice worlds.
//!
//! Codex writes each simulation once, as data, when a course is downloaded.
//! Guru's built-in engine (ui/js/sim/engine.js) plays it back with no AI and no
//! installs: a visual world of machines, terminals with a virtual filesystem,
//! processes and services, log streams, and *simulated tools* (dashboards,
//! query editors, config forms, consoles, conversations). That is how a learner
//! practises Grafana, Kubernetes or incident response without the tool, an
//! account, or a machine big enough to run it.
//!
//! The spec is deliberately declarative (no generated code runs), so it is safe
//! to render and cheap to validate.

use std::collections::HashSet;

use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Simulation {
    pub id: String,
    pub title: String,
    /// incident | tool | guided | roleplay
    pub kind: String,
    /// The real-world tool or environment being simulated ("Grafana", "Linux server").
    pub tool: String,
    pub minutes: u32,
    pub skill_ids: Vec<String>,
    /// Markdown: the story, your role, what you are asked to do.
    pub briefing: String,
    pub world: World,
    pub hosts: Vec<Host>,
    pub logs: Vec<LogStream>,
    pub apps: Vec<App>,
    pub objectives: Vec<Objective>,
    pub hints: Vec<String>,
    /// Markdown shown after completion: what really happened, how pros handle it.
    pub debrief: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct World {
    pub nodes: Vec<Node>,
    pub links: Vec<Link>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Node {
    pub id: String,
    pub label: String,
    /// server | database | load_balancer | container | service | client | cloud | queue | firewall | app | person
    pub kind: String,
    /// healthy | degraded | down | compromised | unknown
    pub status: String,
    pub detail: String,
    /// Position on the map, 0-100 in both axes.
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Link {
    pub from: String,
    pub to: String,
    pub label: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Host {
    pub node_id: String,
    pub hostname: String,
    pub user: String,
    pub cwd: String,
    pub motd: String,
    pub files: Vec<File>,
    pub processes: Vec<Process>,
    pub services: Vec<Service>,
    pub commands: Vec<CommandRule>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct File {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Process {
    pub pid: u32,
    pub user: String,
    pub cpu: f32,
    pub mem: f32,
    pub command: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Service {
    pub name: String,
    /// active | inactive | failed
    pub status: String,
    pub detail: String,
}

/// A scripted command response. `pattern` is a regular expression matched
/// against the whole trimmed command line; rules win over built-in commands.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct CommandRule {
    pub pattern: String,
    pub output: String,
    pub effects: Effects,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Effects {
    pub set_flags: Vec<String>,
    /// Undo earlier progress when the world changes back (e.g. plugging a faulty
    /// appliance back in trips the RCD again).
    pub clear_flags: Vec<String>,
    pub node_status: Vec<NodeStatus>,
    /// Shown to the learner as a notification ("Alert resolved", "Deploy started").
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct NodeStatus {
    pub node_id: String,
    pub status: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct LogStream {
    pub id: String,
    pub name: String,
    pub node_id: String,
    pub lines: Vec<LogLine>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct LogLine {
    pub ts: String,
    /// debug | info | warn | error | critical
    pub level: String,
    pub message: String,
}

/// A simulated tool: a few screens made of widgets.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct App {
    pub id: String,
    pub name: String,
    pub node_id: String,
    /// Accent color (#rrggbb) so different tools are visually distinct.
    pub accent: String,
    pub screens: Vec<Screen>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Screen {
    pub id: String,
    pub title: String,
    pub widgets: Vec<Widget>,
}

/// One widget. `type` selects which of the other fields are meaningful:
///
/// - markdown:   text
/// - stat:       value, unit, status (ok | warn | crit)
/// - timeseries: unit, labels (x axis), series[{name, points}], thresholds
/// - table:      columns, rows
/// - logs:       stream_id (embeds a log stream with search/filter)
/// - query:      language, placeholder, responses[], fallback
/// - form:       fields[], submit_label, rules[], fallback
/// - actions:    buttons[]
/// - editor:     language, path, text (initial content), checks[], effects (on success)
/// - chat:       persona, text (opening line), turns[], fallback
/// - workbench:  text (instructions), tools[], items[] (readings per tool, actions), links[];
///   for physical work such as electrical panels, plumbing, HVAC, engines or circuit boards
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Widget {
    #[serde(rename = "type")]
    pub kind: String,
    pub id: String,
    pub title: String,
    pub text: String,
    pub value: String,
    pub unit: String,
    pub status: String,
    pub labels: Vec<String>,
    pub series: Vec<Series>,
    pub thresholds: Vec<f64>,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub stream_id: String,
    pub language: String,
    pub placeholder: String,
    pub path: String,
    pub persona: String,
    pub responses: Vec<QueryResponse>,
    pub fields: Vec<FormField>,
    pub submit_label: String,
    pub rules: Vec<FormRule>,
    pub buttons: Vec<Button>,
    pub checks: Vec<EditorCheck>,
    pub turns: Vec<ChatTurn>,
    /// workbench: tools the learner can pick, items on the bench, connections.
    pub tools: Vec<String>,
    pub items: Vec<BenchItem>,
    pub links: Vec<Link>,
    pub fallback: String,
    pub effects: Effects,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Series {
    pub name: String,
    pub points: Vec<f64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct QueryResponse {
    pub pattern: String,
    /// table | timeseries | text | error
    pub result: String,
    pub text: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub labels: Vec<String>,
    pub series: Vec<Series>,
    pub effects: Effects,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FormField {
    pub id: String,
    pub label: String,
    /// text | number | select | toggle | textarea
    pub kind: String,
    pub options: Vec<String>,
    pub value: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FormRule {
    /// All must match (field id -> regex on the submitted value).
    pub conditions: Vec<FieldCondition>,
    pub ok: bool,
    pub message: String,
    pub effects: Effects,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FieldCondition {
    pub field: String,
    pub pattern: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Button {
    pub label: String,
    pub message: String,
    pub effects: Effects,
    /// Flags that must already be set (e.g. "isolated", "proved_dead").
    /// Without them the action is refused with `blocked_message`: this is how
    /// simulations teach safe order of work.
    pub requires: Vec<String>,
    pub blocked_message: String,
}

/// A physical object on a workbench: a breaker, a valve, a pipe joint, a chip pin.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct BenchItem {
    pub id: String,
    pub label: String,
    /// Free text: breaker, outlet, valve, pipe, boiler, resistor, fpga…
    pub kind: String,
    /// ok | fault | warning | off | on | unknown
    pub status: String,
    pub detail: String,
    /// Position on the bench, 0-100 in both axes.
    pub x: f32,
    pub y: f32,
    /// What each tool shows. The first reading for the chosen tool whose
    /// `requires` flags are all set wins, so measurements change as the learner acts.
    pub readings: Vec<Reading>,
    pub actions: Vec<Button>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Reading {
    /// One of the widget's `tools`, or "Look" for visual inspection.
    pub tool: String,
    pub requires: Vec<String>,
    pub value: String,
    pub effects: Effects,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct EditorCheck {
    /// true: content must match `pattern`; false: content must NOT match it.
    pub must_match: bool,
    pub pattern: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ChatTurn {
    /// Regex on the learner's message that triggers this turn.
    pub expect: String,
    pub reply: String,
    pub feedback: String,
    pub effects: Effects,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Objective {
    pub id: String,
    pub text: String,
    /// answer: the learner states a finding. state: the learner changes the world.
    pub kind: String,
    pub question: String,
    /// Multiple choice when non-empty (indices of correct options in `correct`).
    pub options: Vec<String>,
    pub correct: Vec<usize>,
    /// Free-text answers: any regex (case-insensitive) that must match.
    pub accept: Vec<String>,
    /// State objectives: all flags must be set.
    pub flags: Vec<String>,
    pub explanation: String,
}

/// Flags the engine sets by itself (see engine.js), in addition to explicit effects.
pub fn builtin_flags(sim: &Simulation) -> HashSet<String> {
    let mut f = HashSet::new();
    for h in &sim.hosts {
        for p in &h.processes {
            f.insert(format!("killed:{}", p.pid));
            if let Some(name) = p.command.split_whitespace().next() {
                let base = name.rsplit('/').next().unwrap_or(name);
                f.insert(format!("killed:{base}"));
            }
        }
        for s in &h.services {
            for verb in ["stopped", "started", "restarted", "disabled", "enabled"] {
                f.insert(format!("{verb}:{}", s.name));
            }
        }
        for file in &h.files {
            f.insert(format!("read:{}", file.path));
            // Removing the file or any parent directory counts.
            let mut p = file.path.as_str();
            loop {
                f.insert(format!("removed:{p}"));
                match p.rfind('/') {
                    Some(0) | None => break,
                    Some(i) => p = &p[..i],
                }
            }
        }
    }
    for l in &sim.logs {
        f.insert(format!("viewed:{}", l.id));
    }
    for w in widgets(sim) {
        for it in &w.items {
            f.insert(format!("inspected:{}", it.id));
        }
    }
    f
}

fn widgets(sim: &Simulation) -> impl Iterator<Item = &Widget> {
    sim.apps
        .iter()
        .flat_map(|a| a.screens.iter())
        .flat_map(|s| s.widgets.iter())
}

fn effect_flags(e: &Effects, out: &mut HashSet<String>) {
    out.extend(e.set_flags.iter().cloned());
}

/// Every flag that some action in the simulation can set.
pub fn producible_flags(sim: &Simulation) -> HashSet<String> {
    let mut f = builtin_flags(sim);
    for h in &sim.hosts {
        for c in &h.commands {
            effect_flags(&c.effects, &mut f);
        }
    }
    for a in &sim.apps {
        for s in &a.screens {
            for w in &s.widgets {
                effect_flags(&w.effects, &mut f);
                for r in &w.responses {
                    effect_flags(&r.effects, &mut f);
                }
                for r in &w.rules {
                    effect_flags(&r.effects, &mut f);
                }
                for b in &w.buttons {
                    effect_flags(&b.effects, &mut f);
                }
                for t in &w.turns {
                    effect_flags(&t.effects, &mut f);
                }
                for it in &w.items {
                    for r in &it.readings {
                        effect_flags(&r.effects, &mut f);
                    }
                    for b in &it.actions {
                        effect_flags(&b.effects, &mut f);
                    }
                }
            }
        }
    }
    f
}

const NODE_STATUSES: &[&str] = &["healthy", "degraded", "down", "compromised", "unknown"];
const WIDGETS: &[&str] = &[
    "markdown",
    "stat",
    "timeseries",
    "table",
    "logs",
    "query",
    "form",
    "actions",
    "editor",
    "chat",
    "workbench",
];
const ITEM_STATUSES: &[&str] = &["ok", "fault", "warning", "off", "on", "unknown"];

/// Semantic validation: everything the engine needs to play the simulation
/// must be consistent. Returns human-readable problems (empty = valid); these
/// are fed back to Codex verbatim when a retry is needed.
pub fn validate(sim: &Simulation) -> Vec<String> {
    let mut errs = Vec::new();
    let regex_ok = |where_: &str, p: &str, errs: &mut Vec<String>| {
        if p.trim().is_empty() {
            errs.push(format!("{where_}: empty regex pattern"));
        } else if let Err(e) = regex::RegexBuilder::new(p).case_insensitive(true).build() {
            errs.push(format!(
                "{where_}: invalid regex {p:?} ({}). Use simple regex without lookaround or backreferences.",
                e.to_string().lines().last().unwrap_or("error")
            ));
        }
    };

    if sim.id.trim().is_empty() || sim.title.trim().is_empty() {
        errs.push("simulation needs an id and a title".into());
    }
    if sim.briefing.trim().is_empty() {
        errs.push("briefing is empty".into());
    }
    if sim.objectives.is_empty() {
        errs.push("at least one objective is required".into());
    }
    if sim.hosts.is_empty() && sim.apps.is_empty() && sim.logs.is_empty() {
        errs.push("nothing to interact with: add hosts, logs or apps".into());
    }

    let nodes: HashSet<&str> = sim.world.nodes.iter().map(|n| n.id.as_str()).collect();
    if nodes.len() != sim.world.nodes.len() {
        errs.push("world.nodes ids must be unique".into());
    }
    for n in &sim.world.nodes {
        if !NODE_STATUSES.contains(&n.status.as_str()) {
            errs.push(format!(
                "node {}: status must be one of {NODE_STATUSES:?}",
                n.id
            ));
        }
    }
    for l in &sim.world.links {
        if !nodes.contains(l.from.as_str()) || !nodes.contains(l.to.as_str()) {
            errs.push(format!(
                "link {}->{} references an unknown node",
                l.from, l.to
            ));
        }
    }
    let check_node = |what: String, id: &str, errs: &mut Vec<String>| {
        if !id.is_empty() && !nodes.contains(id) {
            errs.push(format!("{what} references unknown node '{id}'"));
        }
    };
    let items: HashSet<&str> = widgets(sim)
        .flat_map(|w| w.items.iter())
        .map(|i| i.id.as_str())
        .collect();
    let producible = producible_flags(sim);
    let check_effects = |what: &str, e: &Effects, errs: &mut Vec<String>| {
        for ns in &e.node_status {
            let id = ns.node_id.as_str();
            if items.contains(id) {
                if !ITEM_STATUSES.contains(&ns.status.as_str()) {
                    errs.push(format!(
                        "{what}: item status '{}' must be one of {ITEM_STATUSES:?}",
                        ns.status
                    ));
                }
            } else if !nodes.contains(id) {
                errs.push(format!(
                    "{what}: effect references unknown node or item '{id}'"
                ));
            } else if !NODE_STATUSES.contains(&ns.status.as_str()) {
                errs.push(format!(
                    "{what}: effect status '{}' is not valid",
                    ns.status
                ));
            }
        }
    };
    let check_requires = |what: &str, b: &Button, errs: &mut Vec<String>| {
        for f in &b.requires {
            if !producible.contains(f) {
                errs.push(format!(
                    "{what}: action '{}' requires flag '{f}' that nothing can set",
                    b.label
                ));
            }
        }
        if !b.requires.is_empty() && b.blocked_message.trim().is_empty() {
            errs.push(format!("{what}: action '{}' has requirements but no blocked_message explaining why it is refused", b.label));
        }
    };

    for h in &sim.hosts {
        if h.node_id.is_empty() {
            errs.push(format!("host {}: node_id is required", h.hostname));
        }
        check_node(format!("host {}", h.hostname), &h.node_id, &mut errs);
        for (i, c) in h.commands.iter().enumerate() {
            regex_ok(
                &format!("host {} command rule {i}", h.hostname),
                &c.pattern,
                &mut errs,
            );
            check_effects(
                &format!("host {} command rule {i}", h.hostname),
                &c.effects,
                &mut errs,
            );
        }
        for f in &h.files {
            if !f.path.starts_with('/') {
                errs.push(format!(
                    "host {}: file path '{}' must be absolute",
                    h.hostname, f.path
                ));
            }
        }
    }
    let streams: HashSet<&str> = sim.logs.iter().map(|l| l.id.as_str()).collect();
    for l in &sim.logs {
        check_node(format!("log stream {}", l.id), &l.node_id, &mut errs);
        if l.lines.is_empty() {
            errs.push(format!("log stream {} has no lines", l.id));
        }
    }
    for a in &sim.apps {
        check_node(format!("app {}", a.name), &a.node_id, &mut errs);
        if a.screens.is_empty() {
            errs.push(format!("app {} has no screens", a.name));
        }
        for s in &a.screens {
            for (i, w) in s.widgets.iter().enumerate() {
                let at = format!("app {} screen {} widget {i} ({})", a.name, s.title, w.kind);
                if !WIDGETS.contains(&w.kind.as_str()) {
                    errs.push(format!("{at}: unknown widget type; use one of {WIDGETS:?}"));
                    continue;
                }
                check_effects(&at, &w.effects, &mut errs);
                match w.kind.as_str() {
                    "logs" if !streams.contains(w.stream_id.as_str()) => {
                        errs.push(format!("{at}: stream_id '{}' does not exist", w.stream_id));
                    }
                    "timeseries" => {
                        if w.series.is_empty() {
                            errs.push(format!("{at}: needs at least one series"));
                        }
                        for s in &w.series {
                            if !w.labels.is_empty() && s.points.len() != w.labels.len() {
                                errs.push(format!(
                                    "{at}: series '{}' has {} points but {} labels",
                                    s.name,
                                    s.points.len(),
                                    w.labels.len()
                                ));
                            }
                        }
                    }
                    "table" => {
                        for r in &w.rows {
                            if r.len() != w.columns.len() {
                                errs.push(format!(
                                    "{at}: every row needs {} cells",
                                    w.columns.len()
                                ));
                                break;
                            }
                        }
                    }
                    "query" => {
                        if w.responses.is_empty() {
                            errs.push(format!("{at}: needs responses"));
                        }
                        for (j, r) in w.responses.iter().enumerate() {
                            regex_ok(&format!("{at} response {j}"), &r.pattern, &mut errs);
                            check_effects(&at, &r.effects, &mut errs);
                        }
                    }
                    "form" => {
                        let ids: HashSet<&str> = w.fields.iter().map(|f| f.id.as_str()).collect();
                        for (j, r) in w.rules.iter().enumerate() {
                            for c in &r.conditions {
                                if !ids.contains(c.field.as_str()) {
                                    errs.push(format!(
                                        "{at} rule {j}: unknown field '{}'",
                                        c.field
                                    ));
                                }
                                regex_ok(&format!("{at} rule {j}"), &c.pattern, &mut errs);
                            }
                            check_effects(&at, &r.effects, &mut errs);
                        }
                    }
                    "editor" => {
                        if w.checks.is_empty() {
                            errs.push(format!("{at}: needs checks"));
                        }
                        for (j, c) in w.checks.iter().enumerate() {
                            regex_ok(&format!("{at} check {j}"), &c.pattern, &mut errs);
                        }
                    }
                    "chat" => {
                        for (j, t) in w.turns.iter().enumerate() {
                            regex_ok(&format!("{at} turn {j}"), &t.expect, &mut errs);
                            check_effects(&at, &t.effects, &mut errs);
                        }
                    }
                    "actions" => {
                        for b in &w.buttons {
                            check_effects(&at, &b.effects, &mut errs);
                            check_requires(&at, b, &mut errs);
                        }
                    }
                    "workbench" => {
                        if w.items.is_empty() {
                            errs.push(format!("{at}: needs items"));
                        }
                        let ids: HashSet<&str> = w.items.iter().map(|i| i.id.as_str()).collect();
                        if ids.len() != w.items.len() {
                            errs.push(format!("{at}: item ids must be unique"));
                        }
                        for l in &w.links {
                            if !ids.contains(l.from.as_str()) || !ids.contains(l.to.as_str()) {
                                errs.push(format!(
                                    "{at}: link {}->{} references an unknown item",
                                    l.from, l.to
                                ));
                            }
                        }
                        for it in &w.items {
                            if !ITEM_STATUSES.contains(&it.status.as_str()) {
                                errs.push(format!(
                                    "{at}: item {} status must be one of {ITEM_STATUSES:?}",
                                    it.id
                                ));
                            }
                            for r in &it.readings {
                                if r.tool != "Look" && !w.tools.contains(&r.tool) {
                                    errs.push(format!("{at}: item {} has a reading for tool '{}' which is not in tools (or use \"Look\")", it.id, r.tool));
                                }
                                for f in &r.requires {
                                    if !producible.contains(f) {
                                        errs.push(format!("{at}: item {} reading requires flag '{f}' that nothing can set", it.id));
                                    }
                                }
                                check_effects(&at, &r.effects, &mut errs);
                            }
                            for b in &it.actions {
                                check_effects(&at, &b.effects, &mut errs);
                                check_requires(&format!("{at} item {}", it.id), b, &mut errs);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    let mut ids = HashSet::new();
    for o in &sim.objectives {
        if !ids.insert(o.id.as_str()) {
            errs.push(format!("objective id '{}' is duplicated", o.id));
        }
        match o.kind.as_str() {
            "answer" => {
                if o.question.trim().is_empty() {
                    errs.push(format!(
                        "objective {}: answer objectives need a question",
                        o.id
                    ));
                }
                if o.options.is_empty() {
                    if o.accept.is_empty() {
                        errs.push(format!(
                            "objective {}: free-text answers need accept patterns",
                            o.id
                        ));
                    }
                    for p in &o.accept {
                        regex_ok(&format!("objective {} accept", o.id), p, &mut errs);
                    }
                } else if o.correct.is_empty() || o.correct.iter().any(|&c| c >= o.options.len()) {
                    errs.push(format!(
                        "objective {}: correct must list valid option indices",
                        o.id
                    ));
                }
            }
            "state" => {
                if o.flags.is_empty() {
                    errs.push(format!("objective {}: state objectives need flags", o.id));
                }
                for f in &o.flags {
                    if !producible.contains(f) {
                        errs.push(format!(
                            "objective {}: flag '{f}' can never be set (no command rule, widget or built-in action produces it)",
                            o.id
                        ));
                    }
                }
            }
            other => errs.push(format!(
                "objective {}: kind '{other}' must be 'answer' or 'state'",
                o.id
            )),
        }
    }
    errs
}

/// Is this simulation worth the learner's time? Unlike `validate`, these are
/// richness problems: Codex is asked to fix them, but a valid simulation is
/// accepted on the last attempt.
pub fn quality(sim: &Simulation) -> Vec<String> {
    let mut q = Vec::new();
    if sim.objectives.len() < 3 {
        q.push(format!(
            "only {} objectives; give 3-6 that mix finding (answer) and doing (state)",
            sim.objectives.len()
        ));
    }
    if !sim.objectives.iter().any(|o| o.kind == "state") {
        q.push(
            "no objective requires the learner to act; add at least one 'state' objective".into(),
        );
    }
    if sim.briefing.chars().count() < 150 {
        q.push(
            "the briefing is too short; set the scene, the learner's role and what is expected"
                .into(),
        );
    }
    if sim.hints.len() < 3 {
        q.push("give 3 graduated hints (nudge, pointer, near-answer)".into());
    }
    if sim.debrief.chars().count() < 300 {
        q.push("the debrief is too short; explain what happened, how a professional approaches it, and prevention".into());
    }
    let lines: usize = sim.logs.iter().map(|l| l.lines.len()).sum();
    if !sim.logs.is_empty() && lines < 20 {
        q.push(format!("logs have only {lines} lines in total; realistic logs need background noise around the signal (aim for 40+)"));
    }
    for h in &sim.hosts {
        if h.files.len() + h.commands.len() < 4 {
            q.push(format!(
                "host {} has too little to explore; add realistic files and command rules",
                h.hostname
            ));
        }
    }
    for w in widgets(sim) {
        for it in &w.items {
            if it.readings.is_empty() && it.actions.is_empty() {
                q.push(format!(
                    "workbench item {} has no readings or actions",
                    it.id
                ));
            }
        }
    }
    let interactive = !sim.hosts.is_empty()
        || widgets(sim).any(|w| {
            matches!(
                w.kind.as_str(),
                "query" | "form" | "actions" | "editor" | "chat" | "workbench"
            )
        });
    if !interactive {
        q.push("nothing to operate: add a terminal host or an interactive tool widget".into());
    }
    q
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Every simulation in every built-in demo course.
    pub(crate) fn demo_sim_paths() -> Vec<std::path::PathBuf> {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../ui/demo");
        let mut v = Vec::new();
        for course in std::fs::read_dir(root).unwrap() {
            for e in std::fs::read_dir(course.unwrap().path().join("sims")).unwrap() {
                v.push(e.unwrap().path());
            }
        }
        v.sort();
        v
    }

    fn demo() -> Simulation {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../ui/demo/incident-response/sims/infected-web-01.json"
        ))
        .expect("demo simulation present");
        serde_json::from_str(&text).unwrap()
    }

    #[test]
    fn demo_simulations_are_valid_and_rich() {
        let paths = demo_sim_paths();
        assert!(paths.len() >= 3);
        for p in paths {
            let sim: Simulation = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
            assert_eq!(validate(&sim), Vec::<String>::new(), "{}", p.display());
            assert_eq!(quality(&sim), Vec::<String>::new(), "{}", p.display());
        }
    }

    #[test]
    fn workbench_rules_are_checked() {
        let p = demo_sim_paths()
            .into_iter()
            .find(|p| p.ends_with("rcd-kitchen.json"))
            .unwrap();
        let mut s: Simulation = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
        let bench = &mut s.apps[0].screens[0].widgets[0];
        bench.items[1].actions[0].requires.push("never:set".into());
        bench.items[1].actions[0].blocked_message.clear();
        bench.items[4].readings[0].tool = "Oscilloscope".into();
        bench.items[0].status = "melted".into();
        let errs = validate(&s).join("\n");
        assert!(errs.contains("requires flag 'never:set'"), "{errs}");
        assert!(errs.contains("no blocked_message"), "{errs}");
        assert!(errs.contains("tool 'Oscilloscope'"), "{errs}");
        assert!(errs.contains("status must be one of"), "{errs}");
        assert!(builtin_flags(&s).contains("inspected:kettle"));
    }

    #[test]
    fn builtin_flags_cover_processes_services_files() {
        let f = builtin_flags(&demo());
        assert!(f.contains("killed:4242"));
        assert!(f.iter().any(|x| x.starts_with("removed:/")));
        assert!(f.iter().any(|x| x.starts_with("stopped:")));
    }

    #[test]
    fn detects_unreachable_flags_and_bad_refs() {
        let mut s = demo();
        s.objectives[0].kind = "state".into();
        s.objectives[0].flags = vec!["never:set".into()];
        s.hosts[0].node_id = "ghost".into();
        s.hosts[0].commands[0].pattern = "(?<=x)y".into();
        let errs = validate(&s).join("\n");
        assert!(errs.contains("can never be set"), "{errs}");
        assert!(errs.contains("unknown node 'ghost'"), "{errs}");
        assert!(errs.contains("invalid regex"), "{errs}");
    }

    #[test]
    fn lenient_parsing_fills_defaults() {
        let s: Simulation = serde_json::from_str(r#"{"id":"x","title":"t"}"#).unwrap();
        assert!(s.hosts.is_empty());
        assert!(!validate(&s).is_empty());
    }
}
