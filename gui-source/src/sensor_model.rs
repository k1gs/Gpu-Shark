use gpu_shark::SensorReading;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SensorId(String);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SensorGroup {
    Gpu,
    Activity,
    System,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SensorKind {
    GpuCoreTemperature,
    HotspotTemperature,
    MemoryTemperature,
    Fan,
    GpuClock,
    MemoryClock,
    Power,
    Voltage,
    PerfCap,
    Cpu,
    Other,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SensorMetadata {
    pub id: SensorId,
    pub group: SensorGroup,
    pub kind: SensorKind,
    pub priority: u8,
    pub visible: bool,
    pub graphable: bool,
}

pub fn metadata(sensor: &SensorReading) -> SensorMetadata {
    let lower = sensor.name.trim().to_ascii_lowercase();
    let unit = sensor.unit.trim().to_ascii_lowercase();
    let temperature = unit.contains('°') && unit.contains('c');
    let clock = unit == "mhz" || unit == "ghz";
    let kind = if temperature && (lower.contains("hot spot") || lower.contains("hotspot")) {
        SensorKind::HotspotTemperature
    } else if temperature
        && (lower.contains("memory temperature") || lower.contains("memory junction"))
    {
        SensorKind::MemoryTemperature
    } else if temperature && lower.contains("gpu core") {
        SensorKind::GpuCoreTemperature
    } else if lower.contains("perfcap") || lower.contains("performance limit") {
        SensorKind::PerfCap
    } else if lower.contains("fan") && unit == "rpm" {
        SensorKind::Fan
    } else if clock && lower.contains("memory clock") {
        SensorKind::MemoryClock
    } else if clock && (lower.contains("gpu clock") || lower.contains("gpu core clock")) {
        SensorKind::GpuClock
    } else if lower.contains("power") && unit == "w" {
        SensorKind::Power
    } else if lower.contains("voltage") && unit == "v" {
        SensorKind::Voltage
    } else if lower.contains("cpu") || lower.contains("system") {
        SensorKind::Cpu
    } else {
        SensorKind::Other
    };

    let group = match kind {
        SensorKind::GpuCoreTemperature
        | SensorKind::HotspotTemperature
        | SensorKind::MemoryTemperature
        | SensorKind::Fan
        | SensorKind::GpuClock
        | SensorKind::MemoryClock
        | SensorKind::Power
        | SensorKind::Voltage
        | SensorKind::PerfCap => SensorGroup::Gpu,
        SensorKind::Cpu => SensorGroup::System,
        SensorKind::Other => SensorGroup::Activity,
    };
    let priority = match kind {
        SensorKind::GpuCoreTemperature => 0,
        SensorKind::HotspotTemperature => 1,
        SensorKind::MemoryTemperature => 2,
        SensorKind::Fan => 3,
        SensorKind::GpuClock => 4,
        SensorKind::MemoryClock => 5,
        SensorKind::Power => 6,
        SensorKind::Voltage => 7,
        SensorKind::PerfCap => 8,
        SensorKind::Cpu | SensorKind::Other => 9,
    };
    let id = match kind {
        SensorKind::GpuCoreTemperature => stable("gpu.temperature.core"),
        SensorKind::HotspotTemperature => stable("gpu.temperature.hotspot"),
        SensorKind::MemoryTemperature => stable("gpu.temperature.memory"),
        SensorKind::GpuClock => stable("gpu.clock.core"),
        SensorKind::MemoryClock => stable("gpu.clock.memory"),
        SensorKind::PerfCap => stable("gpu.performance.limit"),
        SensorKind::Cpu if lower.contains("package") => stable("cpu.temperature.package"),
        _ => SensorId(format!(
            "sensor.{}.{}",
            normalized(&sensor.unit),
            normalized(&sensor.name)
        )),
    };

    SensorMetadata {
        id,
        group,
        kind,
        priority,
        visible: kind != SensorKind::MemoryClock,
        graphable: kind != SensorKind::PerfCap,
    }
}

pub fn sensor_id(sensor: &SensorReading) -> SensorId {
    metadata(sensor).id
}

fn stable(value: &str) -> SensorId {
    SensorId(value.to_owned())
}

fn normalized(value: &str) -> String {
    let mut result = String::new();
    let mut separator = false;
    for character in value.chars().flat_map(char::to_lowercase) {
        if character.is_alphanumeric() {
            if separator && !result.is_empty() {
                result.push('.');
            }
            result.push(character);
            separator = false;
        } else {
            separator = true;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reading(name: &str, unit: &str) -> SensorReading {
        SensorReading {
            name: name.to_owned(),
            value: 42.0,
            unit: unit.to_owned(),
        }
    }

    #[test]
    fn known_aliases_share_stable_ids() {
        assert_eq!(
            sensor_id(&reading("GPU Core", "°C")),
            sensor_id(&reading("GPU Core Temperature", "°C"))
        );
        assert_eq!(
            sensor_id(&reading("GPU Hot Spot", "°C")),
            sensor_id(&reading("Hotspot", "°C"))
        );
        assert_eq!(
            sensor_id(&reading("PerfCap", "%")),
            sensor_id(&reading("Performance Limit", "%"))
        );
        assert!(!metadata(&reading("PerfCap Reason", "Pwr")).graphable);
        assert!(metadata(&reading("GPU Core", "°C")).graphable);
    }

    #[test]
    fn separate_fans_keep_separate_ids() {
        assert_ne!(
            sensor_id(&reading("GPU Fan 1", "RPM")),
            sensor_id(&reading("GPU Fan 2", "RPM"))
        );
    }

    #[test]
    fn memory_clock_remains_hidden() {
        let item = metadata(&reading("Memory Clock", "MHz"));
        assert_eq!(item.kind, SensorKind::MemoryClock);
        assert!(!item.visible);
    }

    #[test]
    fn names_do_not_override_explicit_units() {
        assert_eq!(metadata(&reading("GPU Core", "%")).kind, SensorKind::Other);
        assert_eq!(
            metadata(&reading("GPU Core Clock", "MHz")).kind,
            SensorKind::GpuClock
        );
    }
}
