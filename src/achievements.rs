use std::fmt;

/// A badge tier. `Default` is the base badge GitHub awards at the first
/// threshold; Bronze, Silver and Gold are the upgrades on top of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    Default,
    Bronze,
    Silver,
    Gold,
}

impl Tier {
    /// Tiers in ascending order of difficulty.
    pub const ALL: [Tier; 4] = [Tier::Default, Tier::Bronze, Tier::Silver, Tier::Gold];

    /// Human-readable tier name.
    pub fn name(self) -> &'static str {
        match self {
            Tier::Default => "Default",
            Tier::Bronze => "Bronze",
            Tier::Silver => "Silver",
            Tier::Gold => "Gold",
        }
    }
}

impl fmt::Display for Tier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Static definition of a GitHub profile achievement.
pub struct AchievementDef {
    pub emoji: &'static str,
    pub name: &'static str,
    /// Short unit used in the remaining-count line.
    pub unit: &'static str,
    /// Thresholds for [Default, Bronze, Silver, Gold].
    pub tiers: [u64; 4],
}

// Tier thresholds. GitHub does not publish these officially; the values
// below are the community-documented ones (Default / Bronze / Silver / Gold).

pub const PULL_SHARK: AchievementDef = AchievementDef {
    emoji: "🦈",
    name: "Pull Shark",
    unit: "merged PRs",
    tiers: [2, 16, 128, 1024],
};

pub const STARSTRUCK: AchievementDef = AchievementDef {
    emoji: "⭐",
    name: "Starstruck",
    unit: "stars",
    tiers: [16, 128, 512, 4096],
};

pub const GALAXY_BRAIN: AchievementDef = AchievementDef {
    emoji: "🧠",
    name: "Galaxy Brain",
    unit: "accepted answers",
    tiers: [2, 8, 16, 32],
};

pub const PAIR_EXTRAORDINAIRE: AchievementDef = AchievementDef {
    emoji: "👥",
    name: "Pair Extraordinaire",
    unit: "co-authored commits",
    tiers: [1, 10, 24, 48],
};

/// Live progress of one achievement.
pub struct Progress {
    pub def: &'static AchievementDef,
    pub current: u64,
}

impl Progress {
    /// The next tier not yet reached: `(tier, threshold)`.
    /// `None` means Gold is complete.
    pub fn next_tier(&self) -> Option<(Tier, u64)> {
        Tier::ALL
            .into_iter()
            .zip(self.def.tiers)
            .find(|&(_, threshold)| self.current < threshold)
    }

    /// Remaining count to the next tier (0 when all tiers are done).
    pub fn remaining(&self) -> u64 {
        match self.next_tier() {
            Some((_, threshold)) => threshold - self.current,
            None => 0,
        }
    }

    /// Progress fraction toward the next tier, clamped to [0, 1].
    pub fn fraction(&self) -> f64 {
        match self.next_tier() {
            Some((_, threshold)) => (self.current as f64 / threshold as f64).clamp(0.0, 1.0),
            None => 1.0,
        }
    }
}
