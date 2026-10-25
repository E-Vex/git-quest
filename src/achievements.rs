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

#[cfg(test)]
mod tests {
    use super::*;

    const DEF: AchievementDef = AchievementDef {
        emoji: "T",
        name: "Test",
        unit: "things",
        tiers: [2, 16, 128, 1024],
    };

    fn progress(current: u64) -> Progress {
        Progress { def: &DEF, current }
    }

    #[test]
    fn starts_at_the_default_tier() {
        let p = progress(0);
        assert_eq!(p.next_tier(), Some((Tier::Default, 2)));
        assert_eq!(p.remaining(), 2);
        assert_eq!(p.fraction(), 0.0);
    }

    #[test]
    fn just_below_a_threshold_keeps_the_same_target() {
        let p = progress(15);
        assert_eq!(p.next_tier(), Some((Tier::Bronze, 16)));
        assert_eq!(p.remaining(), 1);
    }

    #[test]
    fn reaching_a_threshold_advances_to_the_next_tier() {
        assert_eq!(progress(2).next_tier(), Some((Tier::Bronze, 16)));
        assert_eq!(progress(16).next_tier(), Some((Tier::Silver, 128)));
        assert_eq!(progress(128).next_tier(), Some((Tier::Gold, 1024)));
    }

    #[test]
    fn gold_is_terminal() {
        for current in [1024, 1025, u64::MAX] {
            let p = progress(current);
            assert_eq!(p.next_tier(), None);
            assert_eq!(p.remaining(), 0);
            assert_eq!(p.fraction(), 1.0);
        }
    }

    #[test]
    fn fraction_is_relative_to_the_next_threshold() {
        assert_eq!(progress(8).fraction(), 0.5);
        assert_eq!(progress(64).fraction(), 0.5);
    }

    #[test]
    fn fraction_never_leaves_unit_range() {
        for current in [0, 1, 2, 15, 16, 127, 128, 1023, 1024, 5000] {
            let f = progress(current).fraction();
            assert!((0.0..=1.0).contains(&f), "{current} -> {f}");
        }
    }

    #[test]
    fn real_definitions_have_strictly_increasing_tiers() {
        for def in [
            &PULL_SHARK,
            &STARSTRUCK,
            &GALAXY_BRAIN,
            &PAIR_EXTRAORDINAIRE,
        ] {
            assert!(
                def.tiers.windows(2).all(|w| w[0] < w[1]),
                "{} tiers are not strictly increasing: {:?}",
                def.name,
                def.tiers
            );
        }
    }

    #[test]
    fn tier_names_are_displayed() {
        let names: Vec<String> = Tier::ALL.iter().map(|t| t.to_string()).collect();
        assert_eq!(names, ["Default", "Bronze", "Silver", "Gold"]);
    }
}
