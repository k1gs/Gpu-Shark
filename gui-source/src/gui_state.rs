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
    selected: Option<String>,
    stats: Option<SensorStats>,
    samples: VecDeque<f32>,
    show_maximum: bool,
}

impl SensorHistory {
    pub fn select(&mut self, name: &str, current: f32) {
        self.show_maximum = false;
        if self.selected.as_deref() != Some(name) {
            self.selected = Some(name.to_owned());
            self.stats = Some(SensorStats {
                current,
                min: current,
                max: current,
            });
            self.samples.clear();
            self.samples.push_back(current);
        }
    }

    pub fn select_maximum(&mut self, name: &str, current: f32) {
        self.select(name, current);
        self.show_maximum = true;
    }

    pub fn shows_maximum(&self) -> bool {
        self.show_maximum
    }

    pub fn selected_name(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    pub fn record(&mut self, sensors: &[SensorReading]) {
        let Some(name) = self.selected.as_deref() else {
            return;
        };
        let Some(sensor) = sensors.iter().find(|sensor| sensor.name == name) else {
            return;
        };
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
