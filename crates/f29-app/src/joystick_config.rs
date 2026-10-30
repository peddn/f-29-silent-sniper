//! Joystick-Belegung als Text (`config/joystick.cfg`). Ohne Bevy-Abhängigkeit,
//! damit sich das Einlesen für sich prüfen lässt.

use std::fmt;

/// Eine Achse in der Schreibweise der Belegungsdatei.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxisId {
    X,
    Y,
    Z,
    Rx,
    Ry,
    Rz,
    /// Linux-Achsennummer (`ABS_*`), z. B. 6 = `ABS_THROTTLE`.
    Abs(u16),
    /// Rohkennung, wie sie das Eingabesystem meldet.
    Raw(u32),
}

/// Eine Taste in der Schreibweise der Belegungsdatei.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ButtonId {
    /// Joystick-Taste 1 … 16 (Linux `BTN_TRIGGER` …).
    Number(u8),
    HatUp,
    HatRight,
    HatDown,
    HatLeft,
    /// Gamepad-Name (South, East, …).
    Named(&'static str),
    /// Rohkennung.
    Raw(u32),
}

/// Gamepad-Tastennamen, die die Datei kennt.
pub const BUTTON_NAMES: [&str; 15] = [
    "South",
    "East",
    "North",
    "West",
    "C",
    "Z",
    "LeftTrigger",
    "LeftTrigger2",
    "RightTrigger",
    "RightTrigger2",
    "Select",
    "Start",
    "Mode",
    "LeftThumb",
    "RightThumb",
];

/// Was eine Taste auslöst.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Gun,
    Fire,
    NextWeapon,
    BayDoors,
    Chaff,
    Flare,
    Radar,
    NextSteerpoint,
    LookForward,
    LookRight,
    LookAft,
    LookLeft,
    Gear,
    Flaps,
    Airbrake,
    MapZoom,
    ZoomIn,
    ZoomOut,
    MapMode,
    MfdPage,
    HudMode,
    View,
    Pause,
}

impl Action {
    pub const ALL: [(Action, &'static str); 23] = [
        (Action::Gun, "gun"),
        (Action::Fire, "fire"),
        (Action::NextWeapon, "next_weapon"),
        (Action::BayDoors, "bay_doors"),
        (Action::Chaff, "chaff"),
        (Action::Flare, "flare"),
        (Action::Radar, "radar"),
        (Action::NextSteerpoint, "next_steerpoint"),
        (Action::LookForward, "look_forward"),
        (Action::LookRight, "look_right"),
        (Action::LookAft, "look_aft"),
        (Action::LookLeft, "look_left"),
        (Action::Gear, "gear"),
        (Action::Flaps, "flaps"),
        (Action::Airbrake, "airbrake"),
        (Action::MapZoom, "map_zoom"),
        (Action::ZoomIn, "zoom_in"),
        (Action::ZoomOut, "zoom_out"),
        (Action::MapMode, "map_mode"),
        (Action::MfdPage, "mfd_page"),
        (Action::HudMode, "hud_mode"),
        (Action::View, "view"),
        (Action::Pause, "pause"),
    ];

    pub fn name(self) -> &'static str {
        Self::ALL.iter().find(|(a, _)| *a == self).map_or("?", |(_, n)| n)
    }
}

/// Achse mit Richtung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AxisBinding {
    pub axis: AxisId,
    pub invert: bool,
}

/// Die ganze Belegung.
#[derive(Debug, Clone, PartialEq)]
pub struct JoystickConfig {
    pub roll: Option<AxisBinding>,
    pub pitch: Option<AxisBinding>,
    pub yaw: Option<AxisBinding>,
    pub throttle: Option<AxisBinding>,
    pub deadzone: f32,
    pub buttons: Vec<(ButtonId, Action)>,
}

/// Mitgelieferte Belegung (Logitech Extreme 3D Pro, Linux).
pub const DEFAULT_TEXT: &str = include_str!("../../../config/joystick.cfg");

impl Default for JoystickConfig {
    fn default() -> Self {
        parse(DEFAULT_TEXT).expect("mitgelieferte Joystick-Belegung ist gültig")
    }
}

impl fmt::Display for AxisId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AxisId::X => write!(f, "X"),
            AxisId::Y => write!(f, "Y"),
            AxisId::Z => write!(f, "Z"),
            AxisId::Rx => write!(f, "RX"),
            AxisId::Ry => write!(f, "RY"),
            AxisId::Rz => write!(f, "RZ"),
            AxisId::Abs(n) => write!(f, "ABS {n}"),
            AxisId::Raw(n) => write!(f, "RAW {n}"),
        }
    }
}

impl fmt::Display for ButtonId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ButtonId::Number(n) => write!(f, "B{n}"),
            ButtonId::HatUp => write!(f, "HatUp"),
            ButtonId::HatRight => write!(f, "HatRight"),
            ButtonId::HatDown => write!(f, "HatDown"),
            ButtonId::HatLeft => write!(f, "HatLeft"),
            ButtonId::Named(n) => write!(f, "{n}"),
            ButtonId::Raw(n) => write!(f, "RAW {n}"),
        }
    }
}

fn parse_raw(rest: &str) -> Option<u32> {
    rest.trim().parse().ok()
}

fn parse_axis(text: &str) -> Result<Option<AxisBinding>, String> {
    let mut words: Vec<&str> = text.split_whitespace().collect();
    let invert = words.last().is_some_and(|w| w.eq_ignore_ascii_case("invert"));
    if invert {
        words.pop();
    }
    let axis = match words.as_slice() {
        ["-"] => return Ok(None),
        [w] => match w.to_ascii_uppercase().as_str() {
            "X" => AxisId::X,
            "Y" => AxisId::Y,
            "Z" => AxisId::Z,
            "RX" => AxisId::Rx,
            "RY" => AxisId::Ry,
            "RZ" => AxisId::Rz,
            _ => return Err(format!("unbekannte Achse „{w}“")),
        },
        [kind, n] if kind.eq_ignore_ascii_case("ABS") => {
            AxisId::Abs(n.parse().map_err(|_| format!("Achsennummer „{n}“ ungültig"))?)
        }
        [kind, n] if kind.eq_ignore_ascii_case("RAW") => {
            AxisId::Raw(parse_raw(n).ok_or_else(|| format!("Rohkennung „{n}“ ungültig"))?)
        }
        _ => return Err(format!("Achse „{text}“ nicht verstanden")),
    };
    Ok(Some(AxisBinding { axis, invert }))
}

fn parse_button(text: &str) -> Result<ButtonId, String> {
    let t = text.trim();
    if let Some(n) = t.strip_prefix(['B', 'b']).and_then(|n| n.parse::<u8>().ok()) {
        return if (1..=16).contains(&n) {
            Ok(ButtonId::Number(n))
        } else {
            Err(format!("Taste „{t}“: 1 … 16"))
        };
    }
    let words: Vec<&str> = t.split_whitespace().collect();
    if let [kind, n] = words.as_slice()
        && kind.eq_ignore_ascii_case("RAW")
    {
        return parse_raw(n).map(ButtonId::Raw).ok_or_else(|| format!("Rohkennung „{n}“ ungültig"));
    }
    match t.to_ascii_lowercase().as_str() {
        "hatup" => return Ok(ButtonId::HatUp),
        "hatright" => return Ok(ButtonId::HatRight),
        "hatdown" => return Ok(ButtonId::HatDown),
        "hatleft" => return Ok(ButtonId::HatLeft),
        _ => {}
    }
    BUTTON_NAMES
        .iter()
        .find(|n| n.eq_ignore_ascii_case(t))
        .map(|n| ButtonId::Named(n))
        .ok_or_else(|| format!("unbekannte Taste „{t}“"))
}

/// Liest eine Belegung. Kommentare beginnen mit `#`; Fehler nennen die Zeile.
pub fn parse(text: &str) -> Result<JoystickConfig, String> {
    let mut cfg = JoystickConfig {
        roll: None,
        pitch: None,
        yaw: None,
        throttle: None,
        deadzone: 0.08,
        buttons: Vec::new(),
    };
    for (i, raw) in text.lines().enumerate() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let err = |e: String| format!("Zeile {}: {e}", i + 1);
        let (key, value) = line.split_once('=').ok_or_else(|| err("„name = wert“ erwartet".into()))?;
        let (key, value) = (key.trim().to_ascii_lowercase(), value.trim());
        match key.as_str() {
            "roll" => cfg.roll = parse_axis(value).map_err(err)?,
            "pitch" => cfg.pitch = parse_axis(value).map_err(err)?,
            "yaw" => cfg.yaw = parse_axis(value).map_err(err)?,
            "throttle" => cfg.throttle = parse_axis(value).map_err(err)?,
            "deadzone" => {
                let d: f32 = value.parse().map_err(|_| err(format!("Zahl erwartet statt „{value}“")))?;
                if !(0.0..=0.5).contains(&d) {
                    return Err(err("deadzone liegt zwischen 0 und 0,5".into()));
                }
                cfg.deadzone = d;
            }
            name => {
                let action = Action::ALL
                    .iter()
                    .find(|(_, n)| *n == name)
                    .map(|(a, _)| *a)
                    .ok_or_else(|| err(format!("unbekannte Aktion „{name}“")))?;
                if value != "-" {
                    cfg.buttons.push((parse_button(value).map_err(err)?, action));
                }
            }
        }
    }
    Ok(cfg)
}

/// Linux-Kennungen, wie sie das Eingabesystem (gilrs) für Joysticks ohne
/// eigene Zuordnung meldet: Ereignistyp im oberen, Code im unteren Wort.
pub mod linux {
    const EV_KEY: u32 = 1;
    const EV_ABS: u32 = 3;
    /// `BTN_TRIGGER`: erste Joystick-Taste; die Tasten folgen fortlaufend.
    const BTN_TRIGGER: u32 = 0x120;

    pub fn abs_code(n: u16) -> u32 {
        (EV_ABS << 16) | u32::from(n)
    }

    pub fn button_code(n: u8) -> u32 {
        (EV_KEY << 16) | (BTN_TRIGGER + u32::from(n) - 1)
    }

    /// Rohkennung → Achsennummer, falls es eine Linux-Achse ist.
    pub fn abs_number(raw: u32) -> Option<u16> {
        (raw >> 16 == EV_ABS).then_some((raw & 0xffff) as u16)
    }

    /// Rohkennung → Joystick-Tastennummer 1 … 16.
    pub fn button_number(raw: u32) -> Option<u8> {
        let code = raw & 0xffff;
        (raw >> 16 == EV_KEY && (BTN_TRIGGER..BTN_TRIGGER + 16).contains(&code))
            .then(|| (code - BTN_TRIGGER + 1) as u8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_matches_the_agreed_layout() {
        let c = JoystickConfig::default();
        assert_eq!(c.roll, Some(AxisBinding { axis: AxisId::X, invert: false }));
        assert_eq!(c.pitch, Some(AxisBinding { axis: AxisId::Y, invert: true }));
        assert_eq!(c.yaw, Some(AxisBinding { axis: AxisId::Rz, invert: false }));
        assert_eq!(c.throttle, Some(AxisBinding { axis: AxisId::Abs(6), invert: true }));
        let b = |a: Action| c.buttons.iter().find(|(_, x)| *x == a).map(|(b, _)| b.clone());
        assert_eq!(b(Action::Gun), Some(ButtonId::Number(1)));
        assert_eq!(b(Action::Fire), Some(ButtonId::Number(2)));
        assert_eq!(b(Action::NextWeapon), Some(ButtonId::Number(3)));
        assert_eq!(b(Action::BayDoors), Some(ButtonId::Number(4)));
        assert_eq!(b(Action::Radar), Some(ButtonId::Number(5)));
        assert_eq!(b(Action::NextSteerpoint), Some(ButtonId::Number(6)));
        assert_eq!(b(Action::LookForward), Some(ButtonId::HatUp));
        assert_eq!(b(Action::Gear), Some(ButtonId::Number(7)));
        assert_eq!(b(Action::HudMode), Some(ButtonId::Number(12)));
        assert_eq!(b(Action::View), None);
    }

    #[test]
    fn parses_variants_and_reports_lines() {
        let c = parse(
            "roll = abs 0 invert\nyaw = -\nfire = RAW 65826\nfire = South\ngun = hatleft # Kommentar\n",
        )
        .expect("gültig");
        assert_eq!(c.roll, Some(AxisBinding { axis: AxisId::Abs(0), invert: true }));
        assert_eq!(c.yaw, None);
        assert_eq!(c.buttons.len(), 3);
        assert_eq!(c.buttons[1].0, ButtonId::Named("South"));
        let e = parse("\n\nfire = B17").unwrap_err();
        assert!(e.starts_with("Zeile 3"), "{e}");
        assert!(parse("fly = B1").is_err());
        assert!(parse("roll = W").is_err());
        assert!(parse("deadzone = 0.9").is_err());
    }

    #[test]
    fn display_round_trips() {
        for a in [AxisId::X, AxisId::Rz, AxisId::Abs(6), AxisId::Raw(42)] {
            assert_eq!(parse_axis(&a.to_string()).unwrap().unwrap().axis, a);
        }
        for b in [ButtonId::Number(12), ButtonId::HatDown, ButtonId::Named("Start"), ButtonId::Raw(7)] {
            assert_eq!(parse_button(&b.to_string()).unwrap(), b);
        }
    }

    #[test]
    fn linux_codes() {
        assert_eq!(linux::button_code(1), 0x10120);
        assert_eq!(linux::button_number(linux::button_code(12)), Some(12));
        assert_eq!(linux::button_number(0x10130), None, "BTN_SOUTH ist keine Joystick-Taste");
        assert_eq!(linux::abs_number(linux::abs_code(6)), Some(6));
        assert_eq!(linux::abs_number(linux::button_code(1)), None);
    }
}
