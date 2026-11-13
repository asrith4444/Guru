//! The level bar: beginner → intermediate → advanced, mapped to study hours.
//!
//! One quick, low-effort Codex call sizes the topic (three anchor levels with
//! hours and what you can do at each); after that, moving the bar is pure
//! arithmetic, so the UI responds instantly and costs nothing.

use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Anchor {
    pub hours: f32,
    pub can_do: String,
}

impl Default for Anchor {
    fn default() -> Self {
        Self {
            hours: 1.0,
            can_do: String::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Scope {
    pub title: String,
    pub breadth: u8,
    pub beginner: Anchor,
    pub intermediate: Anchor,
    pub advanced: Anchor,
    pub simulated_tools: Vec<String>,
    pub note: String,
    /// True when produced without AI (offline fallback).
    pub estimated_offline: bool,
}

impl Scope {
    /// Offline fallback when Codex is unavailable: a generic, aggressive sizing.
    pub fn offline(topic: &str) -> Self {
        let words = topic.split_whitespace().count();
        let broad = [
            "devops",
            "engineering",
            "cloud",
            "security",
            "architecture",
            "language",
            "spanish",
            "french",
            "german",
            "japanese",
            "data",
        ]
        .iter()
        .any(|w| topic.to_lowercase().contains(w));
        let f = if broad {
            2.0
        } else if words > 3 {
            1.5
        } else {
            1.0
        };
        Self {
            title: topic.to_string(),
            breadth: if broad { 7 } else { 4 },
            beginner: Anchor {
                hours: 1.0 * f,
                can_do: "Understand the core ideas and complete common tasks with guidance.".into(),
            },
            intermediate: Anchor {
                hours: 2.5 * f,
                can_do:
                    "Work independently on typical real-world tasks and fix the common failures."
                        .into(),
            },
            advanced: Anchor {
                hours: 6.0 * f,
                can_do: "Handle complex, unusual situations, optimise, and teach others.".into(),
            },
            simulated_tools: vec![],
            note: "Estimated offline; sign in for a topic-specific estimate.".into(),
            estimated_offline: true,
        }
        .normalized()
    }

    /// Keep anchors positive and increasing, in 0.5 h steps.
    pub fn normalized(mut self) -> Self {
        let r = |h: f32| ((h.max(0.5) * 2.0).round() / 2.0).min(40.0);
        self.beginner.hours = r(self.beginner.hours);
        self.intermediate.hours = r(self.intermediate.hours).max(self.beginner.hours + 0.5);
        self.advanced.hours = r(self.advanced.hours).max(self.intermediate.hours + 0.5);
        self.breadth = self.breadth.clamp(1, 10);
        self
    }

    /// Suggested hours for a bar position 0..=100. Anchors sit at 33 (beginner),
    /// 66 (intermediate) and 100 (advanced); the far left is a quick taste.
    pub fn hours_for(&self, level: u8) -> f32 {
        let pts = [
            (0.0, (self.beginner.hours * 0.5).max(0.5)),
            (33.0, self.beginner.hours),
            (66.0, self.intermediate.hours),
            (100.0, self.advanced.hours),
        ];
        let x = f32::from(level.min(100));
        let mut h = pts[3].1;
        for w in pts.windows(2) {
            let ((x0, y0), (x1, y1)) = (w[0], w[1]);
            if x <= x1 {
                h = y0 + (y1 - y0) * (x - x0) / (x1 - x0);
                break;
            }
        }
        ((h * 2.0).round() / 2.0).max(0.5)
    }

    pub fn can_do(&self, level: u8) -> &str {
        match level {
            0..=49 => &self.beginner.can_do,
            50..=82 => &self.intermediate.can_do,
            _ => &self.advanced.can_do,
        }
    }
}

pub fn level_label(level: u8) -> &'static str {
    match level {
        0..=19 => "Quick start",
        20..=49 => "Beginner",
        50..=82 => "Intermediate",
        _ => "Advanced",
    }
}

/// About 45 minutes per module: long enough for a lesson and a simulation,
/// short enough to finish in one sitting.
pub fn modules_for(hours: f32) -> usize {
    ((hours * 60.0 / 45.0).ceil() as usize).clamp(1, 16)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope() -> Scope {
        Scope {
            beginner: Anchor {
                hours: 1.0,
                can_do: "b".into(),
            },
            intermediate: Anchor {
                hours: 3.0,
                can_do: "i".into(),
            },
            advanced: Anchor {
                hours: 6.0,
                can_do: "a".into(),
            },
            ..Default::default()
        }
    }

    #[test]
    fn bar_interpolates_between_anchors() {
        let s = scope();
        assert_eq!(s.hours_for(0), 0.5);
        assert_eq!(s.hours_for(33), 1.0);
        assert_eq!(s.hours_for(50), 2.0);
        assert_eq!(s.hours_for(66), 3.0);
        assert_eq!(s.hours_for(100), 6.0);
        assert_eq!(s.hours_for(255), 6.0);
        let mut last = 0.0;
        for l in 0..=100 {
            let h = s.hours_for(l);
            assert!(h >= last, "monotonic");
            assert_eq!((h * 2.0).fract(), 0.0, "half-hour steps");
            last = h;
        }
    }

    #[test]
    fn labels_and_modules() {
        assert_eq!(level_label(10), "Quick start");
        assert_eq!(level_label(66), "Intermediate");
        assert_eq!(level_label(100), "Advanced");
        assert_eq!(scope().can_do(70), "i");
        assert_eq!(modules_for(0.5), 1);
        assert_eq!(modules_for(2.0), 3);
        assert_eq!(modules_for(7.0), 10);
        assert_eq!(modules_for(40.0), 16);
    }

    #[test]
    fn normalization_fixes_bad_anchors() {
        let s = Scope {
            beginner: Anchor {
                hours: 3.2,
                ..Default::default()
            },
            intermediate: Anchor {
                hours: 1.0,
                ..Default::default()
            },
            advanced: Anchor {
                hours: 0.0,
                ..Default::default()
            },
            breadth: 99,
            ..Default::default()
        }
        .normalized();
        assert_eq!(s.beginner.hours, 3.0);
        assert_eq!(s.intermediate.hours, 3.5);
        assert_eq!(s.advanced.hours, 4.0);
        assert_eq!(s.breadth, 10);
        assert!(
            Scope::offline("DevOps").intermediate.hours > Scope::offline("Git").intermediate.hours
        );
    }
}
