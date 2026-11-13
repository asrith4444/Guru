//! Bridge to the Codex CLI. Guru never talks to a model directly. It drives
//! `codex exec`, which runs non-interactively and returns structured JSON, to
//! plan courses and generate their assets. Authentication is Codex's own
//! "Sign in with ChatGPT".

use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Child;
use std::process::Command;
use std::process::Stdio;

use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use serde_json::Value;

use crate::config::Config;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sandbox {
    /// Codex may read but change nothing (all of Guru's generation is pure output).
    ReadOnly,
    /// Codex may write inside the working directory.
    WorkspaceWrite,
}

impl Sandbox {
    fn as_arg(self) -> &'static str {
        match self {
            Sandbox::ReadOnly => "read-only",
            Sandbox::WorkspaceWrite => "workspace-write",
        }
    }
}

pub struct ExecRequest<'a> {
    /// Used for scratch file names; must be filesystem-safe.
    pub label: &'a str,
    pub cwd: &'a Path,
    pub prompt: String,
    pub schema: Option<Value>,
    pub sandbox: Sandbox,
    pub web_search: bool,
    /// Reasoning effort override for this call ("low" for quick sizing calls).
    pub effort: Option<&'a str>,
}

#[derive(Clone, Debug)]
pub struct Codex {
    pub bin: PathBuf,
    /// Hard limit for one request; a hung request is killed and reported as a
    /// timeout so the caller can retry instead of waiting forever.
    pub timeout: std::time::Duration,
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub web_search: bool,
}

/// Something Codex is doing right now, for progress displays.
pub type Activity<'a> = &'a (dyn Fn(&str) + Sync);

impl Codex {
    pub fn from_config(cfg: &Config) -> Self {
        let bin = std::env::var_os("GURU_CODEX_BIN")
            .map(PathBuf::from)
            .or_else(|| cfg.codex_bin.clone().map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("codex"));
        let timeout_secs = std::env::var("GURU_CODEX_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(20 * 60);
        Self {
            bin,
            timeout: std::time::Duration::from_secs(timeout_secs.max(1)),
            model: cfg.model.clone(),
            reasoning_effort: cfg.reasoning_effort.clone(),
            web_search: cfg.web_search,
        }
    }

    pub fn version(&self) -> Option<String> {
        let out = Command::new(&self.bin).arg("--version").output().ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    /// Returns (logged_in, human readable status).
    pub fn login_status(&self) -> (bool, String) {
        match Command::new(&self.bin).args(["login", "status"]).output() {
            Ok(out) => {
                let mut msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
                if msg.is_empty() {
                    msg = String::from_utf8_lossy(&out.stdout).trim().to_string();
                }
                (out.status.success(), msg)
            }
            Err(e) => (false, format!("Codex CLI not found ({e})")),
        }
    }

    /// Start `codex login` (Sign in with ChatGPT). Codex opens the browser and
    /// prints the URL; the caller reads stderr/stdout and waits for the child.
    pub fn spawn_login(&self) -> Result<Child> {
        Command::new(&self.bin)
            .arg("login")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("failed to start {}", self.bin.display()))
    }

    fn common_args(&self, cmd: &mut Command, web_search: bool, effort: Option<&str>) {
        if let Some(m) = &self.model {
            cmd.args(["-m", m]);
        }
        if let Some(e) = effort.or(self.reasoning_effort.as_deref()) {
            cmd.args(["-c", &format!("model_reasoning_effort=\"{e}\"")]);
        }
        let mode = if web_search && self.web_search {
            "live"
        } else {
            "disabled"
        };
        cmd.args(["-c", &format!("web_search=\"{mode}\"")]);
    }

    /// Run one non-interactive Codex turn and return its final message.
    /// With a schema, the final message is parsed and returned as JSON.
    pub fn exec(&self, req: ExecRequest<'_>, activity: Option<Activity<'_>>) -> Result<Value> {
        let scratch = req.cwd.join(".guru");
        std::fs::create_dir_all(&scratch)?;
        let out_file = scratch.join(format!("out-{}.txt", req.label));
        let _ = std::fs::remove_file(&out_file);

        let mut cmd = Command::new(&self.bin);
        cmd.arg("exec")
            .arg("--json")
            .arg("--skip-git-repo-check")
            .arg("--ephemeral")
            .args(["--sandbox", req.sandbox.as_arg()])
            .arg("-C")
            .arg(req.cwd)
            .arg("-o")
            .arg(&out_file);
        self.common_args(&mut cmd, req.web_search, req.effort);
        if let Some(schema) = &req.schema {
            let schema_file = scratch.join(format!("schema-{}.json", req.label));
            std::fs::write(&schema_file, serde_json::to_vec_pretty(schema)?)?;
            cmd.arg("--output-schema").arg(&schema_file);
        }
        // Prompt via stdin: no argument-length limits, nothing in `ps` output.
        cmd.arg("-");
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn().with_context(|| {
            format!(
                "failed to start Codex at '{}'. Install it (npm i -g @openai/codex) or set its path in Settings.",
                self.bin.display()
            )
        })?;
        {
            let mut stdin = child.stdin.take().context("codex stdin unavailable")?;
            stdin.write_all(req.prompt.as_bytes())?;
        }
        let stderr = child.stderr.take();
        let stderr_reader = std::thread::spawn(move || {
            let mut buf = String::new();
            if let Some(mut e) = stderr {
                let _ = std::io::Read::read_to_string(&mut e, &mut buf);
            }
            buf
        });

        // Read events on a separate thread so a hung process (or a grandchild
        // holding stdout open) can never block us past the deadline.
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        if let Some(stdout) = child.stdout.take() {
            std::thread::spawn(move || {
                for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                    if tx.send(line).is_err() {
                        break;
                    }
                }
            });
        }
        let deadline = std::time::Instant::now() + self.timeout;
        let mut errors = Vec::new();
        let mut last_agent_message = None;
        let mut timed_out = false;
        loop {
            let now = std::time::Instant::now();
            if now >= deadline {
                timed_out = true;
                break;
            }
            match rx.recv_timeout(deadline - now) {
                Ok(line) => {
                    let Ok(event) = serde_json::from_str::<Value>(&line) else {
                        continue;
                    };
                    match interpret_event(&event) {
                        Some(EventInfo::Message(m)) => last_agent_message = Some(m),
                        Some(EventInfo::Activity(a)) => {
                            if let Some(cb) = activity {
                                cb(&a);
                            }
                        }
                        Some(EventInfo::Error(e)) => errors.push(e),
                        None => {}
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    timed_out = true;
                    break;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        if timed_out {
            let _ = child.kill();
            let _ = child.wait();
            bail!(
                "Codex step '{}' timed out after {} min",
                req.label,
                self.timeout.as_secs().div_ceil(60)
            );
        }
        let status = child.wait()?;
        let stderr_text = stderr_reader.join().unwrap_or_default();
        if !status.success() && !errors.is_empty() {
            // Surface Codex's own error (auth, usage limits, network) verbatim so
            // the caller can decide whether to retry, wait or stop.
            bail!("Codex step '{}' failed: {}", req.label, errors.join(" "));
        }

        let text = std::fs::read_to_string(&out_file)
            .ok()
            .filter(|t| !t.trim().is_empty())
            .or(last_agent_message);
        let Some(text) = text else {
            let tail: Vec<&str> = stderr_text.lines().rev().take(8).collect();
            let tail: Vec<&str> = tail.into_iter().rev().collect();
            bail!(
                "Codex step '{}' failed (exit {}). {} {}",
                req.label,
                status.code().unwrap_or(-1),
                errors.join(" "),
                tail.join("\n")
            );
        };
        if req.schema.is_some() {
            parse_json_lenient(&text)
                .with_context(|| format!("Codex step '{}' returned invalid JSON", req.label))
        } else {
            Ok(Value::String(text))
        }
    }
}

#[derive(Debug, PartialEq)]
enum EventInfo {
    Message(String),
    Activity(String),
    Error(String),
}

fn interpret_event(event: &Value) -> Option<EventInfo> {
    let ty = event.get("type")?.as_str()?;
    match ty {
        "item.started" | "item.completed" => {
            let item = event.get("item")?;
            let item_ty = item.get("type")?.as_str()?;
            let started = ty == "item.started";
            match item_ty {
                "agent_message" if !started => item
                    .get("text")
                    .and_then(Value::as_str)
                    .map(|t| EventInfo::Message(t.to_string())),
                "web_search" if started => {
                    let q = item.get("query").and_then(Value::as_str).unwrap_or("");
                    Some(EventInfo::Activity(format!("Researching: {q}")))
                }
                "command_execution" if started => {
                    let c = item.get("command").and_then(Value::as_str).unwrap_or("");
                    let c: String = c.chars().take(80).collect();
                    Some(EventInfo::Activity(format!("Checking: {c}")))
                }
                "reasoning" if !started => Some(EventInfo::Activity("Thinking…".into())),
                "error" => item
                    .get("message")
                    .and_then(Value::as_str)
                    .map(|m| EventInfo::Error(m.to_string())),
                _ => None,
            }
        }
        "turn.failed" => event
            .pointer("/error/message")
            .and_then(Value::as_str)
            .map(|m| EventInfo::Error(m.to_string())),
        "error" => event
            .get("message")
            .and_then(Value::as_str)
            .map(|m| EventInfo::Error(m.to_string())),
        _ => None,
    }
}

/// Parse JSON, tolerating a fenced code block or prose around it.
pub fn parse_json_lenient(text: &str) -> Result<Value> {
    if let Ok(v) = serde_json::from_str(text.trim()) {
        return Ok(v);
    }
    let start = text.find('{').context("no JSON object found")?;
    let end = text.rfind('}').context("no JSON object found")?;
    if end <= start {
        bail!("no JSON object found");
    }
    Ok(serde_json::from_str(&text[start..=end])?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn lenient_json_parsing() {
        assert_eq!(parse_json_lenient("{\"a\":1}").unwrap(), json!({"a":1}));
        assert_eq!(
            parse_json_lenient("Here:\n```json\n{\"a\": {\"b\": 2}}\n```").unwrap(),
            json!({"a":{"b":2}})
        );
        assert!(parse_json_lenient("no json").is_err());
    }

    #[test]
    fn events_are_interpreted() {
        assert_eq!(
            interpret_event(
                &json!({"type":"item.completed","item":{"id":"1","type":"agent_message","text":"hi"}})
            ),
            Some(EventInfo::Message("hi".into()))
        );
        assert_eq!(
            interpret_event(&json!({"type":"turn.failed","error":{"message":"boom"}})),
            Some(EventInfo::Error("boom".into()))
        );
        assert_eq!(
            interpret_event(
                &json!({"type":"item.started","item":{"id":"2","type":"web_search","query":"grafana panels"}})
            ),
            Some(EventInfo::Activity("Researching: grafana panels".into()))
        );
    }
}
