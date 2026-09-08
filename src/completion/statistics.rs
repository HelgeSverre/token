//! Local aggregate outcomes, never source text or provider connection settings.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Accepted,
    Dismissed,
    TypedThrough,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageEvent {
    pub provider: String,
    pub outcome: Outcome,
}

/// One response, regardless of its alternative count or partial accept count.
#[derive(Debug, Clone)]
pub(crate) struct Observation {
    provider: String,
    state: ObservationState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ObservationState {
    Pending,
    Offered,
    Resolved,
}

impl Observation {
    pub fn new(provider: String) -> Self {
        Self {
            provider,
            state: ObservationState::Pending,
        }
    }

    pub fn offered(&mut self) {
        if self.state == ObservationState::Pending {
            self.state = ObservationState::Offered;
        }
    }

    pub fn resolve(&mut self, outcome: Outcome) -> Option<UsageEvent> {
        if self.state != ObservationState::Offered {
            return None;
        }
        self.state = ObservationState::Resolved;
        Some(UsageEvent {
            provider: self.provider.clone(),
            outcome,
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderCounts {
    pub accepted: u64,
    pub dismissed: u64,
    pub typed_through: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Statistics {
    pub version: u32,
    pub providers: BTreeMap<String, ProviderCounts>,
}

impl Default for Statistics {
    fn default() -> Self {
        Self {
            version: 1,
            providers: BTreeMap::new(),
        }
    }
}

impl Statistics {
    pub fn record(&mut self, event: &UsageEvent) {
        let counts = self.providers.entry(event.provider.clone()).or_default();
        let counter = match event.outcome {
            Outcome::Accepted => &mut counts.accepted,
            Outcome::Dismissed => &mut counts.dismissed,
            Outcome::TypedThrough => &mut counts.typed_through,
        };
        *counter = counter.saturating_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observation_counts_one_terminal_outcome_only_after_an_offer() {
        let mut observation = Observation::new("local".into());
        assert!(observation.resolve(Outcome::Dismissed).is_none());
        observation.offered();
        observation.offered();
        assert_eq!(
            observation.resolve(Outcome::Accepted),
            Some(UsageEvent {
                provider: "local".into(),
                outcome: Outcome::Accepted,
            })
        );
        observation.offered();
        assert!(observation.resolve(Outcome::Accepted).is_none());
        assert!(observation.resolve(Outcome::Dismissed).is_none());
    }

    #[test]
    fn statistics_json_contains_only_version_provider_names_and_saturating_counts() {
        let mut statistics = Statistics::default();
        statistics.providers.insert(
            "local".into(),
            ProviderCounts {
                accepted: u64::MAX,
                dismissed: 0,
                typed_through: 0,
            },
        );
        for outcome in [Outcome::Accepted, Outcome::Dismissed, Outcome::TypedThrough] {
            statistics.record(&UsageEvent {
                provider: "local".into(),
                outcome,
            });
        }
        assert_eq!(
            serde_json::to_value(&statistics).unwrap(),
            serde_json::json!({
                "version": 1,
                "providers": { "local": { "accepted": u64::MAX, "dismissed": 1, "typed_through": 1 } }
            })
        );
    }
}
