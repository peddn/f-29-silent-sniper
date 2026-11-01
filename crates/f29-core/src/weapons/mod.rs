//! Waffen (M-014, Wirkung M-010 bis M-012).
//!
//! Vier interne Schächte mit je einer Waffenart. Der Schacht wird von Hand
//! geöffnet; ausgelöst wird nur bei offenem Schacht und mäßiger Querlage.
//!
//! Alle Waffen fliegen **physikalisch** – wie im Original entscheidet der
//! Flug, nicht ein Würfel beim Abschuss (Umbau v0.2.9):
//! - **Bomben**: Massenpunkt mit Schwerkraft und Luftwiderstand; Schaden nach
//!   dem Abstand des Einschlags vom Ziel.
//! - **Raketen**: übernehmen das gewählte Ziel, wenn es im
//!   **Erfassungskreis** um die Nase liegt und ihr Suchkopf es sieht, sonst
//!   fliegen sie geradeaus und schalten auf, was ihnen vor den Suchkopf kommt.
//!   Sie verfolgen das Ziel (reine Verfolgung, begrenzte Drehrate), solange es
//!   im Suchkegel bleibt; liegt es weit hinter ihnen, geben sie auf. Ein
//!   Annäherungszünder löst aus, wenn sie dem Ziel näher kommen als ihr
//!   Wirkradius. **Die Wirkung wird gewürfelt** wie im Original: Die
//!   Wirkungsmatrix ist die Chance, das Ziel zu zerstören, sonst
//!   „wirkungslos“.
//! - **Laserbombe** (GBU-12): fällt wie eine Bombe und lenkt auf das Ziel,
//!   solange das Flugzeug es gewählt und optisch entdeckt hält (Laser).
//! - Ob ein Schuss treffen würde, rechnet [`launch_zone`] mit genau diesem Flug
//!   voraus („SHOOT“).
//! - **Feindliche Boden-Luft-Raketen** (M-021) fliegen in derselben Liste und
//!   mit demselben Flug, Ziel ist das Flugzeug: Startet eine Radarstellung,
//!   lenkt die Rakete halbaktiv – weit weg nur, solange die Stellung das
//!   Flugzeug beim letzten Umlauf erfasst hat, nah mit eigenem Suchkopf. Ein
//!   Treffer beschädigt zufällige Bordsysteme (keine Trefferpunkte).
//! - **Täuschkörper** (M-021, wie im Original): Düppel und Fackeln bleiben am
//!   Abwurfort stehen und brennen einige Sekunden. Liegt einer im Suchkegel
//!   einer Rakete der passenden Lenkart (Düppel: Radar, Fackel: Infrarot /
//!   optisch), lenkt sie auf ihn statt auf ihr Ziel – kein Würfel.
//! - **Vorhalt** für alle Lenkwaffen: Sie fliegen auf den Treffpunkt eines
//!   bewegten Ziels (Abweichung vom Original, damit Hineindrehen nicht
//!   rettet); bei stehenden Zielen ist das die reine Verfolgung des Originals.
//!   Der Vorhaltfaktor je Waffe stellt die Gefährlichkeit ein.
//!
//! Werte: eigene Größenordnungen aus öffentlichen Daten realer Waffen und
//! Spielbalance, siehe M-014.

use crate::atmosphere::{self, G0};
use crate::detmath;
use crate::events::EventLog;
use crate::math::{DQuat, DVec3};
use crate::rng::Pcg32;
use crate::sensors::line_of_sight;
use crate::systems::System;
use crate::targets::{Target, TargetClass, TargetState};
use f29_terrain::Terrain;

mod countermeasures;
mod guidance;
mod spec;

pub use countermeasures::*;
pub(crate) use guidance::ground_crossing;
pub use guidance::predict_release;
use guidance::*;
pub use spec::*;

/// RNG-Strom der Waffenwirkung.
pub const STREAM_WEAPONS: u64 = 0x5745_4150;
/// Anzahl der Schächte.
pub const BAYS: usize = 4;
/// Höchstens so viele eigene Waffen gleichzeitig in der Luft.
pub const MAX_IN_FLIGHT: usize = 8;
/// Zeit zum Öffnen bzw. Schließen der Schachtklappen (s).
pub const DOOR_TIME_S: f64 = 1.2;
/// Auslösen nur bis zu dieser Querlage (rad, 60°).
pub const MAX_RELEASE_BANK_RAD: f64 = std::f64::consts::FRAC_PI_3;
/// Ausstoßgeschwindigkeit nach unten (Körperachse z, m/s).
const EJECT_SPEED_MPS: f64 = 3.0;
/// Startpunkt unter dem Rumpf (Körperachse z, m).
const EJECT_OFFSET_M: f64 = 1.5;
/// Ab diesem Eintreffwinkel (unter der Waagerechten) wirkt eine Bombe gegen
/// gehärtete Ziele (M-012, rad, 60°).
pub const HARDENED_MIN_IMPACT_RAD: f64 = std::f64::consts::FRAC_PI_3;
/// Bis zu diesem Anteil des Wirkradius ist die Wirkung voll (Treffer), darüber
/// fällt sie linear auf null (M-011).
const FULL_EFFECT_FRACTION: f64 = 0.4;
/// Schaden darunter zählt nicht.
const MIN_DAMAGE: f64 = 0.05;
/// Wahrscheinlichkeit eines kritischen Bombentreffers.
pub const CRITICAL_CHANCE: f64 = 0.01;
/// Längste Flugzeit einer ungelenkten Waffe (s), Schutz gegen Endlosflug.
const MAX_FALL_S: f64 = 180.0;
/// Sicherheitsabstand über dem Boden, ab dem die Vorhersage das Gelände nur
/// noch selten abfragt (m).
const PREDICT_MARGIN_M: f64 = 300.0;
/// Lenkwaffen fallen erst frei aus dem Schacht, dann setzt die Lenkung ein (s).
const GUIDANCE_DELAY_S: f64 = 0.4;
/// Suchkopf und Laser alle so viele Ticks prüfen (Sichtlinie ist teuer).
const SEEKER_TICKS: u64 = 12;
/// Liegt das Ziel mehr als so weit hinter der Flugrichtung, gibt die Rakete
/// auf (wie im Original, rad, 135°).
const GIVE_UP_RAD: f64 = 3.0 * std::f64::consts::FRAC_PI_4;
/// Vorausberechnung: so viele Simulationsschritte auf einmal.
const PREDICT_STEPS: f64 = 4.0;
/// Feindliche Raketen höchstens so viele gleichzeitig in der Luft (je
/// Stellung eine, wie im Original).
pub const MAX_HOSTILE: usize = 4;
/// Näher als so (m) lenkt eine Boden-Luft-Rakete mit eigenem Suchkopf, weiter
/// weg nur mit der Beleuchtung ihrer Stellung (halbaktiv).
pub const SEMI_ACTIVE_M: f64 = 5_000.0;
/// Boden-Luft-Raketen starten mindestens so steil über der Linie zum
/// Flugzeug (rad, 10° – innerhalb des Suchkegels).
const SAM_DIRECT_RAD: f64 = 10.0 * std::f64::consts::PI / 180.0;
/// Vorhalt höchstens für so viele Sekunden Restflugzeit.
const MAX_LEAD_S: f64 = 10.0;
/// Startgeschwindigkeit der Boden-Luft-Raketen (m/s).
const SAM_LAUNCH_MPS: f64 = 60.0;
/// Störer (ECM bzw. Infrarot): Raketen der gestörten Lenkart verlieren die
/// Lenkung, solange sie weiter als so weg sind (m) – wie im Original.
pub const BURN_THROUGH_M: f64 = 2_000.0;
/// Wirkdauer des Infrarotstörers (s).
pub const IR_JAMMER_S: f64 = 30.0;
/// Danach muss er so lange abkühlen, bevor er wieder läuft (s).
pub const IR_COOLDOWN_S: f64 = 60.0;
/// Täuschkörper: Vorrat je Art und Brenndauer (s). Jeder Tastendruck wirft
/// einen aus, ohne Grenze für die Zahl in der Luft (Spielerwunsch v0.2.9).
pub const DECOY_STOCK: u8 = 20;
pub const DECOY_LIFE_S: f64 = 5.0;
/// So viele Bordsysteme beschädigt ein Raketentreffer (Original: Konflikt-
/// stufe plus eins; Schwierigkeitsgrade folgen).
pub const HIT_SYSTEMS: usize = 2;

/// Zone einer Lenkwaffe gegen das gewählte Ziel (Zielrahmen und
/// Feuersignal im HUD).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchZone {
    /// Waffe wirkt nicht gegen dieses Ziel oder ihr Suchkopf sieht es nicht.
    NoEffect,
    /// Die Waffe würde das Ziel nicht übernehmen (Reichweite, Erfassungskreis,
    /// keine optische Entdeckung).
    NoLock,
    /// Sie würde es übernehmen, aber laut Vorausberechnung verfehlen.
    Locked,
    /// Die Vorausberechnung trifft: gute Abschussbedingungen („SHOOT“).
    Good,
}

/// Zone der Waffe `kind` gegen Ziel `i`, wenn sie jetzt unter `l` ausgelöst
/// würde: Übergabe wie beim echten Auslösen, dann derselbe Flug in groben
/// Schritten vorausgerechnet (Sichtlinie und Laser als gehalten angenommen).
pub fn launch_zone(kind: WeaponKind, l: &Launch<'_>, i: usize) -> LaunchZone {
    let t = &l.targets[i];
    if !kind.spec().seeker.sees(t.class) || kind.effectiveness(t.class, std::f64::consts::FRAC_PI_2) <= 0.0 {
        return LaunchZone::NoEffect;
    }
    let mut w = Weapon::launched(0, kind, l);
    if w.ground_target() != Some(i) {
        return LaunchZone::NoLock;
    }
    let view = View {
        targets: l.targets,
        terrain: l.terrain,
        designated: Some(i),
        aircraft: None,
        decoys: &[],
        painted: false,
        jammed: (false, false),
        los: false,
    };
    let dt = l.dt * PREDICT_STEPS;
    let mut step = 0_u32;
    loop {
        if step.is_multiple_of(3) {
            seek(&mut w, &view);
        }
        step += 1;
        let aim = w.target.filter(|_| w.tracking).and_then(|t| view.locate(t));
        match advance(&mut w, aim, l.terrain, dt) {
            Fate::Flying => {}
            Fate::Fuze(_) => return LaunchZone::Good,
            Fate::Ground(p) => {
                let (dn, de) = (t.north - p.x, t.east - p.y);
                let near = (dn * dn + de * de).sqrt() - t.class.radius() < kind.spec().lethal_radius_m;
                return if near { LaunchZone::Good } else { LaunchZone::Locked };
            }
            Fate::Expired => return LaunchZone::Locked,
        }
    }
}

/// Beladung der vier Schächte (eine Waffenart je Schacht oder leer).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Loadout(pub [Option<WeaponKind>; BAYS]);

impl Default for Loadout {
    /// Standardbeladung: Mk 82, Snakeye, GBU-12, Maverick.
    fn default() -> Self {
        Self([
            Some(WeaponKind::FreeFall),
            Some(WeaponKind::Retarded),
            Some(WeaponKind::GuidedBomb),
            Some(WeaponKind::AirToGround),
        ])
    }
}

impl Loadout {
    /// Leere Schächte (z. B. für Landeanflüge nach der Mission).
    pub const EMPTY: Loadout = Loadout([None; BAYS]);

    /// `fall,chute,lgb,agm` – `-` für einen leeren Schacht.
    pub fn parse(text: &str) -> Option<Self> {
        let parts: Vec<&str> = text.split(',').map(str::trim).collect();
        if parts.len() != BAYS {
            return None;
        }
        let mut bays = [None; BAYS];
        for (bay, p) in bays.iter_mut().zip(parts) {
            *bay = match p {
                "-" | "" => None,
                code => Some(WeaponKind::from_code(code)?),
            };
        }
        Some(Self(bays))
    }

    pub fn to_text(&self) -> String {
        self.0.iter().map(|b| b.map_or("-", WeaponKind::code)).collect::<Vec<_>>().join(",")
    }

    /// Masse der vollen Beladung (kg).
    pub fn mass_kg(&self) -> f64 {
        self.0.iter().flatten().map(|k| k.spec().mass_kg * f64::from(k.spec().per_bay)).sum()
    }
}

/// Ein Schacht.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Bay {
    pub kind: Option<WeaponKind>,
    pub count: u8,
}

/// Was eine Lenkwaffe verfolgt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Track {
    /// Bodenziel (Index in `Sim::targets`).
    Ground(usize),
    /// Das eigene Flugzeug (feindliche Raketen).
    Aircraft,
}

/// Eine Waffe im Flug.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Weapon {
    /// Fortlaufende Nummer (für die Darstellung).
    pub id: u32,
    pub kind: WeaponKind,
    /// NED (m).
    pub position: DVec3,
    /// NED (m/s).
    pub velocity: DVec3,
    pub age_s: f64,
    /// Verfolgtes Ziel; `None`, wenn es keines (mehr) hat.
    pub target: Option<Track>,
    /// Der Suchkopf (bzw. der Laser) hatte das Ziel bei der letzten Prüfung;
    /// sonst fliegt die Waffe ungelenkt weiter.
    pub tracking: bool,
    /// Ohne gewähltes Ziel abgefeuert: sucht selbst.
    pub searching: bool,
    /// Richtung, in der die Rakete ohne Ziel weiterfliegt: die Abschusslage
    /// (Flugzeugnase).
    pub boresight: DVec3,
    /// Feindliche Rakete: gestartet von der Radarstellung mit diesem Index in
    /// `Sim::targets`; `None` = eigene Waffe.
    pub owner: Option<usize>,
    /// Ein Täuschkörper im Suchkegel blendet sie gerade.
    pub blinded: bool,
}

impl Weapon {
    fn new(id: u32, kind: WeaponKind, position: DVec3, velocity: DVec3) -> Self {
        Self {
            id,
            kind,
            position,
            velocity,
            age_s: 0.0,
            target: None,
            tracking: false,
            searching: false,
            boresight: velocity.normalize_or_zero(),
            owner: None,
            blinded: false,
        }
    }

    /// Frisch ausgelöst unter `l`, mit Übergabe des gewählten Ziels:
    /// Laserbombe bei optischer Entdeckung; Rakete, wenn das Ziel im
    /// Erfassungskreis liegt und ihr Suchkopf es sieht (optisch nur, wenn es
    /// optisch entdeckt ist) – sonst sucht sie selbst.
    fn launched(id: u32, kind: WeaponKind, l: &Launch<'_>) -> Self {
        let down = l.attitude.rotate(DVec3::new(0.0, 0.0, 1.0));
        let mut w =
            Weapon::new(id, kind, l.position + down * EJECT_OFFSET_M, l.velocity + down * EJECT_SPEED_MPS);
        let lock = l.lock.and_then(|i| l.targets.get(i).map(|t| (i, t)));
        match kind.spec().seeker {
            Seeker::None => {}
            Seeker::Laser => {
                w.target =
                    lock.filter(|(_, t)| l.lock_optical && t.is_intact()).map(|(i, _)| Track::Ground(i));
            }
            seeker => {
                let nose = l.attitude.rotate(DVec3::new(1.0, 0.0, 0.0));
                w.boresight = nose;
                let optical_ok = seeker != Seeker::Optical || l.lock_optical;
                w.target = lock.filter(|_| optical_ok).and_then(|(i, t)| {
                    seeker_sees(kind, l.position, nose, crate::sensors::GROUND_CIRCLE_RAD, t, l.terrain, true)
                        .map(|_| Track::Ground(i))
                });
                w.searching = w.target.is_none();
            }
        }
        w.tracking = w.target.is_some();
        w
    }

    /// Lage für die Darstellung: Längsachse entlang der Flugrichtung.
    pub fn attitude(&self) -> DQuat {
        let v = self.velocity;
        let yaw = detmath::atan2(v.y, v.x);
        let horiz = (v.x * v.x + v.y * v.y).sqrt();
        let pitch = detmath::atan2(-v.z, horiz);
        DQuat::from_euler(yaw, pitch, 0.0)
    }

    /// Bremsschirm offen (Fallschirmbombe).
    pub fn chute_open(&self) -> bool {
        let s = self.kind.spec();
        s.deployed_drag_area_m2 != s.drag_area_m2 && self.age_s >= s.deploy_s
    }

    /// Verfolgtes Bodenziel (Index in `Sim::targets`).
    pub fn ground_target(&self) -> Option<usize> {
        match self.target? {
            Track::Ground(i) => Some(i),
            _ => None,
        }
    }

    /// Punkt, den eine Waffe gegen Bodenziele gerade anfliegt: die Zielmitte,
    /// solange der Suchkopf sie hat.
    pub fn aim_point(&self, targets: &[Target]) -> Option<DVec3> {
        let t = targets.get(self.ground_target()?).filter(|_| self.tracking)?;
        let c = t.center_ned();
        Some(DVec3::new(c[0], c[1], c[2]))
    }
}

/// Was beim Einschlag geschah (bezogen auf das wichtigste getroffene Ziel).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImpactResult {
    /// Ziel zerstört; `critical` = durch kritischen Treffer.
    Destroyed { target: usize, critical: bool },
    /// Ziel beschädigt, aber noch nicht zerstört.
    Damaged(usize),
    /// Ziel im Wirkbereich, aber die Waffe wirkt nicht gegen seine Klasse.
    Ineffective(usize),
    /// Kein Ziel getroffen.
    Ground,
}

impl ImpactResult {
    fn rank(self) -> u8 {
        match self {
            ImpactResult::Destroyed { .. } => 3,
            ImpactResult::Damaged(_) => 2,
            ImpactResult::Ineffective(_) => 1,
            ImpactResult::Ground => 0,
        }
    }
}

/// Ereignisse für Meldungen, Klang und Darstellung.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WeaponEvent {
    Released {
        kind: WeaponKind,
        bay: usize,
        id: u32,
    },
    /// Auslösen bei geschlossenem Schacht.
    BayClosed,
    /// Waffenschacht defekt (Schadensmodell): kein Auslösen.
    BayFault,
    /// Gewählter Schacht leer.
    BayEmpty,
    /// Querlage zu groß zum Auslösen.
    BankLimit,
    /// Zu viele eigene Waffen in der Luft.
    Busy,
    /// Suchkopf hat ein Ziel erfasst (beim Abschuss oder selbst gesucht).
    Acquired {
        kind: WeaponKind,
        id: u32,
        target: usize,
    },
    /// Laserbombe: Das Flugzeug beleuchtet das Ziel nicht mehr (keine
    /// optische Entdeckung mehr) – sie fällt ungelenkt weiter.
    LaserLost {
        kind: WeaponKind,
        id: u32,
    },
    Impact {
        kind: WeaponKind,
        id: u32,
        position: DVec3,
        result: ImpactResult,
    },
    /// Einschlag im Splitterbereich des eigenen Flugzeugs.
    FragWarning,
    /// Flugkörper hat seine Flugzeit überschritten und ist abgestürzt.
    Expired {
        kind: WeaponKind,
        id: u32,
    },
    /// Eine Radarstellung (Index in `Sim::targets`) hat eine Rakete gestartet.
    SamLaunch {
        kind: WeaponKind,
        id: u32,
        site: usize,
    },
    /// Ein Täuschkörper blendet eine Rakete (sie fliegt geradeaus weiter).
    Decoyed {
        kind: WeaponKind,
        id: u32,
    },
    /// Täuschkörper dieser Art sind aufgebraucht.
    DecoyEmpty(DecoyKind),
    /// Eine feindliche Rakete hat das Flugzeug getroffen und diese Systeme
    /// beschädigt.
    AircraftHit {
        kind: WeaponKind,
        id: u32,
        systems: [System; HIT_SYSTEMS],
    },
}

/// Bestand und Zustand der Waffen.
#[derive(Debug, Clone, PartialEq)]
pub struct Stores {
    pub bays: [Bay; BAYS],
    /// Gewählter Schacht (0-basiert).
    pub selected: usize,
    /// Die Bordkanone ist gewählt (vor Schacht 1 in der Reihe GUN, BAY 1–4;
    /// Startwahl, Spielerwunsch v0.2.8).
    pub gun_selected: bool,
    /// Klappen 0 (zu) .. 1 (offen).
    pub door: f64,
    /// Klappen sollen offen sein.
    pub door_cmd: bool,
    pub in_flight: Vec<Weapon>,
    pub countermeasures: Countermeasures,
    next_id: u32,
    rng: Pcg32,
    /// Letzte Ereignisse mit Tick, neueste zuletzt.
    pub events: EventLog<WeaponEvent>,
}

/// Umgebung eines Waffen-Ticks.
#[derive(Debug)]
pub struct World<'a> {
    pub terrain: &'a Terrain,
    pub targets: &'a mut [Target],
    pub tick: u64,
    pub dt: f64,
    /// Position und Geschwindigkeit des eigenen Flugzeugs (NED).
    pub aircraft: DVec3,
    pub aircraft_velocity: DVec3,
    /// Gewähltes, optisch entdecktes Ziel des Flugzeugs (Laserbeleuchtung).
    pub designated: Option<usize>,
    /// Das Flugzeug fliegt (Ziel der feindlichen Raketen).
    pub flying: bool,
    /// Radarstellungen (Index in `Sim::targets`), die das Flugzeug beim letzten
    /// Umlauf erfasst haben: Sie beleuchten es für ihre Raketen.
    pub painting: &'a [usize],
}

/// Was der Abschuss über Flugzeug und Ziel wissen muss.
#[derive(Debug, Clone, Copy)]
pub struct Launch<'a> {
    pub tick: u64,
    pub position: DVec3,
    pub velocity: DVec3,
    pub attitude: DQuat,
    /// Gewähltes Ziel des Flugzeugs.
    pub lock: Option<usize>,
    pub targets: &'a [Target],
    pub terrain: &'a Terrain,
    /// Das gewählte Ziel ist optisch entdeckt (optische Suchköpfe
    /// übernehmen nur optisch entdeckte Ziele).
    pub lock_optical: bool,
    /// Waffenschacht funktioniert (Schadensmodell).
    pub bay_ok: bool,
    /// Zeitschritt der Simulation (s), für die Vorausberechnung.
    pub dt: f64,
}

/// Winkel zwischen zwei Richtungen (rad).
fn angle_between(a: DVec3, b: DVec3) -> f64 {
    let (la, lb) = (a.length(), b.length());
    if la <= 0.0 || lb <= 0.0 {
        return std::f64::consts::PI;
    }
    detmath::acos((a.dot(b) / (la * lb)).clamp(-1.0, 1.0))
}

/// Sieht der Suchkopf in `pos` mit Blickrichtung `dir` das Ziel innerhalb des
/// Kegels `cone`? Prüft Klasse, Kegel, Reichweite und – optisch, wenn `los` –
/// die Sichtlinie. Liefert die Entfernung.
fn seeker_sees(
    kind: WeaponKind,
    pos: DVec3,
    dir: DVec3,
    cone: f64,
    t: &Target,
    terrain: &Terrain,
    los: bool,
) -> Option<f64> {
    let s = kind.spec();
    if !t.is_intact() || !s.seeker.sees(t.class) {
        return None;
    }
    let c = t.center_ned();
    let to = DVec3::new(c[0], c[1], c[2]) - pos;
    let range = to.length();
    if angle_between(dir, to) > cone || range > s.max_range_m {
        return None;
    }
    if los
        && s.seeker == Seeker::Optical
        && !line_of_sight(|n, e| terrain.elevation(n, e), [pos.x, pos.y, -pos.z], [c[0], c[1], -c[2]])
    {
        return None;
    }
    Some(range)
}

impl Stores {
    pub fn new(loadout: &Loadout, seed: u64) -> Self {
        let bays = loadout.0.map(|kind| Bay { kind, count: kind.map_or(0, |k| k.spec().per_bay) });
        let selected = bays.iter().position(|b| b.count > 0).unwrap_or(0);
        Self {
            bays,
            selected,
            gun_selected: true,
            door: 0.0,
            door_cmd: false,
            in_flight: Vec::new(),
            countermeasures: Countermeasures::default(),
            next_id: 0,
            rng: Pcg32::new(seed, STREAM_WEAPONS),
            events: EventLog::default(),
        }
    }

    /// Zustand des Zufallsstroms (für den Zustandshash).
    pub fn rng_state(&self) -> (u64, u64) {
        self.rng.raw_state()
    }

    /// Masse der Waffen an Bord (kg).
    pub fn mass_kg(&self) -> f64 {
        self.bays.iter().filter_map(|b| b.kind.map(|k| k.spec().mass_kg * f64::from(b.count))).sum()
    }

    pub fn door_open(&self) -> bool {
        self.door >= 1.0
    }

    /// Waffe des gewählten Schachts, falls noch eine da ist.
    pub fn selected_weapon(&self) -> Option<WeaponKind> {
        if self.gun_selected {
            return None;
        }
        let b = self.bays[self.selected];
        b.kind.filter(|_| b.count > 0)
    }

    /// Nächste Waffe in der Reihe GUN → BAY 1 → … → BAY 4 → GUN.
    pub fn next_bay(&mut self) {
        if self.gun_selected {
            self.gun_selected = false;
            self.selected = 0;
        } else if self.selected + 1 >= BAYS {
            self.gun_selected = true;
        } else {
            self.selected += 1;
        }
    }

    /// Klappen bewegen; `speed` 0..1 aus dem Systemzustand (0 = klemmen).
    pub fn update_door(&mut self, dt: f64, speed: f64) {
        let step = speed * dt / DOOR_TIME_S;
        self.door = if self.door_cmd { (self.door + step).min(1.0) } else { (self.door - step).max(0.0) };
    }

    /// Löst die Waffe des gewählten Schachts aus, wenn alle Bedingungen
    /// erfüllt sind (Übergabe siehe [`Weapon::launched`]). Liefert die
    /// abgeworfene Masse (kg).
    pub fn release(&mut self, l: &Launch<'_>) -> f64 {
        let bay = self.bays[self.selected];
        let (_, _, roll) = l.attitude.to_euler();
        let event = match bay.kind {
            _ if !l.bay_ok => Some(WeaponEvent::BayFault),
            _ if !self.door_open() => Some(WeaponEvent::BayClosed),
            Some(_) if bay.count == 0 => Some(WeaponEvent::BayEmpty),
            None => Some(WeaponEvent::BayEmpty),
            _ if roll.abs() > MAX_RELEASE_BANK_RAD => Some(WeaponEvent::BankLimit),
            _ if self.in_flight.iter().filter(|w| w.owner.is_none()).count() >= MAX_IN_FLIGHT => {
                Some(WeaponEvent::Busy)
            }
            _ => None,
        };
        if let Some(e) = event {
            self.events.push(l.tick, e);
            return 0.0;
        }
        let kind = bay.kind.expect("geprüft");
        self.bays[self.selected].count -= 1;
        let id = self.next_id;
        self.next_id += 1;
        let w = Weapon::launched(id, kind, l);
        self.events.push(l.tick, WeaponEvent::Released { kind, bay: self.selected, id });
        if let Some(target) = w.ground_target() {
            self.events.push(l.tick, WeaponEvent::Acquired { kind, id, target });
        }
        self.in_flight.push(w);
        kind.spec().mass_kg
    }

    /// Startet eine Boden-Luft-Rakete der Radarstellung `site` (Index in
    /// `Sim::targets`, Startpunkt `from`) auf das Flugzeug bei `aircraft` –
    /// wenn es in Reichweite ist, die Stellung keine Rakete mehr in der Luft
    /// hat und nicht schon [`MAX_HOSTILE`] fliegen (wie im Original).
    pub fn launch_sam(
        &mut self,
        tick: u64,
        kind: WeaponKind,
        site: usize,
        from: DVec3,
        aircraft: DVec3,
    ) -> bool {
        let to = aircraft - from;
        let hostile = self.in_flight.iter().filter(|w| w.owner.is_some());
        if to.length() > kind.spec().max_range_m
            || hostile.clone().any(|w| w.owner == Some(site))
            || hostile.count() >= MAX_HOSTILE
        {
            return false;
        }
        // Im Startwinkel des Typs, mindestens knapp über der Linie zum
        // Flugzeug abheben.
        let flat = DVec3::new(to.x, to.y, 0.0).normalize_or_zero();
        let direct = detmath::atan2(-to.z, to.x.hypot(to.y)) + SAM_DIRECT_RAD;
        let elev = direct.max(kind.spec().launch_rad).min(std::f64::consts::FRAC_PI_2);
        let dir = flat * elev.cos() + DVec3::new(0.0, 0.0, -elev.sin());
        let id = self.next_id;
        self.next_id += 1;
        let mut w = Weapon::new(id, kind, from, dir * SAM_LAUNCH_MPS);
        w.owner = Some(site);
        w.target = Some(Track::Aircraft);
        self.in_flight.push(w);
        self.events.push(tick, WeaponEvent::SamLaunch { kind, id, site });
        true
    }

    /// Wirft einen Täuschkörper des Flugzeugs bei `position` ab (leerer
    /// Vorrat meldet sich).
    pub fn drop_decoy(&mut self, tick: u64, kind: DecoyKind, position: DVec3) {
        let cm = &mut self.countermeasures;
        let stock = match kind {
            DecoyKind::Chaff => &mut cm.chaff,
            DecoyKind::Flare => &mut cm.flares,
        };
        if *stock == 0 {
            self.events.push(tick, WeaponEvent::DecoyEmpty(kind));
            return;
        }
        *stock -= 1;
        let id = self.next_id;
        self.next_id += 1;
        // Zufall und Abwurfzeitpunkt: Der k-te Täuschkörper einer Mission
        // würfelt nicht immer gleich.
        let salt = self.rng.next_u32() ^ tick as u32;
        cm.decoys.push(Decoy { id, kind, owner: None, position, age_s: 0.0, salt });
    }

    /// Alle Waffen im Flug einen Zeitschritt weiterbewegen; Zünder und
    /// Einschläge auswerten.
    pub fn step(&mut self, world: &mut World<'_>) {
        let cm = &mut self.countermeasures;
        for d in &mut cm.decoys {
            d.age_s += world.dt;
        }
        cm.decoys.retain(|d| d.age_s < DECOY_LIFE_S);
        let decoys = cm.decoys.clone();
        let jammed = (cm.ecm, cm.ir_active());
        let mut i = 0;
        while i < self.in_flight.len() {
            let mut w = self.in_flight[i];
            let view = View {
                targets: world.targets,
                terrain: world.terrain,
                designated: world.designated,
                aircraft: world.flying.then_some((world.aircraft, world.aircraft_velocity)),
                decoys: &decoys,
                painted: w.owner.is_some_and(|o| world.painting.contains(&o)),
                jammed,
                los: true,
            };
            if w.kind.is_guided() && (world.tick + u64::from(w.id)).is_multiple_of(SEEKER_TICKS) {
                let had = w.tracking;
                if let Some(e) = seek(&mut w, &view) {
                    self.events.push(world.tick, e);
                }
                if had && !w.tracking && w.kind.spec().seeker == Seeker::Laser {
                    self.events.push(world.tick, WeaponEvent::LaserLost { kind: w.kind, id: w.id });
                }
            }
            let aim = w.target.filter(|_| w.tracking).and_then(|t| view.locate(t));
            match advance(&mut w, aim, world.terrain, world.dt) {
                Fate::Flying => {
                    self.in_flight[i] = w;
                    i += 1;
                }
                Fate::Fuze(_) if w.target == Some(Track::Aircraft) => {
                    self.in_flight.remove(i);
                    let systems = self.pick_systems();
                    self.events
                        .push(world.tick, WeaponEvent::AircraftHit { kind: w.kind, id: w.id, systems });
                }
                // Feindlich im Gelände: wirkungslos.
                Fate::Fuze(p) | Fate::Ground(p) if w.owner.is_some() => {
                    self.in_flight.remove(i);
                    let result = ImpactResult::Ground;
                    self.events.push(
                        world.tick,
                        WeaponEvent::Impact { kind: w.kind, id: w.id, position: p, result },
                    );
                }
                Fate::Fuze(p) | Fate::Ground(p) => {
                    self.in_flight.remove(i);
                    self.impact(world, &w, p);
                }
                Fate::Expired => {
                    self.in_flight.remove(i);
                    self.events.push(world.tick, WeaponEvent::Expired { kind: w.kind, id: w.id });
                }
            }
        }
    }

    /// Zufällige, verschiedene Bordsysteme für einen Raketentreffer.
    fn pick_systems(&mut self) -> [System; HIT_SYSTEMS] {
        let mut pool = System::ALL.to_vec();
        std::array::from_fn(|_| pool.remove(self.rng.next_u32() as usize % pool.len()))
    }

    /// Schaden ohne kritischen Treffer (Bordkanone).
    pub(crate) fn apply_damage(target: &mut Target, i: usize, amount: f64) -> ImpactResult {
        Self::apply(target, i, amount, false)
    }

    /// Schaden an einem Ziel anrichten; liefert das Ergebnis für dieses Ziel.
    fn apply(target: &mut Target, i: usize, amount: f64, critical: bool) -> ImpactResult {
        if critical {
            target.damage = 1.0;
            target.state = TargetState::Destroyed;
            return ImpactResult::Destroyed { target: i, critical: true };
        }
        target.damage = (target.damage + amount).min(1.0);
        // Rundungsreste (drei Drittel) zählen als ganz.
        if target.damage >= 1.0 - 1e-6 {
            target.damage = 1.0;
            target.state = TargetState::Destroyed;
            ImpactResult::Destroyed { target: i, critical: false }
        } else {
            target.state = TargetState::Damaged;
            ImpactResult::Damaged(i)
        }
    }

    /// Detonation bei `p`: wirkt auf alle intakten Ziele im Wirkradius.
    fn impact(&mut self, world: &mut World<'_>, w: &Weapon, p: DVec3) {
        let v = w.velocity;
        let impact_rad = detmath::atan2(v.z, (v.x * v.x + v.y * v.y).sqrt());
        let result = self.blast(world, w, p, impact_rad);
        self.events.push(world.tick, WeaponEvent::Impact { kind: w.kind, id: w.id, position: p, result });
        if (world.aircraft - p).length() < w.kind.spec().frag_radius_m {
            self.events.push(world.tick, WeaponEvent::FragWarning);
        }
    }

    /// Wirkung im Radius, nach Abstand vom Zielrand sortiert. Raketen würfeln
    /// wie im Original: Die Wirksamkeit ist die Chance, das Ziel zu zerstören,
    /// sonst wirkungslos. Bomben: Schaden fällt mit dem Abstand; ein voller
    /// Treffer würfelt einen kritischen.
    fn blast(&mut self, world: &mut World<'_>, w: &Weapon, p: DVec3, impact_rad: f64) -> ImpactResult {
        let reach = w.kind.spec().lethal_radius_m;
        let mut near: Vec<(f64, usize)> = world
            .targets
            .iter()
            .enumerate()
            .filter(|(_, t)| t.is_intact())
            .filter_map(|(i, t)| {
                let c = t.center_ned();
                let d = DVec3::new(c[0], c[1], c[2]) - p;
                let miss = ((d.x * d.x + d.y * d.y).sqrt() - t.class.radius())
                    .max(0.0)
                    .max(d.z.abs() - t.class.height());
                (miss <= reach).then_some((miss, i))
            })
            .collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let missile = w.kind.spec().cruise_mps > 0.0;
        let mut result = ImpactResult::Ground;
        for (miss, i) in near {
            let e = w.kind.effectiveness(world.targets[i].class, impact_rad);
            let x = (miss / reach - FULL_EFFECT_FRACTION) / (1.0 - FULL_EFFECT_FRACTION);
            let falloff = (1.0 - x).clamp(0.0, 1.0);
            let this = if e <= 0.0 {
                ImpactResult::Ineffective(i)
            } else if missile {
                if self.rng.next_f64() < e {
                    Self::apply(&mut world.targets[i], i, 1.0, false)
                } else {
                    ImpactResult::Ineffective(i)
                }
            } else if e * falloff < MIN_DAMAGE {
                ImpactResult::Ground
            } else {
                let critical = falloff >= 1.0 && self.rng.next_f64() < CRITICAL_CHANCE;
                Self::apply(&mut world.targets[i], i, e * falloff, critical)
            };
            if this.rank() > result.rank() {
                result = this;
            }
        }
        result
    }
}

#[cfg(test)]
mod tests;
