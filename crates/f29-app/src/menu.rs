//! Menüführung vor dem Flug (M-036), Ablauf wie im Original: Erkennung
//! (Platzhalter) → Dienstplan (Platzhalter) → Auftrag (annehmen oder der
//! Reihe nach ändern: Gebiet, Konfliktlage, Missionsart, Gegnerqualität,
//! Flugverhalten) → Briefing → Schlussauswahl (neue Mission, Briefing,
//! Bewaffnung, Start). Eine neue Mission und das Ende der Nachbesprechung
//! führen zurück zum Dienstplan.
//!
//! Die Simulation steht still, solange ein Menü offen ist; sie trägt aber
//! schon die Mission, die das Briefing beschreibt.

use crate::actions::Actions;
use crate::input::PilotInput;
use crate::joystick_config::Action;
use crate::sim::{SimState, SimSystems};
use crate::store_art::StoreArt;
use crate::view::ViewSettings;
use bevy::prelude::*;
use f29_core::rules::{Conflict, Landings, Opponents};
use f29_core::weapons::{Loadout, WeaponKind};
use f29_hud::arming::Arming;
use f29_hud::{Page, Row};

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            menu_input
                .in_set(SimSystems::Input)
                .after(crate::actions::keyboard_actions)
                .after(crate::joystick::joystick_input)
                .before(crate::actions::apply_actions),
        );
    }
}

/// Die Bildschirme vor dem Flug.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Identification,
    Roster,
    Assignment,
    Region,
    Conflict,
    MissionType,
    Opponents,
    Landings,
    Briefing,
    Arming,
    Final,
}

/// Inhalt des offenen Bildschirms.
pub fn page(state: &SimState) -> Page {
    let sim = &state.sim;
    let sp = &sim.config().spawn;
    let region = sim.config().theater.name.to_uppercase();
    let list = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let (title, lines, options) = match state.menu.unwrap_or(Screen::Final) {
        Screen::Identification => (
            "AIRCRAFT IDENTIFICATION",
            list(&["PLACEHOLDER - THE IDENTIFICATION", "QUIZ FOLLOWS IN A LATER VERSION."]),
            list(&["CONTINUE"]),
        ),
        Screen::Roster => (
            "DUTY ROSTER",
            list(&["PLACEHOLDER - PILOT SELECTION", "FOLLOWS IN A LATER VERSION."]),
            list(&["CONTINUE"]),
        ),
        Screen::Assignment => (
            "MISSION ASSIGNMENT",
            vec![
                format!("REGION: {region}"),
                format!("CONFLICT: {}", sp.conflict.rules().label),
                "MISSION: STRIKE MISSIONS".into(),
                format!("OPPONENTS: {}", sp.opponents.label()),
                format!("FLIGHT: {}", sp.landings.label()),
            ],
            list(&["ACCEPT MISSION", "CHANGE CHOICES"]),
        ),
        Screen::Region => ("REGION OF THE WORLD", vec![], vec![region]),
        Screen::Conflict => {
            ("LEVEL OF CONFLICT", vec![], Conflict::ALL.map(|c| c.rules().label.to_string()).into())
        }
        Screen::MissionType => ("TYPE OF MISSION", vec![], list(&["STRIKE MISSIONS"])),
        Screen::Opponents => {
            ("OPPONENT QUALITY", vec![], Opponents::ALL.map(|o| format!("{} OPPONENTS", o.label())).into())
        }
        Screen::Landings => {
            ("FLIGHT PERFORMANCE", vec![], Landings::ALL.map(|l| l.label().to_string()).into())
        }
        Screen::Briefing => ("MISSION BRIEFING", sim.briefing(), list(&["CONTINUE"])),
        // eigener Bildschirm (`arming`)
        Screen::Arming => ("ARMING", vec![], vec![]),
        // Das Briefing noch einmal, um die Mission einzuschätzen.
        Screen::Final => (
            "FINAL CHOICES",
            sim.briefing(),
            list(&["SELECT NEW MISSION", "INTELLIGENCE BRIEFING", "ARM YOUR PLANE", "BEGIN MISSION"]),
        ),
    };
    let rows = lines.into_iter().map(Row::from).collect();
    Page { title: title.into(), rows, options, cursor: state.cursor }
}

/// Liste der Bewaffnung: die Waffen, „leer“, „fertig“.
const ARM_ITEMS: usize = WeaponKind::ALL.len() + 2;

/// Inhalt des Bewaffnungsbildschirms; `blink` ist der Takt des Blinkens.
pub fn arming(state: &SimState, art: &StoreArt, blink: bool) -> Arming {
    let sim = &state.sim;
    let bays = sim.stores().bays.map(|b| b.kind.map(|k| (k.label().to_string(), b.count)));
    let items = WeaponKind::ALL
        .iter()
        .map(|k| k.spec().name.to_string())
        .chain(["EMPTY".into(), "ARMING COMPLETE".into()]);
    Arming {
        bays,
        items: items.collect(),
        cursor: state.cursor,
        bay: state.arm_bay,
        blink,
        fuel: (sim.fuel_required_kg(), sim.fuel_carried_kg()),
        art: sim.stores().bays.map(|b| b.kind.is_some_and(|k| art.has(k))),
    }
}

/// Bewaffnung bedienen: in der Liste eine Waffe nehmen, dann mit den Pfeilen
/// einen Schacht wählen und sie dort ablegen; zurück in der Liste blinkt sie.
fn arm(state: &mut SimState, (up, down, left, right, ok): (bool, bool, bool, bool, bool)) {
    match state.arm_bay {
        None => {
            let n = ARM_ITEMS;
            let step = |c: usize, d: usize| (c + d) % n;
            let mut c = state.cursor;
            c = if up {
                step(c, n - 1)
            } else if down {
                step(c, 1)
            } else {
                c
            };
            if left || right {
                c = step(
                    c,
                    if c < f29_hud::arming::ROWS { f29_hud::arming::ROWS } else { n - f29_hud::arming::ROWS },
                );
            }
            state.cursor = c.min(n - 1);
            if ok && state.cursor == n - 1 {
                open(state, Screen::Final);
            } else if ok {
                state.arm_bay = Some(0);
            }
        }
        Some(mut k) => {
            // 2×2, Nase links: hoch/runter wechselt die Seite, links/rechts
            // vorn/hinten.
            k ^= usize::from(up || down) | usize::from(left || right) << 1;
            state.arm_bay = Some(k);
            if ok {
                let mut config = state.sim.config().clone();
                let mut l = Loadout(state.sim.stores().bays.map(|b| b.kind));
                l.0[k] = WeaponKind::ALL.get(state.cursor).copied();
                config.spawn.loadout = Some(l);
                state.reset(config);
                state.arm_bay = None;
            }
        }
    }
}

/// Öffnet einen Bildschirm; Auswahllisten beginnen bei der aktuellen Wahl.
fn open(state: &mut SimState, screen: Screen) {
    let sp = &state.sim.config().spawn;
    state.cursor = match screen {
        Screen::Conflict => sp.conflict as usize,
        Screen::Opponents => sp.opponents as usize,
        Screen::Landings => sp.landings as usize,
        _ => 0,
    };
    state.menu = Some(screen);
}

/// Neue Mission: nächster Seed (SplitMix64), dieselben Einstellungen, die
/// vorgeschlagene Beladung.
fn new_mission(state: &mut SimState) {
    let mut config = state.sim.config().clone();
    config.spawn.loadout = None;
    let z = config.seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    config.seed = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB) >> 1;
    state.reset(config);
}

/// Wahl auf dem offenen Bildschirm.
fn choose(state: &mut SimState, choice: usize) {
    let mut config = state.sim.config().clone();
    let next = match state.menu {
        Some(Screen::Identification) => Some(Screen::Roster),
        Some(Screen::Roster) => Some(Screen::Assignment),
        Some(Screen::Assignment) => Some(if choice == 0 { Screen::Briefing } else { Screen::Region }),
        Some(Screen::Region) => Some(Screen::Conflict),
        Some(Screen::Conflict) => {
            config.spawn.conflict = Conflict::ALL[choice];
            state.reset(config);
            Some(Screen::MissionType)
        }
        Some(Screen::MissionType) => Some(Screen::Opponents),
        Some(Screen::Opponents) => {
            config.spawn.opponents = Opponents::ALL[choice];
            state.reset(config);
            Some(Screen::Landings)
        }
        Some(Screen::Landings) => {
            config.spawn.landings = Landings::ALL[choice];
            state.reset(config);
            Some(Screen::Briefing)
        }
        Some(Screen::Briefing | Screen::Arming) => Some(Screen::Final),
        Some(Screen::Final) => match choice {
            0 => {
                new_mission(state);
                Some(Screen::Roster)
            }
            1 => Some(Screen::Briefing),
            2 => Some(Screen::Arming),
            _ => None,
        },
        None => None,
    };
    match next {
        Some(s) => open(state, s),
        None => state.menu = None,
    }
}

/// Bedienung: Pfeile bzw. W/S oder Blick vorn/hinten am Joystick wählen,
/// Leertaste, Enter bzw. Auslöser bestätigen. Alle anderen Aktionen ruhen
/// im Menü (nur Escape beendet). Am Ende der Nachbesprechung öffnet sich der
/// Dienstplan mit einer neuen Mission.
fn menu_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut actions: ResMut<Actions>,
    mut state: ResMut<SimState>,
    mut pilot: ResMut<PilotInput>,
    mut view: ResMut<ViewSettings>,
) {
    if state.menu.is_none() {
        // Enter blättert die Nachbesprechung wie Leertaste und Auslöser.
        if state.sim.mission_over() && keys.just_pressed(KeyCode::Enter) {
            view.debrief += 1;
        }
        let done =
            state.sim.mission_over() && view.debrief >= f29_hud::debrief::screens(&state.sim.score()).len();
        if done {
            view.debrief = 0;
            new_mission(&mut state);
            open(&mut state, Screen::Roster);
            *pilot = PilotInput::matching(&state.sim);
        }
        return;
    }
    let act = |a: Action| actions.0.iter().any(|&(x, pressed, _)| x == a && pressed);
    let up = keys.any_just_pressed([KeyCode::ArrowUp, KeyCode::KeyW]) || act(Action::LookForward);
    let down = keys.any_just_pressed([KeyCode::ArrowDown, KeyCode::KeyS]) || act(Action::LookAft);
    let ok = keys.just_pressed(KeyCode::Enter) || act(Action::Fire);
    let left = keys.any_just_pressed([KeyCode::ArrowLeft, KeyCode::KeyA]) || act(Action::LookLeft);
    let right = keys.any_just_pressed([KeyCode::ArrowRight, KeyCode::KeyD]) || act(Action::LookRight);
    actions.0.retain(|&(a, ..)| a == Action::Quit);
    if state.menu == Some(Screen::Arming) {
        arm(&mut state, (up, down, left, right, ok));
        *pilot = PilotInput::matching(&state.sim);
        return;
    }
    let n = page(&state).options.len();
    if up {
        state.cursor = (state.cursor + n - 1) % n;
    }
    if down {
        state.cursor = (state.cursor + 1) % n;
    }
    if ok {
        let choice = state.cursor;
        choose(&mut state, choice);
        *pilot = PilotInput::matching(&state.sim);
    }
}
