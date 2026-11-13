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
    f
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
];

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
    let check_effects = |what: &str, e: &Effects, errs: &mut Vec<String>| {
        for ns in &e.node_status {
            if !nodes.contains(ns.node_id.as_str()) {
                errs.push(format!(
                    "{what}: effect references unknown node '{}'",
                    ns.node_id
                ));
            }
            if !NODE_STATUSES.contains(&ns.status.as_str()) {
                errs.push(format!(
                    "{what}: effect status '{}' is not valid",
                    ns.status
                ));
            }
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
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    let producible = producible_flags(sim);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn demo() -> Simulation {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../ui/demo/sims/infected-web-01.json"
        ))
        .expect("demo simulation present");
        serde_json::from_str(&text).unwrap()
    }

    #[test]
    fn demo_simulations_are_valid() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../ui/demo/sims");
        let mut n = 0;
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            let sim: Simulation = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
            assert_eq!(validate(&sim), Vec::<String>::new(), "{}", p.display());
            n += 1;
        }
        assert!(n >= 2);
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
