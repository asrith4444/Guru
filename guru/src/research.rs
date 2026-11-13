//! Real-world problem mining from the Stack Exchange API.
//!
//! The most up-voted questions under a tag are, almost by definition, the
//! problems the largest number of practitioners ran into. Guru pulls them
//! (plus their accepted/top answers), compresses them, and lets Codex turn them
//! into an issue bank used for break-fix practice. Content is CC BY-SA; every
//! issue keeps its source link for attribution.
//!
//! Uses the system `curl` (handles the API's gzip responses and proxies) so
//! Guru itself needs no HTTP/TLS stack.

use std::path::Path;
use std::process::Command;

use anyhow::bail;
use anyhow::Result;
use serde_json::Value;

use crate::model::ResearchHints;

const API: &str = "https://api.stackexchange.com/2.3";
const PER_TAG: usize = 15;
const BODY_CHARS: usize = 700;

pub struct SeItem {
    pub title: String,
    pub link: String,
    pub score: i64,
    pub question: String,
    pub answer: String,
}

fn get(url: &str) -> Result<Value> {
    let out = Command::new("curl")
        .args([
            "-sS",
            "--compressed",
            "--max-time",
            "20",
            "-A",
            "guru-learning-cli",
            url,
        ])
        .output()?;
    if !out.status.success() {
        bail!("{}", String::from_utf8_lossy(&out.stderr).trim());
    }
    let v: Value = serde_json::from_slice(&out.stdout)?;
    if let Some(msg) = v.get("error_message").and_then(Value::as_str) {
        bail!("stack exchange: {msg}");
    }
    Ok(v)
}

fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Fetch top-voted Q&A for the plan's research hints. Best effort: returns
/// whatever could be fetched, and the reason when nothing could.
pub fn fetch(hints: &ResearchHints, key: Option<&str>, save_to: &Path) -> Result<Vec<SeItem>> {
    let site = hints.stackexchange_site.trim();
    if site.is_empty() {
        return Ok(vec![]);
    }
    let key_param = key
        .map(|k| format!("&key={}", encode(k)))
        .unwrap_or_default();
    let mut questions: Vec<Value> = Vec::new();
    for tag in hints.tags.iter().take(3) {
        let url = format!(
            "{API}/questions?order=desc&sort=votes&tagged={}&site={}&pagesize={PER_TAG}&filter=withbody{key_param}",
            encode(tag),
            encode(site)
        );
        if let Some(items) = get(&url)?.get("items").and_then(Value::as_array) {
            questions.extend(items.iter().cloned());
        }
    }
    let first_tag = hints.tags.first().cloned().unwrap_or_default();
    for q in hints.queries.iter().take(2) {
        let url = format!(
            "{API}/search/advanced?order=desc&sort=votes&q={}&tagged={}&site={}&pagesize=10&filter=withbody{key_param}",
            encode(q),
            encode(&first_tag),
            encode(site)
        );
        if let Ok(v) = get(&url) {
            if let Some(items) = v.get("items").and_then(Value::as_array) {
                questions.extend(items.iter().cloned());
            }
        }
    }
    // Deduplicate by question id.
    let mut seen = std::collections::HashSet::new();
    questions.retain(|q| seen.insert(q.get("question_id").and_then(Value::as_i64).unwrap_or(-1)));

    // Fetch the accepted (or best) answers in one batched call.
    let answer_ids: Vec<String> = questions
        .iter()
        .filter_map(|q| q.get("accepted_answer_id").and_then(Value::as_i64))
        .take(100)
        .map(|id| id.to_string())
        .collect();
    let mut answers = std::collections::HashMap::new();
    if !answer_ids.is_empty() {
        let url = format!(
            "{API}/answers/{}?order=desc&sort=votes&site={}&pagesize=100&filter=withbody{key_param}",
            answer_ids.join(";"),
            encode(site)
        );
        if let Ok(v) = get(&url) {
            for a in v
                .get("items")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if let (Some(id), Some(body)) = (
                    a.get("answer_id").and_then(Value::as_i64),
                    a.get("body").and_then(Value::as_str),
                ) {
                    answers.insert(id, body.to_string());
                }
            }
        }
    }

    let items: Vec<SeItem> = questions
        .iter()
        .map(|q| {
            let ans_id = q.get("accepted_answer_id").and_then(Value::as_i64);
            SeItem {
                title: html_to_text(q.get("title").and_then(Value::as_str).unwrap_or("")),
                link: q
                    .get("link")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                score: q.get("score").and_then(Value::as_i64).unwrap_or(0),
                question: truncate(
                    &html_to_text(q.get("body").and_then(Value::as_str).unwrap_or("")),
                    BODY_CHARS,
                ),
                answer: ans_id
                    .and_then(|id| answers.get(&id))
                    .map(|b| truncate(&html_to_text(b), BODY_CHARS))
                    .unwrap_or_default(),
            }
        })
        .collect();

    let raw: Vec<Value> = items
        .iter()
        .map(|i| serde_json::json!({"title": i.title, "link": i.link, "score": i.score, "question": i.question, "answer": i.answer}))
        .collect();
    let _ = std::fs::write(save_to, serde_json::to_vec_pretty(&raw)?);
    Ok(items)
}

pub fn digest(items: &[SeItem]) -> String {
    let mut out = String::new();
    for i in items {
        out.push_str(&format!(
            "### {} (score {}) {}\nQ: {}\n",
            i.title, i.score, i.link, i.question
        ));
        if !i.answer.is_empty() {
            out.push_str(&format!("Accepted answer: {}\n", i.answer));
        }
        out.push('\n');
    }
    out
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        return s.to_string();
    }
    let mut t: String = s.chars().take(n).collect();
    t.push('…');
    t
}

/// Crude but dependency-free HTML to text: drop tags, decode common entities,
/// collapse whitespace.
pub fn html_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => {
                in_tag = true;
                out.push(' ');
            }
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    let decoded = out
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&");
    decoded.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_is_flattened() {
        assert_eq!(
            html_to_text("<p>Use <code>a &lt; b</code> &amp;&amp; done</p>\n<pre>x</pre>"),
            "Use a < b && done x"
        );
    }

    #[test]
    fn url_encoding() {
        assert_eq!(encode("spring-boot"), "spring-boot");
        assert_eq!(encode("c# async"), "c%23%20async");
    }

    #[test]
    fn truncation_is_char_safe() {
        assert_eq!(truncate("héllo", 2), "hé…");
        assert_eq!(truncate("hi", 5), "hi");
    }

    #[test]
    fn no_site_means_no_fetch() {
        let hints = ResearchHints::default();
        let items = fetch(&hints, None, Path::new("/nonexistent/x.json")).unwrap();
        assert!(items.is_empty());
    }
}
