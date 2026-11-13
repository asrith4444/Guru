//! Bridge to the Codex CLI. Guru never talks to a model directly: it drives
//! `codex exec` (non-interactive, structured JSON output) for planning,
//! environment building and grading, and launches the interactive `codex` TUI
//! as a live tutor. Authentication is Codex's own "Sign in with ChatGPT".

use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;

use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use serde_json::Value;

use crate::config::Config;
use crate::ui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sandbox {
    /// Codex may read anything but change nothing (grading, answering).
    ReadOnly,
    /// Codex may write inside the course directory (lessons, labs, scripts).
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
    pub label: &'a str,
    pub cwd: &'a Path,
    pub prompt: String,
    pub schema: Option<Value>,
    pub sandbox: Sandbox,
    pub web_search: bool,
    /// Suppress progress lines (used when running in the background).
    pub quiet: bool,
    /// Optional images (e.g. a photo of a drawing or handwritten work).
    pub images: Vec<PathBuf>,
}

#[derive(Clone)]
pub struct Codex {
    pub bin: PathBuf,
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub web_search: bool,
}

impl Codex {
    pub fn from_config(cfg: &Config) -> Self {
        let bin = std::env::var_os("GURU_CODEX_BIN")
            .map(PathBuf::from)
            .or_else(|| cfg.codex_bin.clone().map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("codex"));
        Self {
            bin,
            model: cfg.model.clone(),
            reasoning_effort: cfg.reasoning_effort.clone(),
            web_search: cfg.web_search,
        }
    }

    pub fn available(&self) -> bool {
        Command::new(&self.bin)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
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
            Err(e) => (false, format!("could not run codex: {e}")),
        }
    }

    /// Hand the terminal to `codex login` (Sign in with ChatGPT).
    pub fn login(&self) -> Result<()> {
        let status = Command::new(&self.bin)
            .arg("login")
            .status()
            .with_context(|| format!("failed to start {}", self.bin.display()))?;
        if !status.success() {
            bail!("codex login did not complete");
        }
        Ok(())
    }

    fn common_args(&self, cmd: &mut Command, web_search: bool) {
        if let Some(m) = &self.model {
            cmd.args(["-m", m]);
        }
        if let Some(e) = &self.reasoning_effort {
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
    pub fn exec(&self, req: ExecRequest<'_>) -> Result<Value> {
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
        self.common_args(&mut cmd, req.web_search);
        if let Some(schema) = &req.schema {
            let schema_file = scratch.join(format!("schema-{}.json", req.label));
            std::fs::write(&schema_file, serde_json::to_vec_pretty(schema)?)?;
            cmd.arg("--output-schema").arg(&schema_file);
        }
        for img in &req.images {
            cmd.arg("--image").arg(img);
        }
        // Prompt via stdin: no argument-length limits, nothing in `ps` output.
        cmd.arg("-");
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn().with_context(|| {
            format!(
                "failed to start {} (is Codex installed? run `guru doctor`)",
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

        let mut errors = Vec::new();
        let mut last_agent_message = None;
        if let Some(stdout) = child.stdout.take() {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                let Ok(event) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };
                if let Some(msg) = handle_event(&event, req.quiet, &mut errors) {
                    last_agent_message = Some(msg);
                }
            }
        }
        let status = child.wait()?;
        let stderr_text = stderr_reader.join().unwrap_or_default();

        let text = std::fs::read_to_string(&out_file)
            .ok()
            .filter(|t| !t.trim().is_empty())
            .or(last_agent_message);
        let Some(text) = text else {
            let tail: String = stderr_text
                .lines()
                .rev()
                .take(8)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join("\n");
            bail!(
                "Codex step '{}' failed (exit {}).\n{}\n{}",
                req.label,
                status.code().unwrap_or(-1),
                errors.join("\n"),
                tail
            );
        };
        if req.schema.is_some() {
            parse_json_lenient(&text)
                .with_context(|| format!("Codex step '{}' returned invalid JSON", req.label))
        } else {
            Ok(Value::String(text))
        }
    }

    /// Launch the interactive Codex TUI as a live tutor in `cwd`.
    pub fn interactive(&self, cwd: &Path, prompt: &str, web_search: bool) -> Result<()> {
        let mut cmd = Command::new(&self.bin);
        cmd.arg("-C").arg(cwd);
        self.common_args(&mut cmd, web_search);
        if !prompt.is_empty() {
            cmd.arg(prompt);
        }
        let status = cmd
            .status()
            .with_context(|| format!("failed to start {}", self.bin.display()))?;
        if !status.success() {
            ui::warn("The tutor exited with an error.");
        }
        Ok(())
    }
}

/// Print a compact progress line for interesting events; collect errors;
/// return the text of completed agent messages.
fn handle_event(event: &Value, quiet: bool, errors: &mut Vec<String>) -> Option<String> {
    let ty = event.get("type")?.as_str()?;
    match ty {
        "item.started" | "item.completed" => {
            let item = event.get("item")?;
            let item_ty = item.get("type")?.as_str()?;
            let started = ty == "item.started";
            match item_ty {
                "agent_message" if !started => {
                    return item.get("text").and_then(Value::as_str).map(String::from);
                }
                "command_execution" if started && !quiet => {
                    let cmd = item.get("command").and_then(Value::as_str).unwrap_or("");
                    ui::activity("running", cmd);
                }
                "file_change" if !started && !quiet => {
                    let files: Vec<&str> = item
                        .get("changes")
                        .and_then(Value::as_array)
                        .map(|c| c.iter().filter_map(|x| x.get("path")?.as_str()).collect())
                        .unwrap_or_default();
                    let names: Vec<String> = files
                        .iter()
                        .map(|p| {
                            Path::new(p)
                                .file_name()
                                .map(|f| f.to_string_lossy().into_owned())
                                .unwrap_or_default()
                        })
                        .collect();
                    ui::activity("writing", &names.join(", "));
                }
                "web_search" if started && !quiet => {
                    let q = item.get("query").and_then(Value::as_str).unwrap_or("");
                    ui::activity("researching", q);
                }
                "error" => {
                    if let Some(m) = item.get("message").and_then(Value::as_str) {
                        errors.push(m.to_string());
                    }
                }
                _ => {}
            }
        }
        "turn.failed" => {
            if let Some(m) = event.pointer("/error/message").and_then(Value::as_str) {
                errors.push(m.to_string());
            }
        }
        "error" => {
            if let Some(m) = event.get("message").and_then(Value::as_str) {
                errors.push(m.to_string());
            }
        }
        _ => {}
    }
    None
}

/// Parse JSON, tolerating a fenced code block or leading prose around it.
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
            parse_json_lenient("Here you go:\n```json\n{\"a\": {\"b\": 2}}\n```").unwrap(),
            json!({"a":{"b":2}})
        );
        assert!(parse_json_lenient("no json").is_err());
    }

    #[test]
    fn events_are_interpreted() {
        let mut errors = vec![];
        let msg = handle_event(
            &json!({"type":"item.completed","item":{"id":"1","type":"agent_message","text":"hi"}}),
            true,
            &mut errors,
        );
        assert_eq!(msg.as_deref(), Some("hi"));
        handle_event(
            &json!({"type":"turn.failed","error":{"message":"boom"}}),
            true,
            &mut errors,
        );
        handle_event(&json!({"type":"error","message":"bad"}), true, &mut errors);
        assert_eq!(errors, vec!["boom", "bad"]);
    }
}
