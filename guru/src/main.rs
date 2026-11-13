//! Guru: learn any skill to working competence in ~7 focused hours.
//! Reading-first lessons, hands-on practice environments sized to your
//! machine, spaced retrieval and real-world break-fix problems, powered by the
//! Codex CLI (Sign in with ChatGPT).

mod codex;
mod commands;
mod config;
mod learn;
mod machine;
mod model;
mod prompts;
mod research;
mod schemas;
mod session;
mod srs;
mod ui;

use clap::Parser;
use clap::Subcommand;

#[derive(Parser)]
#[command(
    name = "guru",
    version,
    about = "Learn any skill to working competence in ~7 focused hours: read, practice, recall.",
    after_help = "Typical week:\n  guru learn \"Spring Boot\"   # once: goal, pretest, plan, issue bank\n  guru next                   # each day: one 60-minute session\n  guru review                 # any time: 5 minutes of spaced recall\n  guru status                 # mastery map and calibration"
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Start a new course on any topic.
    Learn {
        /// What you want to learn, e.g. "Spring Boot", "DevOps", "conversational Spanish".
        #[arg(required = true, num_args = 1..)]
        topic: Vec<String>,
        /// Number of sessions (default 7).
        #[arg(long)]
        hours: Option<u32>,
        /// Minutes per session (default 60).
        #[arg(long)]
        minutes: Option<u32>,
    },
    /// Run (or resume) the next one-hour session.
    #[command(alias = "session", alias = "start")]
    Next,
    /// Spaced retrieval practice of everything due.
    Review,
    /// Progress, mastery per skill and calibration.
    Status,
    /// Show the full plan.
    Plan,
    /// Ask your tutor a question (Socratic for lab tasks, direct for facts).
    Ask {
        #[arg(required = true, num_args = 1..)]
        question: Vec<String>,
    },
    /// Open a live tutoring session (interactive Codex with your learner model).
    Tutor,
    /// Check the current lab (automatic checks, optional AI feedback).
    Check,
    /// Get a realistic break-fix incident built from real-world issues.
    Incident,
    /// Stop lab services and report (or with --deep, reclaim) disk usage.
    Clean {
        #[arg(long)]
        deep: bool,
    },
    /// Write the end-of-course report (REPORT.md) and the next-level plan.
    Report,
    /// List your courses.
    List,
    /// Switch the active course.
    Use { slug: String },
    /// Check Codex, sign-in, and your machine's lab budget.
    Doctor,
    /// Sign in with ChatGPT (via Codex).
    Login,
    /// Show or set configuration (e.g. `guru config model gpt-5`).
    Config {
        key: Option<String>,
        value: Option<String>,
    },
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("{} {e:#}", ui::red("error:"));
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> anyhow::Result<()> {
    let cfg = config::Config::load();
    match cli.command {
        Cmd::Learn {
            topic,
            hours,
            minutes,
        } => {
            let hours = hours.unwrap_or(cfg.hours).clamp(1, 40);
            let minutes = minutes.unwrap_or(cfg.minutes_per_hour).clamp(20, 180);
            learn::run(&topic.join(" "), &cfg, hours, minutes)
        }
        Cmd::Next => session::run(&cfg),
        Cmd::Review => commands::review(),
        Cmd::Status => commands::status(),
        Cmd::Plan => commands::plan(),
        Cmd::Ask { question } => commands::ask(&cfg, &question.join(" ")),
        Cmd::Tutor => commands::tutor(&cfg),
        Cmd::Check => commands::check(&cfg),
        Cmd::Incident => commands::incident(&cfg),
        Cmd::Clean { deep } => commands::clean(deep),
        Cmd::Report => commands::report(&cfg),
        Cmd::List => commands::list(),
        Cmd::Use { slug } => commands::use_course(&slug),
        Cmd::Doctor => commands::doctor(&cfg),
        Cmd::Login => codex::Codex::from_config(&cfg).login(),
        Cmd::Config { key, value } => commands::config(key, value),
    }
}
