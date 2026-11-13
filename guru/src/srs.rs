//! Spaced retrieval practice.
//!
//! Uses the SM-2 algorithm (Wozniak, 1990) with a compressed second interval so
//! that a card learned in hour 1 of a 7-day course is retrieved on day 2 and
//! again around day 5, i.e. it is spaced *inside* the course and then keeps
//! expanding afterwards for long-term retention.

use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Card {
    pub id: String,
    pub skill_id: String,
    pub front: String,
    pub back: String,
    /// SM-2 easiness factor (>= 1.3).
    pub ease: f32,
    /// Current inter-repetition interval in days.
    pub interval_days: u32,
    /// Number of consecutive successful reviews.
    pub reps: u32,
    pub lapses: u32,
    /// Day number (days since the Unix epoch, local-agnostic) when the card is due.
    pub due_day: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grade {
    /// Could not recall.
    Again,
    /// Recalled with serious effort.
    Hard,
    /// Recalled correctly.
    Good,
    /// Instant, effortless recall.
    Easy,
}

impl Grade {
    fn quality(self) -> f32 {
        match self {
            Grade::Again => 1.0,
            Grade::Hard => 3.0,
            Grade::Good => 4.0,
            Grade::Easy => 5.0,
        }
    }
}

impl Card {
    pub fn new(id: String, skill_id: String, front: String, back: String, today: i64) -> Self {
        Self {
            id,
            skill_id,
            front,
            back,
            ease: 2.5,
            interval_days: 0,
            reps: 0,
            lapses: 0,
            due_day: today,
        }
    }

    pub fn is_due(&self, today: i64) -> bool {
        self.due_day <= today
    }

    /// Apply a review. Returns true when the card should be shown again in the
    /// same session (failed recall is retried before the session ends).
    pub fn review(&mut self, grade: Grade, today: i64) -> bool {
        let q = grade.quality();
        self.ease = (self.ease + (0.1 - (5.0 - q) * (0.08 + (5.0 - q) * 0.02))).max(1.3);
        if grade == Grade::Again {
            self.reps = 0;
            self.lapses += 1;
            self.interval_days = 1;
            self.due_day = today + 1;
            return true;
        }
        self.interval_days = match self.reps {
            0 => 1,
            1 => {
                if grade == Grade::Easy {
                    4
                } else {
                    3
                }
            }
            _ => {
                let factor = if grade == Grade::Hard { 1.2 } else { self.ease };
                ((self.interval_days.max(1) as f32) * factor).round() as u32
            }
        };
        if grade == Grade::Easy && self.reps == 0 {
            self.interval_days = 2;
        }
        self.interval_days = self.interval_days.clamp(1, 365);
        self.reps += 1;
        self.due_day = today + i64::from(self.interval_days);
        false
    }
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Day number used for scheduling. Respects `GURU_TODAY` for testing.
pub fn today() -> i64 {
    if let Ok(v) = std::env::var("GURU_TODAY") {
        if let Ok(d) = v.parse() {
            return d;
        }
    }
    (now_unix() / 86_400) as i64
}

/// Tiny xorshift RNG: enough for shuffling cards, no dependency needed.
pub struct Rng(u64);

impl Rng {
    pub fn seeded() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as u64 ^ d.as_secs())
            .unwrap_or(0x9E37_79B9);
        Self(nanos | 1)
    }

    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = (self.next() % (i as u64 + 1)) as usize;
            items.swap(i, j);
        }
    }
}

/// Order due cards for a review block: interleave skills (round-robin across
/// skills, shuffled within) instead of blocking them, since interleaved
/// retrieval improves discrimination between similar concepts.
pub fn interleave_due(cards: &[Card], today: i64, limit: usize, rng: &mut Rng) -> Vec<usize> {
    let mut by_skill: Vec<(String, Vec<usize>)> = Vec::new();
    for (i, c) in cards.iter().enumerate() {
        if !c.is_due(today) {
            continue;
        }
        match by_skill.iter_mut().find(|(s, _)| *s == c.skill_id) {
            Some((_, v)) => v.push(i),
            None => by_skill.push((c.skill_id.clone(), vec![i])),
        }
    }
    for (_, v) in by_skill.iter_mut() {
        // Most overdue / most lapsed first, then shuffled ties.
        rng.shuffle(v);
        v.sort_by_key(|&i| (cards[i].due_day, std::cmp::Reverse(cards[i].lapses)));
    }
    rng.shuffle(&mut by_skill);
    let mut out = Vec::new();
    let mut round = 0;
    loop {
        let mut any = false;
        for (_, v) in &by_skill {
            if let Some(&i) = v.get(round) {
                out.push(i);
                any = true;
                if out.len() >= limit {
                    return out;
                }
            }
        }
        if !any {
            return out;
        }
        round += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(skill: &str, due: i64) -> Card {
        Card::new(
            format!("{skill}-{due}"),
            skill.into(),
            "f".into(),
            "b".into(),
            due,
        )
    }

    #[test]
    fn good_reviews_expand_intervals_within_a_week() {
        let mut c = card("s", 0);
        assert!(!c.review(Grade::Good, 0));
        assert_eq!(c.due_day, 1);
        c.review(Grade::Good, 1);
        assert_eq!(c.due_day, 4);
        c.review(Grade::Good, 4);
        assert!(
            c.interval_days >= 7,
            "third interval should grow: {}",
            c.interval_days
        );
    }

    #[test]
    fn again_resets_and_requeues() {
        let mut c = card("s", 0);
        c.review(Grade::Good, 0);
        c.review(Grade::Good, 1);
        let ease_before = c.ease;
        assert!(c.review(Grade::Again, 4));
        assert_eq!(c.reps, 0);
        assert_eq!(c.lapses, 1);
        assert_eq!(c.due_day, 5);
        assert!(c.ease < ease_before);
        assert!(c.ease >= 1.3);
    }

    #[test]
    fn ease_never_drops_below_floor() {
        let mut c = card("s", 0);
        for d in 0..50 {
            c.review(Grade::Again, d);
        }
        assert!((c.ease - 1.3).abs() < 1e-6);
    }

    #[test]
    fn interleaving_alternates_skills() {
        let cards = vec![
            card("a", 0),
            card("a", 0),
            card("a", 0),
            card("b", 0),
            card("b", 0),
            card("c", 9),
        ];
        let mut rng = Rng(42);
        let order = interleave_due(&cards, 0, 10, &mut rng);
        assert_eq!(order.len(), 5, "card due on day 9 is excluded");
        // No two consecutive cards from the same skill while both skills remain.
        for w in order.windows(2).take(3) {
            assert_ne!(cards[w[0]].skill_id, cards[w[1]].skill_id);
        }
        assert_eq!(interleave_due(&cards, 0, 2, &mut rng).len(), 2);
    }
}
