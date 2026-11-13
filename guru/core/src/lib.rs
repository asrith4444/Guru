//! guru-core: everything Guru does that is not drawing pixels.
//!
//! - `generator`: the "Download course" pipeline driving the Codex CLI
//! - `goal`:      goal → clarifying questions → professional stage roadmap
//! - `course`:    the offline course package and learner progress
//! - `sim`:       the simulation spec and its validator
//! - `scope`:     the beginner → advanced level bar and hour suggestions
//! - `srs`:       spaced repetition (SM-2)
//! - `machine`:   machine profile and resource budget
//! - `research`:  real-world problems from the Stack Exchange API

pub mod codex;
pub mod config;
pub mod course;
pub mod generator;
pub mod goal;
pub mod machine;
pub mod prompts;
pub mod research;
pub mod schemas;
pub mod scope;
pub mod sim;
pub mod srs;
