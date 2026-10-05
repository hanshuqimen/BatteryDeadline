//! A greedy controller with a time deadband and a global observation interval.
use crate::{
    ActuatorKind, Capability, Confidence, ControllerState, Feasibility, Prediction, Settings,
};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct Decision {
    pub capability: Capability,
    pub value: u32,
    pub reason: String,
    pub relaxing: bool,
}

#[derive(Default)]
pub struct Controller {
    last_action: Option<DateTime<Utc>>,
    dwell_seconds: i64,
    risk_since: Option<DateTime<Utc>>,
    surplus_since: Option<DateTime<Utc>>,
    paused: HashMap<ActuatorKind, DateTime<Utc>>,
}

impl Controller {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn applied(&mut self, now: DateTime<Utc>, kind: ActuatorKind) {
        self.last_action = Some(now);
        self.dwell_seconds = if kind == ActuatorKind::Brightness {
            30
        } else {
            60
        };
        self.risk_since = None;
        self.surplus_since = None;
    }
    pub fn user_override(&mut self, now: DateTime<Utc>, kind: ActuatorKind) {
        self.paused
            .insert(kind, now + chrono::Duration::minutes(10));
    }
    pub fn failed(&mut self, now: DateTime<Utc>, kind: ActuatorKind) {
        self.user_override(now, kind);
    }

    pub fn evaluate(
        &mut self,
        now: DateTime<Utc>,
        prediction: &Prediction,
        settings: &Settings,
        caps: &[Capability],
        baselines: &HashMap<ActuatorKind, u32>,
        effects: &HashMap<ActuatorKind, f64>,
    ) -> (ControllerState, Option<Decision>) {
        if prediction.feasibility == Feasibility::Impossible {
            return (ControllerState::Critical, None);
        }
        let Some(margin) = prediction.deadline_margin_minutes else {
            return (ControllerState::Monitoring, None);
        };
        if prediction.confidence == Confidence::WarmingUp || prediction.sample_seconds < 30 {
            return (ControllerState::Monitoring, None);
        }
        let state = if margin < 0.0 {
            ControllerState::AtRisk
        } else {
            ControllerState::Stable
        };
        if (8.0..=22.0).contains(&margin) {
            self.risk_since = None;
            self.surplus_since = None;
            return (ControllerState::Stable, None);
        }
        // Percentage-only telemetry remains useful for prediction, but is too coarse for feedback.
        if prediction.estimated_power_w.is_none() {
            return (state, None);
        }
        if prediction.confidence == Confidence::Low && prediction.sample_seconds < 180 {
            return (ControllerState::Monitoring, None);
        }
        if self
            .last_action
            .is_some_and(|t| (now - t).num_seconds() < self.dwell_seconds)
        {
            return (ControllerState::Adjusting, None);
        }
        let relaxing = margin > 22.0;
        let since = if relaxing {
            self.risk_since = None;
            self.surplus_since.get_or_insert(now)
        } else {
            self.surplus_since = None;
            self.risk_since.get_or_insert(now)
        };
        if (now - *since).num_seconds() < if relaxing { 90 } else { 20 } {
            return (state, None);
        }
        let mut options = Vec::new();
        for cap in caps {
            if !cap.supported || self.paused.get(&cap.kind).is_some_and(|until| now < *until) {
                continue;
            }
            let Some(current) = cap.current else { continue };
            let baseline = baselines.get(&cap.kind).copied().unwrap_or(current);
            let allowed = match cap.kind {
                ActuatorKind::Brightness => settings.allow_brightness,
                ActuatorKind::RefreshRate => settings.allow_refresh,
                ActuatorKind::CpuPolicy => settings.allow_cpu,
            };
            if !allowed {
                continue;
            }
            let floor = match cap.kind {
                ActuatorKind::Brightness => u32::from(settings.min_brightness),
                ActuatorKind::RefreshRate => settings.min_refresh_hz,
                ActuatorKind::CpuPolicy => u32::from(settings.min_cpu_percent),
            };
            let value = match cap.kind {
                ActuatorKind::RefreshRate => {
                    if relaxing {
                        cap.values
                            .iter()
                            .copied()
                            .filter(|v| *v > current && *v <= baseline)
                            .min()
                    } else {
                        cap.values
                            .iter()
                            .copied()
                            .filter(|v| *v < current && *v >= floor)
                            .max()
                    }
                }
                _ => {
                    if relaxing {
                        (current < baseline).then_some(current.saturating_add(10).min(baseline))
                    } else {
                        (current > floor).then_some(current.saturating_sub(10).max(floor))
                    }
                }
            };
            let value = if cap.kind == ActuatorKind::Brightness && !cap.values.is_empty() {
                value.and_then(|requested| {
                    if relaxing {
                        cap.values
                            .iter()
                            .copied()
                            .filter(|v| *v > current && *v <= requested && *v <= baseline)
                            .max()
                    } else {
                        cap.values
                            .iter()
                            .copied()
                            .filter(|v| *v < current && *v >= requested && *v >= floor)
                            .min()
                    }
                })
            } else {
                value
            };
            if let Some(value) = value {
                let cost = match cap.kind {
                    ActuatorKind::Brightness => 1.0,
                    ActuatorKind::RefreshRate => 2.0,
                    ActuatorKind::CpuPolicy => {
                        if current <= 80 {
                            5.0
                        } else {
                            3.0
                        }
                    }
                };
                let score = if relaxing {
                    match cap.kind {
                        ActuatorKind::CpuPolicy => 0.0,
                        ActuatorKind::RefreshRate => 1.0,
                        ActuatorKind::Brightness => 2.0,
                    }
                } else {
                    effects
                        .get(&cap.kind)
                        .filter(|e| **e > 0.1)
                        .map_or(cost, |saving| cost / saving)
                };
                options.push((score, cap.clone(), value));
            }
        }
        options.sort_by(|a, b| a.0.total_cmp(&b.0));
        let decision=options.into_iter().next().map(|(_,capability,value)| Decision {capability,value,relaxing,
            reason:if relaxing {format!("Recent usage leaves {:.0} minutes of margin. Restoring some comfort gradually.",margin)}
                else if margin<0.0 {format!("Recent usage predicts reaching your reserve {:.0} minutes before the deadline.",-margin)}
                else {format!("The {:.0}-minute margin is below the 8-minute safety threshold.",margin)}});
        (
            if decision.is_some() {
                ControllerState::Adjusting
            } else {
                state
            },
            decision,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p(margin: f64) -> Prediction {
        Prediction {
            deadline_margin_minutes: Some(margin),
            estimated_power_w: Some(14.0),
            confidence: Confidence::High,
            sample_seconds: 100,
            feasibility: Feasibility::Tight,
            ..Prediction::default()
        }
    }
    fn caps() -> Vec<Capability> {
        vec![Capability {
            kind: ActuatorKind::Brightness,
            target: "panel".into(),
            supported: true,
            current: Some(75),
            values: vec![],
            explanation: String::new(),
        }]
    }
    #[test]
    fn hysteresis_and_sustained_deficit() {
        let mut c = Controller::default();
        let now = Utc::now();
        let s = Settings::default();
        let b = HashMap::new();
        let e = HashMap::new();
        for i in 0..100 {
            let (_, d) = c.evaluate(
                now + chrono::Duration::seconds(i),
                &p(if i % 2 == 0 { 14.0 } else { 16.0 }),
                &s,
                &caps(),
                &b,
                &e,
            );
            assert!(d.is_none());
        }
        assert!(
            c.evaluate(
                now + chrono::Duration::seconds(100),
                &p(-30.0),
                &s,
                &caps(),
                &b,
                &e
            )
            .1
            .is_none()
        );
        let d = c
            .evaluate(
                now + chrono::Duration::seconds(121),
                &p(-30.0),
                &s,
                &caps(),
                &b,
                &e,
            )
            .1
            .unwrap();
        assert_eq!(d.value, 65);
        c.applied(now + chrono::Duration::seconds(121), d.capability.kind);
        assert!(
            c.evaluate(
                now + chrono::Duration::seconds(125),
                &p(-30.0),
                &s,
                &caps(),
                &b,
                &e
            )
            .1
            .is_none()
        );
    }
    #[test]
    fn surplus_restores_only_up_to_baseline_and_override_pauses() {
        let mut c = Controller::default();
        let now = Utc::now();
        let b = HashMap::from([(ActuatorKind::Brightness, 80)]);
        let mut caps = caps();
        caps[0].current = Some(75);
        c.evaluate(
            now,
            &p(60.0),
            &Settings::default(),
            &caps,
            &b,
            &HashMap::new(),
        );
        assert_eq!(
            c.evaluate(
                now + chrono::Duration::seconds(91),
                &p(60.0),
                &Settings::default(),
                &caps,
                &b,
                &HashMap::new()
            )
            .1
            .unwrap()
            .value,
            80
        );
        c.user_override(now, ActuatorKind::Brightness);
        assert!(
            c.evaluate(
                now + chrono::Duration::seconds(92),
                &p(60.0),
                &Settings::default(),
                &caps,
                &b,
                &HashMap::new()
            )
            .1
            .is_none()
        );
    }
}
