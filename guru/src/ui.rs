//! Minimal terminal UI: plain lines, a little color, no full-screen framework.
//! Reading happens in the learner's own pager/editor; Guru stays out of the way.

use std::io::BufRead;
use std::io::IsTerminal;
use std::io::Write;
use std::path::Path;
use std::process::Command;

fn color_enabled() -> bool {
    std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal()
}

fn paint(code: &str, s: &str) -> String {
    if color_enabled() {
        format!("\x1b[{code}m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

pub fn bold(s: &str) -> String {
    paint("1", s)
}
pub fn dim(s: &str) -> String {
    paint("2", s)
}
pub fn green(s: &str) -> String {
    paint("32", s)
}
pub fn yellow(s: &str) -> String {
    paint("33", s)
}
pub fn red(s: &str) -> String {
    paint("31", s)
}
pub fn cyan(s: &str) -> String {
    paint("36", s)
}

pub fn header(title: &str) {
    println!();
    println!("{}", bold(&cyan(&format!("━━ {title} "))));
}

pub fn phase(name: &str, minutes: u32, why: &str) {
    println!();
    println!(
        "{} {}",
        bold(&cyan(&format!("▶ {name}"))),
        dim(&format!("· ~{minutes} min"))
    );
    println!("{}", dim(why));
}

pub fn info(s: &str) {
    println!("{s}");
}

pub fn ok(s: &str) {
    println!("{} {s}", green("✓"));
}

pub fn warn(s: &str) {
    println!("{} {s}", yellow("!"));
}

pub fn bar(value: f32, width: usize) -> String {
    let v = value.clamp(0.0, 1.0);
    let filled = (v * width as f32).round() as usize;
    let s = format!("{}{}", "█".repeat(filled), "░".repeat(width - filled));
    if v >= 0.8 {
        green(&s)
    } else if v >= 0.5 {
        yellow(&s)
    } else {
        red(&s)
    }
}

/// Read one line from stdin. Returns None on EOF.
pub fn read_line(prompt: &str) -> Option<String> {
    print!("{prompt}");
    let _ = std::io::stdout().flush();
    let mut buf = String::new();
    match std::io::stdin().lock().read_line(&mut buf) {
        Ok(0) | Err(_) => None,
        Ok(_) => Some(buf.trim_end_matches(['\n', '\r']).to_string()),
    }
}

pub fn ask(prompt: &str) -> String {
    read_line(&format!("{} ", bold(prompt))).unwrap_or_default()
}

/// Multi-line answer: finishes on an empty line (or EOF).
pub fn ask_multiline(prompt: &str) -> String {
    println!("{} {}", bold(prompt), dim("(finish with an empty line)"));
    let mut lines = Vec::new();
    while let Some(line) = read_line("  ") {
        if line.trim().is_empty() {
            break;
        }
        lines.push(line);
    }
    lines.join("\n")
}

pub fn confirm(prompt: &str, default_yes: bool) -> bool {
    let hint = if default_yes { "[Y/n]" } else { "[y/N]" };
    match read_line(&format!("{} {} ", bold(prompt), dim(hint))) {
        None => default_yes,
        Some(a) => match a.trim().to_lowercase().as_str() {
            "" => default_yes,
            "y" | "yes" => true,
            _ => false,
        },
    }
}

/// Ask for an integer in [lo, hi]; empty input returns `default`.
pub fn ask_number(prompt: &str, lo: u32, hi: u32, default: u32) -> u32 {
    loop {
        let Some(a) = read_line(&format!(
            "{} {} ",
            bold(prompt),
            dim(&format!("[{lo}-{hi}, default {default}]"))
        )) else {
            return default;
        };
        let a = a.trim();
        if a.is_empty() {
            return default;
        }
        match a.parse::<u32>() {
            Ok(n) if (lo..=hi).contains(&n) => return n,
            _ => warn(&format!("Please enter a number between {lo} and {hi}.")),
        }
    }
}

/// Show a file to read. Uses $GURU_READER, then $PAGER, then `less -R`, then plain print.
pub fn read_file(path: &Path) {
    let interactive = std::io::stdout().is_terminal() && std::io::stdin().is_terminal();
    if interactive {
        let reader = std::env::var("GURU_READER")
            .or_else(|_| std::env::var("PAGER"))
            .ok()
            .filter(|s| !s.trim().is_empty());
        let (cmd, args): (String, Vec<String>) = match reader {
            Some(r) => {
                let mut parts = r.split_whitespace().map(String::from);
                let cmd = parts.next().unwrap_or_else(|| "less".into());
                (cmd, parts.collect())
            }
            None => ("less".into(), vec!["-R".into()]),
        };
        if Command::new(&cmd)
            .args(&args)
            .arg(path)
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            return;
        }
    }
    match std::fs::read_to_string(path) {
        Ok(text) => println!("{text}"),
        Err(e) => warn(&format!("Could not open {}: {e}", path.display())),
    }
}

pub fn fmt_minutes(secs: u64) -> String {
    let m = secs / 60;
    if m >= 60 {
        format!("{}h{:02}m", m / 60, m % 60)
    } else {
        format!("{m}m")
    }
}

/// Small activity indicator for long-running Codex work.
pub fn activity(label: &str, detail: &str) {
    let detail: String = detail.chars().take(90).collect();
    println!("  {} {}", dim(&format!("{label}:")), dim(&detail));
}
