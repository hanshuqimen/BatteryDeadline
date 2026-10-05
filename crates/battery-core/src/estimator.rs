//! Robust, time-aware estimation. Missing information stays missing all the way to the UI.
use crate::{Confidence, Feasibility, Prediction, Telemetry, TelemetryQuality};
use chrono::{DateTime, Duration, Utc};
use std::collections::VecDeque;

#[derive(Default)]
pub struct Estimator {
    samples: VecDeque<Telemetry>,
    medians: VecDeque<(DateTime<Utc>, f64)>,
    ewma: Option<f64>,
    minimum: Option<f64>,
}

impl Estimator {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn ingest(&mut self, sample: Telemetry) {
        if sample.ac_connected != Some(false) || !sample.battery_present {
            self.reset();
            return;
        }
        if let Some(previous) = self.samples.back() {
            let gap = (sample.timestamp - previous.timestamp).num_seconds();
            if gap <= 0 {
                return;
            }
            if gap > 15 || sample.battery_identity != previous.battery_identity {
                self.reset();
            }
        }
        self.samples.push_back(sample);
        let Some(latest) = self.samples.back() else {
            return;
        };
        let now = latest.timestamp;
        while self
            .samples
            .front()
            .is_some_and(|s| (now - s.timestamp).num_seconds() > 300)
        {
            self.samples.pop_front();
        }

        let raw: Vec<f64> = self
            .samples
            .iter()
            .rev()
            .take_while(|s| (now - s.timestamp).num_seconds() < 20)
            .filter_map(|s| {
                s.discharge_w
                    .filter(|w| w.is_finite() && (0.1..=300.0).contains(w))
            })
            .collect();
        let power = if raw.len() >= 5 {
            median(raw)
        } else {
            self.slope(false).filter(|w| *w <= 300.0)
        };
        if let Some(power) = power {
            let dt = self.medians.back().map_or(1.0, |(ts, _)| {
                (now - *ts).num_milliseconds() as f64 / 1000.0
            });
            let alpha = 1.0 - (-dt / 75.0).exp();
            self.ewma = Some(self.ewma.map_or(power, |old| old + alpha * (power - old)));
            self.medians.push_back((now, power));
            while self
                .medians
                .front()
                .is_some_and(|(ts, _)| (now - *ts).num_seconds() > 120)
            {
                self.medians.pop_front();
            }
            if self.elapsed_seconds() >= 60 {
                let average =
                    self.medians.iter().map(|(_, p)| *p).sum::<f64>() / self.medians.len() as f64;
                self.minimum = Some(self.minimum.map_or(average, |old| old.min(average)));
            }
        }
    }

    fn elapsed_seconds(&self) -> i64 {
        self.samples
            .front()
            .zip(self.samples.back())
            .map_or(0, |(a, b)| (b.timestamp - a.timestamp).num_seconds())
    }

    /// Least-squares slope reduces quantization noise. A minute and a measurable drop are required.
    fn slope(&self, percentage: bool) -> Option<f64> {
        let first = self.samples.front()?;
        let last = self.samples.back()?;
        if (last.timestamp - first.timestamp).num_seconds() < 60 {
            return None;
        }
        let points: Vec<(f64, f64)> = self
            .samples
            .iter()
            .filter_map(|s| {
                let y = if percentage {
                    s.percentage
                } else {
                    s.remaining_wh
                }?;
                y.is_finite().then_some((
                    (s.timestamp - first.timestamp).num_milliseconds() as f64 / 3_600_000.0,
                    y,
                ))
            })
            .collect();
        if points.len() < 10 {
            return None;
        }
        let drop = points.first()?.1 - points.last()?.1;
        if drop < if percentage { 1.0 } else { 0.02 } {
            return None;
        }
        let n = points.len() as f64;
        let mx = points.iter().map(|p| p.0).sum::<f64>() / n;
        let my = points.iter().map(|p| p.1).sum::<f64>() / n;
        let denominator = points.iter().map(|p| (p.0 - mx).powi(2)).sum::<f64>();
        if denominator <= 0.0 {
            return None;
        }
        let slope = -points.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum::<f64>() / denominator;
        (slope.is_finite() && slope > 0.01).then_some(slope)
    }

    pub fn predict(
        &self,
        telemetry: &Telemetry,
        deadline: Option<DateTime<Utc>>,
        reserve_percent: u8,
    ) -> Prediction {
        let now = telemetry.timestamp;
        let seconds = self.elapsed_seconds();
        let usable_wh = telemetry
            .remaining_wh
            .zip(telemetry.full_charge_wh)
            .filter(|(current, full)| {
                current.is_finite()
                    && full.is_finite()
                    && *full > 0.0
                    && *current >= 0.0
                    && *current <= *full * 1.1
            })
            .map(|(current, full)| current - full * f64::from(reserve_percent) / 100.0);
        let usable_percent = telemetry
            .percentage
            .filter(|p| p.is_finite() && (0.0..=100.0).contains(p))
            .map(|p| p - f64::from(reserve_percent));
        let hours = deadline
            .map(|d| (d - now).num_milliseconds() as f64 / 3_600_000.0)
            .filter(|h| *h > 0.0);
        let budget = usable_wh.zip(hours).map(|(e, h)| e.max(0.0) / h);
        let power = (seconds >= 20).then_some(self.ewma).flatten();
        let pct_rate = self.slope(true);
        let remaining = if telemetry.ac_connected != Some(false) {
            None
        } else if let Some((e, w)) = usable_wh.zip(power) {
            Some(e.max(0.0) / w * 60.0)
        } else {
            usable_percent
                .zip(pct_rate)
                .map(|(p, rate)| p.max(0.0) / rate * 60.0)
        };
        let until = remaining
            .filter(|m| m.is_finite() && *m < 60.0 * 24.0 * 365.0)
            .map(|m| now + Duration::milliseconds((m * 60_000.0) as i64));
        let margin = until
            .zip(deadline)
            .map(|(u, d)| (u - d).num_milliseconds() as f64 / 60_000.0);
        let variance = if self.medians.len() >= 10 {
            let n = self.medians.len() as f64;
            let mean = self.medians.iter().map(|(_, w)| *w).sum::<f64>() / n;
            let std = (self
                .medians
                .iter()
                .map(|(_, w)| (*w - mean).powi(2))
                .sum::<f64>()
                / n)
                .sqrt();
            (mean > 0.0).then_some(std / mean)
        } else {
            None
        };
        let confidence = if seconds < 20 || remaining.is_none() {
            Confidence::WarmingUp
        } else if telemetry.discharge_w.is_none()
            || telemetry.quality == TelemetryQuality::Poor
            || power.is_none()
        {
            Confidence::Low
        } else if seconds < 60 || variance.is_some_and(|v| v > 0.25) {
            Confidence::Medium
        } else {
            Confidence::High
        };
        let impossible =
            usable_wh.is_some_and(|e| e <= 0.0) || usable_percent.is_some_and(|p| p <= 0.0);
        let feasibility = if impossible {
            Feasibility::Impossible
        } else if budget.zip(self.minimum).is_some_and(|(b, m)| b < m * 0.75) {
            Feasibility::Unlikely
        } else {
            match margin {
                Some(m) if m >= 45.0 => Feasibility::Easy,
                Some(m) if m >= 8.0 => Feasibility::Possible,
                Some(m) if m >= -15.0 => Feasibility::Tight,
                Some(_) => Feasibility::Unlikely,
                None => Feasibility::Unknown,
            }
        };
        let explanation = if telemetry.ac_connected==Some(true) { "Plugged in. Settings are restored." }
            else if telemetry.ac_connected.is_none() { "Power source is unknown. Automatic adjustments are paused." }
            else if impossible { "The battery is already at or below your reserve. Choose an earlier deadline or connect power." }
            else if confidence==Confidence::WarmingUp { "Collecting a stable reading. Capacity-based estimates may need several minutes." }
            else if power.is_none() { "Estimated from battery percentage. Real-time watts are unavailable." }
            else if telemetry.discharge_w.is_none() { "Estimated from the change in battery capacity. Confidence is limited." }
            else if variance.is_some_and(|v| v>0.25) { "Your workload is changing. The prediction will settle as usage stabilizes." }
            else { "Based on recent usage, with your reserve kept available." }.into();
        Prediction {
            estimated_power_w: power,
            power_budget_w: budget,
            predicted_remaining_minutes: remaining,
            predicted_depletion_time: until,
            deadline_margin_minutes: margin,
            need_to_save_w: power.zip(budget).map(|(p, b)| p - b),
            confidence,
            feasibility,
            sample_seconds: seconds,
            coefficient_of_variation: variance,
            observed_minimum_w: self.minimum,
            explanation,
        }
    }
}

fn median(mut values: Vec<f64>) -> Option<f64> {
    values.sort_by(f64::total_cmp);
    let n = values.len();
    if n == 0 {
        None
    } else if n.is_multiple_of(2) {
        Some((values[n / 2 - 1] + values[n / 2]) / 2.0)
    } else {
        Some(values[n / 2])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Hardware;
    use crate::simulator::FakeHardware;
    #[test]
    fn correct_budget_and_reserve_time() {
        let mut fake = FakeHardware::default();
        let now = Utc::now();
        let mut t = fake.telemetry(now).unwrap();
        t.remaining_wh = Some(40.0);
        t.full_charge_wh = Some(100.0);
        let p = Estimator::default().predict(&t, Some(now + Duration::hours(3)), 10);
        assert_eq!(p.power_budget_w, Some(10.0));
        t.remaining_wh = Some(9.0);
        assert_eq!(
            Estimator::default()
                .predict(&t, Some(now + Duration::hours(3)), 10)
                .feasibility,
            Feasibility::Impossible
        );
    }
    #[test]
    fn isolated_spike_rejected_but_sustained_workload_recognized() {
        let mut fake = FakeHardware::default();
        let now = Utc::now();
        let mut e = Estimator::default();
        for i in 0..100 {
            let mut t = fake.telemetry(now + Duration::seconds(i)).unwrap();
            t.discharge_w = Some(if i == 70 { 100.0 } else { 10.0 });
            e.ingest(t);
        }
        assert!((e.ewma.unwrap() - 10.0).abs() < 0.1);
        for i in 100..280 {
            let mut t = fake.telemetry(now + Duration::seconds(i)).unwrap();
            t.discharge_w = Some(35.0);
            e.ingest(t);
        }
        assert!(e.ewma.unwrap() > 30.0);
    }
    #[test]
    fn energy_slope_and_percentage_fallback_never_invent_watts() {
        let now = Utc::now();
        let mut fake = FakeHardware::default();
        let mut e = Estimator::default();
        for i in 0..240 {
            let mut t = fake.telemetry(now + Duration::seconds(i)).unwrap();
            t.discharge_w = None;
            t.remaining_wh = Some(40.0 - i as f64 * 12.0 / 3600.0);
            e.ingest(t);
        }
        assert!((e.ewma.unwrap() - 12.0).abs() < 0.01);
        e.reset();
        let mut last = fake.telemetry(now).unwrap();
        for i in 0..240 {
            last.timestamp = now + Duration::seconds(i);
            last.discharge_w = None;
            last.remaining_wh = None;
            last.full_charge_wh = None;
            last.percentage = Some(70.0 - i as f64 * 20.0 / 3600.0);
            e.ingest(last.clone());
        }
        let p = e.predict(&last, Some(now + Duration::hours(3)), 10);
        assert!(p.estimated_power_w.is_none());
        assert!(p.power_budget_w.is_none());
        assert!(p.predicted_remaining_minutes.is_some());
        assert_eq!(p.confidence, Confidence::Low);
    }
    #[test]
    fn ac_gap_and_battery_replacement_reset_estimates() {
        let now = Utc::now();
        let mut f = FakeHardware::default();
        let mut e = Estimator::default();
        for i in 0..70 {
            e.ingest(f.telemetry(now + Duration::seconds(i)).unwrap());
        }
        let t = f.telemetry(now + Duration::seconds(100)).unwrap();
        e.ingest(t.clone());
        assert_eq!(e.predict(&t, None, 10).confidence, Confidence::WarmingUp);
        let mut ac = t;
        ac.ac_connected = Some(true);
        e.ingest(ac);
        assert!(e.samples.is_empty());
    }
}
