use crate::sensor_model::{SensorId, metadata, sensor_id};
use gpu_shark::SensorReading;
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug)]
pub struct SensorStats {
    pub current: f32,
    pub min: f32,
    pub max: f32,
}

#[derive(Default)]
pub struct SensorHistory {
    selected: Option<SensorId>,
    stats: Option<SensorStats>,
    samples: VecDeque<f32>,
    show_maximum: bool,
}

impl SensorHistory {
    pub fn select(&mut self, sensor: &SensorReading) {
        let id = sensor_id(sensor);
        self.show_maximum = false;
        if !metadata(sensor).graphable {
            self.selected = Some(id);
            self.stats = None;
            self.samples.clear();
            return;
        }
        if self.selected.as_ref() != Some(&id) {
            self.selected = Some(id);
            self.stats = Some(SensorStats {
                current: sensor.value,
                min: sensor.value,
                max: sensor.value,
            });
            self.samples.clear();
            self.samples.push_back(sensor.value);
        }
    }

    pub fn select_maximum(&mut self, sensor: &SensorReading) {
        self.select(sensor);
        self.show_maximum = metadata(sensor).graphable;
    }

    pub fn shows_maximum(&self) -> bool {
        self.show_maximum
    }

    pub fn selected_id(&self) -> Option<&SensorId> {
        self.selected.as_ref()
    }

    pub fn record(&mut self, sensors: &[SensorReading]) {
        let Some(selected) = self.selected.as_ref() else {
            return;
        };
        let Some(sensor) = sensors.iter().find(|sensor| sensor_id(sensor) == *selected) else {
            return;
        };
        if !metadata(sensor).graphable {
            self.stats = None;
            self.samples.clear();
            return;
        }
        let stats = self.stats.get_or_insert(SensorStats {
            current: sensor.value,
            min: sensor.value,
            max: sensor.value,
        });
        stats.current = sensor.value;
        stats.min = stats.min.min(sensor.value);
        stats.max = stats.max.max(sensor.value);
        self.samples.push_back(sensor.value);
        while self.samples.len() > 120 {
            self.samples.pop_front();
        }
    }

    pub fn stats(&self) -> Option<SensorStats> {
        self.stats
    }

    pub fn samples(&self) -> Vec<f32> {
        self.samples.iter().copied().collect()
    }

    pub fn reset(&mut self) {
        if let Some(stats) = &mut self.stats {
            stats.min = stats.current;
            stats.max = stats.current;
            self.samples.clear();
            self.samples.push_back(stats.current);
        }
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

    #[test]
    fn history_survives_a_known_provider_alias_change() {
        let mut history = SensorHistory::default();
        history.select(&reading("GPU Core", 40.0));
        history.record(&[reading("GPU Core Temperature", 52.0)]);

        let stats = history.stats().expect("selected sensor stats");
        assert_eq!(stats.current, 52.0);
        assert_eq!(stats.min, 40.0);
        assert_eq!(stats.max, 52.0);
        assert_eq!(history.samples(), vec![40.0, 52.0]);
    }

    #[test]
    fn maximum_mode_and_reset_follow_double_click_semantics() {
        let mut history = SensorHistory::default();
        history.select_maximum(&reading("GPU Core", 40.0));
        history.record(&[reading("GPU Core", 55.0)]);
        history.record(&[reading("GPU Core", 48.0)]);

        assert!(history.shows_maximum());
        assert_eq!(history.stats().expect("tracked stats").max, 55.0);

        history.reset();
        let reset = history.stats().expect("reset stats");
        assert_eq!(reset.current, 48.0);
        assert_eq!(reset.min, 48.0);
        assert_eq!(reset.max, 48.0);
        assert_eq!(history.samples(), vec![48.0]);
    }
    #[test]
    fn categorical_perfcap_selection_has_no_numeric_history() {
        let perfcap = SensorReading {
            name: "PerfCap Reason".to_owned(),
            value: 0.0,
            unit: "Pwr, VRel".to_owned(),
        };
        let mut history = SensorHistory::default();

        history.select_maximum(&perfcap);
        history.record(std::slice::from_ref(&perfcap));

        assert_eq!(history.selected_id(), Some(&sensor_id(&perfcap)));
        assert!(!history.shows_maximum());
        assert!(history.stats().is_none());
        assert!(history.samples().is_empty());
    }
}
