use crate::sensor_model::{SensorId, metadata, sensor_id};
use gpu_shark::SensorReading;
use std::collections::{HashMap, HashSet, VecDeque};

const ROW_SAMPLE_LIMIT: usize = 120;

#[derive(Clone, Copy, Debug)]
pub struct SensorStats {
    pub current: f32,
    pub min: f32,
    pub max: f32,
    #[allow(dead_code)]
    pub avg: f32,
}

impl SensorStats {
    #[allow(dead_code)]
    pub fn initial(value: f32) -> Self {
        Self {
            current: value,
            min: value,
            max: value,
            avg: value,
        }
    }
}

#[derive(Clone, Debug, Default)]
struct RowRecord {
    samples: VecDeque<f32>,
    min: f32,
    max: f32,
    sum: f64,
    count: u64,
}

impl RowRecord {
    fn push(&mut self, value: f32) {
        if self.samples.is_empty() {
            self.min = value;
            self.max = value;
        }
        self.samples.push_back(value);
        while self.samples.len() > ROW_SAMPLE_LIMIT {
            self.samples.pop_front();
        }
        self.min = self.min.min(value);
        self.max = self.max.max(value);
        self.sum += f64::from(value);
        self.count += 1;
    }

    fn stats(&self) -> Option<SensorStats> {
        let current = *self.samples.back()?;
        let avg = if self.count > 0 {
            (self.sum / self.count as f64) as f32
        } else {
            current
        };
        Some(SensorStats {
            current,
            min: self.min,
            max: self.max,
            avg,
        })
    }
}

#[derive(Default)]
pub struct SensorHistory {
    selected: Option<SensorId>,
    tracked: HashSet<SensorId>,
    rows: HashMap<SensorId, RowRecord>,
}

impl SensorHistory {
    pub fn select(&mut self, sensor: &SensorReading) {
        self.selected = Some(sensor_id(sensor));
    }

    #[allow(dead_code)]
    pub fn clear_selection(&mut self) {
        self.selected = None;
    }

    /// Double-click toggles per-sensor maximum tracking. Tracked sensors keep
    /// their session maximum visible and recorded independently of selection.
    pub fn select_maximum(&mut self, sensor: &SensorReading) {
        let id = sensor_id(sensor);
        if metadata(sensor).graphable && !self.tracked.remove(&id) {
            self.tracked.insert(id.clone());
        }
        self.selected = Some(id);
    }

    /// Tracks every graphable sensor, as if each row had been double-clicked.
    #[allow(dead_code)]
    pub fn track_all(&mut self, sensors: &[SensorReading]) {
        for sensor in sensors {
            if metadata(sensor).graphable {
                self.tracked.insert(sensor_id(sensor));
            }
        }
    }

    #[allow(dead_code)]
    pub fn is_tracked(&self, id: &SensorId) -> bool {
        self.tracked.contains(id)
    }

    #[allow(dead_code)]
    pub fn shows_maximum(&self) -> bool {
        self.selected
            .as_ref()
            .is_some_and(|selected| self.tracked.contains(selected))
    }

    pub fn selected_id(&self) -> Option<&SensorId> {
        self.selected.as_ref()
    }

    pub fn record(&mut self, sensors: &[SensorReading]) {
        for sensor in sensors {
            if !metadata(sensor).graphable {
                continue;
            }
            self.rows
                .entry(sensor_id(sensor))
                .or_default()
                .push(sensor.value);
        }
    }

    #[allow(dead_code)]
    pub fn stats(&self) -> Option<SensorStats> {
        self.selected_id().and_then(|id| self.row_stats(id))
    }

    #[allow(dead_code)]
    pub fn samples(&self) -> Vec<f32> {
        self.selected_id()
            .map(|id| self.row_samples(id))
            .unwrap_or_default()
    }

    pub fn row_samples(&self, id: &SensorId) -> Vec<f32> {
        self.rows
            .get(id)
            .map(|row| row.samples.iter().copied().collect())
            .unwrap_or_default()
    }

    pub fn row_stats(&self, id: &SensorId) -> Option<SensorStats> {
        self.rows.get(id).and_then(|row| row.stats())
    }

    /// Clears every recorded row and tracking state. Selection is preserved.
    pub fn reset(&mut self) {
        self.rows.clear();
        self.tracked.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reading(name: &str, value: f32) -> SensorReading {
        SensorReading {
            name: name.to_owned(),
            value,
            unit: "°C".to_owned(),
        }
    }

    fn perfcap_reading() -> SensorReading {
        SensorReading {
            name: "PerfCap Reason".to_owned(),
            value: 0.0,
            unit: "Pwr, VRel".to_owned(),
        }
    }

    #[test]
    fn history_survives_a_known_provider_alias_change() {
        let mut history = SensorHistory::default();
        history.record(&[reading("GPU Core", 40.0)]);
        history.record(&[reading("GPU Core Temperature", 52.0)]);

        let id = sensor_id(&reading("GPU Core", 40.0));
        let stats = history.row_stats(&id).expect("merged alias stats");
        assert_eq!(stats.current, 52.0);
        assert_eq!(stats.min, 40.0);
        assert_eq!(stats.max, 52.0);
        assert_eq!(stats.avg, 46.0);
        assert_eq!(history.row_samples(&id), vec![40.0, 52.0]);
    }

    #[test]
    fn per_sensor_history_persists_across_selection_changes() {
        let core = reading("GPU Core", 40.0);
        let fan = SensorReading {
            name: "GPU Fan 1".to_owned(),
            value: 1_000.0,
            unit: "RPM".to_owned(),
        };
        let mut history = SensorHistory::default();
        history.select(&core);
        history.record(&[core.clone(), fan.clone()]);
        history.record(&[reading("GPU Core", 41.0)]);
        history.select(&fan);

        assert_eq!(history.selected_id(), Some(&sensor_id(&fan)));
        assert_eq!(
            history.row_samples(&sensor_id(&core)),
            vec![40.0, 41.0],
            "switching selection must not clear another sensor's history"
        );
        assert_eq!(
            history.row_stats(&sensor_id(&fan)).expect("fan stats").min,
            1000.0
        );
    }

    #[test]
    fn maximum_tracking_is_independent_of_selection() {
        let core = reading("GPU Core", 40.0);
        let fan = SensorReading {
            name: "GPU Fan 1".to_owned(),
            value: 1_000.0,
            unit: "RPM".to_owned(),
        };
        let mut history = SensorHistory::default();
        history.select_maximum(&core);
        history.record(&[core.clone()]);
        history.select(&fan);

        assert!(
            history.is_tracked(&sensor_id(&core)),
            "tracking must survive switching to another sensor"
        );
        assert_eq!(history.row_stats(&sensor_id(&core)).unwrap().max, 40.0);

        history.select_maximum(&fan);
        assert!(history.is_tracked(&sensor_id(&fan)));

        history.select_maximum(&core);
        assert!(
            !history.is_tracked(&sensor_id(&core)),
            "double-click on a tracked sensor removes its tracking"
        );
        assert!(history.is_tracked(&sensor_id(&fan)));
    }

    #[test]
    fn categorical_perfcap_selection_has_no_numeric_history() {
        let perfcap = perfcap_reading();
        let mut history = SensorHistory::default();

        history.select_maximum(&perfcap);
        history.record(std::slice::from_ref(&perfcap));

        assert_eq!(history.selected_id(), Some(&sensor_id(&perfcap)));
        assert!(!history.is_tracked(&sensor_id(&perfcap)));
        assert!(history.row_stats(&sensor_id(&perfcap)).is_none());
        assert!(history.row_samples(&sensor_id(&perfcap)).is_empty());
    }

    #[test]
    fn reset_clears_rows_and_tracking_but_keeps_selection() {
        let core = reading("GPU Core", 48.0);
        let mut history = SensorHistory::default();
        history.select_maximum(&core);
        history.record(&[core.clone()]);

        history.reset();

        assert_eq!(history.selected_id(), Some(&sensor_id(&core)));
        assert!(!history.is_tracked(&sensor_id(&core)));
        assert!(history.row_stats(&sensor_id(&core)).is_none());
        assert!(history.row_samples(&sensor_id(&core)).is_empty());
    }
}
