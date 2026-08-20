/// UI strings are kept out of rendering code so additional translations don't
/// require touching layout or telemetry semantics.
#[allow(dead_code)]
#[derive(Clone, Copy)]
pub enum Language {
    English,
    Russian,
}

impl Language {
    pub const fn from_russian(russian: bool) -> Self {
        if russian {
            Self::Russian
        } else {
            Self::English
        }
    }
    pub const fn text(self, key: Key) -> &'static str {
        match (self, key) {
            (Self::Russian, Key::Sensors) => "ДАТЧИКИ",
            (Self::Russian, Key::Selected) => "ВЫБРАННЫЙ ДАТЧИК",
            (Self::Russian, Key::GpuActivity) => "АКТИВНОСТЬ GPU",
            (Self::Russian, Key::System) => "СИСТЕМА",
            (Self::Russian, Key::ChooseSensor) => "Выберите датчик",
            (Self::Russian, Key::ChooseSensorHint) => {
                "Нажмите на значение, чтобы начать запись истории."
            }
            (Self::Russian, Key::History) => "ИСТОРИЯ",
            (Self::Russian, Key::Reset) => "СБРОС",
            (Self::Russian, Key::Max) => "МАКС",
            (Self::Russian, Key::Min) => "МИН",
            (Self::Russian, Key::Feedback) => "ОБРАТНАЯ СВЯЗЬ",
            (Self::Russian, Key::RefreshHint) => {
                "Обновление раз в секунду · двойной клик показывает максимум"
            }
            (Self::Russian, Key::UnknownGpu) => {
                "Неизвестная видеокарта — отправьте отчёт через обратную связь."
            }
            (Self::Russian, Key::Unavailable) => "ТЕЛЕМЕТРИЯ НЕДОСТУПНА",
            (Self::English, Key::Sensors) => "SENSORS",
            (Self::English, Key::Selected) => "SELECTED SENSOR",
            (Self::English, Key::GpuActivity) => "GPU ACTIVITY",
            (Self::English, Key::System) => "SYSTEM",
            (Self::English, Key::ChooseSensor) => "Choose a sensor",
            (Self::English, Key::ChooseSensorHint) => "Click a value to start its local history.",
            (Self::English, Key::History) => "HISTORY",
            (Self::English, Key::Reset) => "RESET",
            (Self::English, Key::Max) => "MAX",
            (Self::English, Key::Min) => "MIN",
            (Self::English, Key::Feedback) => "FEEDBACK",
            (Self::English, Key::RefreshHint) => {
                "Refreshes every second · double-click shows the maximum"
            }
            (Self::English, Key::UnknownGpu) => "Unknown GPU — send a report through feedback.",
            (Self::English, Key::Unavailable) => "TELEMETRY UNAVAILABLE",
        }
    }
}

#[derive(Clone, Copy)]
pub enum Key {
    Sensors,
    Selected,
    GpuActivity,
    System,
    ChooseSensor,
    ChooseSensorHint,
    History,
    Reset,
    Max,
    Min,
    Feedback,
    RefreshHint,
    UnknownGpu,
    Unavailable,
}
