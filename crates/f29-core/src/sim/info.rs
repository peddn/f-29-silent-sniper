//! Anzeigedaten: was HUD, MFD, Ton und Autopilot vom Zustand sehen.

use super::*;

/// Warnungen für HUD und Ton.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Warnings {
    pub stall: bool,
    pub pull_up: bool,
    pub over_g: bool,
    pub crashed: bool,
    /// Weniger als 15 % Treibstoff.
    pub fuel_low: bool,
    /// Der Rest reicht nicht mehr bis zur Heimatbasis plus Reserve.
    pub bingo: bool,
    /// Tanks leer, Triebwerk aus.
    pub flameout: bool,
    /// Fahrwerk draußen und zu schnell.
    pub gear_overspeed: bool,
    /// Im Landeanflug, aber Fahrwerk nicht ausgefahren.
    pub gear_up: bool,
    /// Eine feindliche Rakete verfolgt das Flugzeug (Raketenwarnung, M-021).
    pub missile: bool,
}

/// Das gewählte Ziel (M-013).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TargetInfo {
    /// Index in [`Sim::targets`].
    pub index: usize,
    pub name: &'static str,
    pub class: TargetClass,
    /// Zivil oder befreundet: nicht angreifen.
    pub civilian: bool,
    pub role: TargetRole,
    /// Optisch erfasst: Zielbild verfügbar.
    pub optical: bool,
    /// Vom Radar erfasst.
    pub radar: bool,
    /// Schrägentfernung zur Zielmitte (m).
    pub range: f64,
    /// Peilung (rechtweisend, rad, 0..2π).
    pub bearing: f64,
    /// Richtung zur Zielmitte in Körperachsen, normiert.
    pub direction_body: DVec3,
    /// Vektor vom Flugzeug zum Zielfußpunkt (NED, m) – für das Zielbild.
    pub offset_ned: DVec3,
    /// Ausrichtung des Ziels (rad, 0 = Nord).
    pub heading: f64,
    /// Reichweitenzone der gewählten Lenkwaffe gegen dieses Ziel (Zielrahmen);
    /// `None`, wenn keine Lenkwaffe gewählt ist.
    pub zone: Option<LaunchZone>,
    /// Angesammelter Schaden 0..1.
    pub damage: f64,
    /// Zerstört: Die Wahl bleibt auf dem Wrack, bis ein neues Ziel
    /// gewählt wird.
    pub destroyed: bool,
}

/// Waffenbestand für die Anzeigen (M-014).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StoresInfo {
    pub bays: [Bay; weapons::BAYS],
    /// Gewählter Schacht (0-basiert); gilt nur, wenn nicht die Kanone gewählt ist.
    pub selected: usize,
    /// Die Bordkanone ist gewählt.
    pub gun_selected: bool,
    /// Schachtklappen 0 (zu) .. 1 (offen).
    pub door: f64,
    /// Eigene Waffen in der Luft.
    pub in_flight: usize,
    /// Vorrat an Düppeln und Fackeln.
    pub chaff: u8,
    pub flares: u8,
    /// Masse der Waffen an Bord (kg).
    pub mass_kg: f64,
}

/// Zielhilfe für ungelenkte Bomben (M-014): vorausberechneter Aufschlag.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BombAim {
    /// Aufschlagpunkt (NED, auf der Oberfläche).
    pub impact: DVec3,
    /// Richtung zum Aufschlagpunkt in Körperachsen, normiert.
    pub direction_body: DVec3,
    /// Fallzeit (s).
    pub fall_time_s: f64,
    /// Der Einschlag läge im Splitterbereich des weiterfliegenden Flugzeugs.
    pub too_low: bool,
    /// Ablage des gewählten (intakten) Ziels vom Aufschlagpunkt (m):
    /// längs der Flugrichtung (+ = Ziel liegt noch voraus, Bombe fiele zu kurz)
    /// und quer (+ = Ziel liegt rechts).
    pub target_offset: Option<(f64, f64)>,
    /// Wirkradius der gewählten Bombe (m), für die Auslöseanzeige.
    pub lethal_radius_m: f64,
}

/// Meldung zu einem Waffenereignis, z. B. „MK-82 AWAY“.
#[derive(Debug, Clone, PartialEq)]
pub struct WeaponMessage {
    pub text: String,
    /// Als Warnung zeigen (rot).
    pub warning: bool,
    /// Alter der Meldung (s).
    pub age_s: f64,
}

/// So lange bleibt eine Waffenmeldung stehen (s).
pub const WEAPON_MESSAGE_S: f64 = 4.0;

/// Informationen zum aktiven Navigationspunkt.
#[derive(Debug, Clone, PartialEq)]
pub struct SteerpointInfo {
    pub index: usize,
    pub name: String,
    /// Peilung (rechtweisend) zum Punkt, rad, 0..2π.
    pub bearing: f64,
    /// Horizontale Entfernung, m.
    pub range: f64,
    /// Richtung zum Punkt (am Boden) in Körperachsen, normiert.
    pub direction_body: DVec3,
    /// Bahnrichtung (rad, 0..π), falls der Punkt ein Flugplatz ist.
    pub runway_heading: Option<f64>,
}

/// Abgeleitete Flugdaten – alles, was Anzeigen brauchen.
/// Zustand der gewählten Kamera.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CameraInfo {
    pub photos: u32,
    /// Bild nur bei offenem Schacht und in der Luft.
    pub active: bool,
    pub in_frame: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FlightData {
    pub tick: u64,
    pub time_s: f64,
    pub status: Status,
    pub position: DVec3,
    pub altitude_m: f64,
    pub ground_elevation_m: f64,
    pub radar_altitude_m: f64,
    pub heading: f64,
    pub pitch: f64,
    pub roll: f64,
    /// Anstellwinkel des Flügels (ohne Durchsackwinkel, siehe
    /// [`flight::sink_angle`]).
    pub alpha: f64,
    pub beta: f64,
    pub tas_mps: f64,
    pub mach: f64,
    pub vertical_speed_mps: f64,
    pub load_factor: f64,
    /// Flugbahnrichtung in Körperachsen (für den Flight Path Marker).
    pub velocity_dir_body: DVec3,
    pub throttle: f64,
    pub engine: f64,
    /// Nettoschub (N).
    pub thrust_n: f64,
    pub airbrake: f64,
    /// Landeklappen 0..1.
    pub flaps: f64,
    /// Stärke der Turbulenz (Betrag der Störraten, rad/s).
    pub turbulence: f64,
    /// Treibstoff, kg.
    pub fuel_kg: f64,
    /// Tankinhalt 0..1.
    pub fuel_fraction: f64,
    /// Aktueller Verbrauch, kg/s.
    pub fuel_flow_kgps: f64,
    /// Restflugzeit beim aktuellen Verbrauch, s.
    pub endurance_s: f64,
    /// Fahrwerk 0..1.
    pub gear: f64,
    pub on_ground: bool,
    /// Steht still auf einer Bahn (gelandet bzw. startbereit).
    pub stopped_on_runway: bool,
    /// Letztes Aufsetzen, solange das Flugzeug am Boden ist.
    pub touchdown: Option<Touchdown>,
    /// Name der Bahn unter dem Flugzeug (am Boden).
    pub runway_name: Option<&'static str>,
    /// Entfernung zur Heimatbasis, m.
    pub home_range_m: f64,
    /// Treibstoff, der bis zur Heimatbasis plus Reserve gebraucht wird (BINGO-Grenze), kg.
    pub bingo_fuel_kg: f64,
    /// Aktuelle Masse, kg.
    pub mass_kg: f64,
    /// Staudruck, Pa.
    pub qbar_pa: f64,
    /// 1-g-Abrissfahrt bei aktueller Masse, Höhe und Klappenstellung, m/s.
    pub stall_speed_mps: f64,
    pub steerpoint: Option<SteerpointInfo>,
    /// Radar eingeschaltet.
    pub radar_on: bool,
    /// Störsender (ECM) an.
    pub ecm: bool,
    /// Restliche Wirkdauer des Infrarotstörers (s, 0 = aus).
    pub ir_jammer_s: f64,
    /// Restliche Abklingzeit des Infrarotstörers (s, 0 = bereit).
    pub ir_cooldown_s: f64,
    /// Blickrichtung der Zielerfassung.
    pub look: LookDir,
    /// Gewähltes Ziel.
    pub target: Option<TargetInfo>,
    /// Waffenbestand, Schacht, Waffen in der Luft.
    pub stores: StoresInfo,
    /// Aktuelle Waffenmeldungen, neueste zuletzt (höchstens drei).
    pub weapon_messages: Vec<WeaponMessage>,
    /// Zustand der Bordsysteme (Schadensmodell).
    pub systems: Systems,
    /// Indizes der zerstörten Ziele, aufsteigend (die Karte blendet sie aus).
    pub destroyed_targets: Vec<usize>,
    /// Indizes der Ziele, die die eigenen Sensoren erfassen, aufsteigend. Alle
    /// anderen kennt das Flugzeug nur über den AWACS-Datalink (verminderte
    /// Information: Lage und Zieltyp, aber keine Kennung „zivil“) – und nur,
    /// wenn ein AWACS in der Luft ist.
    pub own_contacts: Vec<usize>,
    /// Ein freundliches AWACS ist in der Luft (Datalink).
    pub awacs: bool,
    /// Regelset der Mission und ob das Flugzeug über feindlichem Gebiet ist
    /// (im Kalten Krieg feuert der Gegner nur dort).
    pub rules: &'static str,
    pub territory: Side,
    /// Auftrag je Ziel (Primär, Sekundär): Auftragsart und erledigt.
    pub objectives: [Option<(Task, bool)>; 2],
    /// Verletzungen der Einsatzregeln bisher.
    pub roe_violations: u32,
    /// Nach dem Missionsende: das Flugprotokoll als Zeilen „MM:SS TEXT“.
    pub debrief: Vec<String>,
    /// Jüngstes Auslösen (Tick, Art; die Kamera für ein Foto) – für den Klang.
    pub last_release: Option<(u64, WeaponKind)>,
    /// Kamera gewählt (M-032): Fotos bisher und ob das gewählte Ziel jetzt
    /// gültig im Bild wäre.
    pub camera: Option<CameraInfo>,
    /// Zielhilfe, wenn eine ungelenkte Bombe gewählt ist und das Flugzeug fliegt.
    pub bomb_aim: Option<BombAim>,
    /// Waffen in der Luft für die Karte: eigene und feindliche.
    pub munitions: Vec<MunitionInfo>,
    /// Täuschkörper in der Luft.
    pub decoys: Vec<DecoyInfo>,
    /// Feindliche Jäger in der Luft.
    pub fighters: Vec<FighterInfo>,
    /// Gewählter Jäger bzw. gewählte Landebahn.
    pub air_target: Option<PickedInfo>,
    pub runway_target: Option<PickedInfo>,
    /// Bekannte Ziele aller Arten vor dem Flugzeug.
    pub marks: Vec<Mark>,
    /// Bild der jüngsten eigenen Waffe mit Fernsehsuchkopf in der Luft.
    pub weapon_cam: Option<WeaponCam>,
    /// Jüngste Detonation (Klang).
    pub blast: Option<BlastInfo>,
    /// Bordkanone: Munition, Abzug und Aufschlagpunkt.
    pub gun: GunInfo,
    /// Lage gegenüber den feindlichen Radaren (M-020).
    pub threat: ThreatInfo,
    pub warnings: Warnings,
}

/// Lage gegenüber den feindlichen Radaren (M-020).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ThreatInfo {
    /// Eigenes Stealth-Level 0..1 (ein Wert für alle Radare).
    pub stealth: f64,
    /// Die Stellung mit dem stärksten Impuls am Flugzeug.
    pub worst: Option<ThreatSite>,
    /// Höchste Alarmstufe aller Stellungen.
    pub max_alert: u8,
    /// Letzter Ping, der das Flugzeug erreicht hat.
    pub last_ping: Option<PingInfo>,
    /// Jüngster gemerkter Ping-Ort (NED).
    pub ping_point: Option<DVec3>,
    /// Alle intakten Radarstellungen (für die Karte).
    pub sites: Vec<RadarSiteInfo>,
}

/// Eine Radarstellung in Reichweite.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThreatSite {
    /// Index in [`RadarNet::sites`].
    pub site: usize,
    /// Index in [`Sim::targets`].
    pub target: usize,
    pub name: &'static str,
    pub kind: RadarKind,
    /// Stärke ihres Impulses am Flugzeug 0..1.
    pub signal: f64,
    pub alert: u8,
    pub range_m: f64,
}

/// Eine Detonation für den Klang (jüngste der letzten halben Sekunde).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlastInfo {
    /// Eindeutig je Detonation (Tick und Waffennummer).
    pub id: u64,
    pub kind: BlastKind,
    /// Entfernung vom Flugzeug (m).
    pub distance_m: f64,
}

/// Art der Detonation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlastKind {
    /// Eigene Waffe hat ein Ziel beschädigt oder zerstört.
    TargetHit,
    /// Eine Waffe (eigene oder feindliche) hat verfehlt oder nicht gewirkt.
    Miss,
    /// Eine feindliche Rakete hat das Flugzeug getroffen.
    AircraftHit,
}

/// Letzter Ping.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PingInfo {
    pub tick: u64,
    pub age_s: f64,
    pub signal: f64,
    pub stealth: f64,
    pub hit: bool,
    /// Typ des pingenden Radars.
    pub kind: RadarKind,
}

/// Radarstellung für die Karte.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadarSiteInfo {
    pub target: usize,
    /// Nord, Ost (m).
    pub pos: [f64; 2],
    pub range_m: f64,
    pub kind: RadarKind,
    pub alert: u8,
}

/// Bordkanone für die Anzeigen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GunInfo {
    pub rounds: u16,
    /// Abzug gedrückt.
    pub firing: bool,
    /// Wo eine jetzt abgefeuerte Patrone auf dem Gelände einschlüge – nur
    /// innerhalb ihrer Flugzeit.
    pub aim: Option<GunAim>,
    /// Vorhaltemarke gegen den gewählten Jäger: Richtung (Körperachsen), in
    /// die die Nase zeigen muss; nur in Kanonenreichweite.
    pub lead: Option<DVec3>,
}

/// Aufschlagpunkt der Kanone.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GunAim {
    /// NED (m).
    pub impact: DVec3,
    /// Richtung zum Aufschlagpunkt in Körperachsen, normiert.
    pub direction_body: DVec3,
    /// Schrägentfernung (m).
    pub range_m: f64,
}

/// Eine Waffe in der Luft, wie die Karte sie zeigt.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MunitionInfo {
    /// NED (m).
    pub position: DVec3,
    /// NED (m/s).
    pub velocity: DVec3,
    /// Rakete (mit Antrieb) statt Bombe: Die Karte zeigt sie als Strich.
    pub missile: bool,
    /// Feindliche Waffe (Boden-Luft-Rakete).
    pub hostile: bool,
    /// Optischer bzw. Infrarot-Suchkopf statt Radarlenkung.
    pub optical: bool,
}

/// Gewählter Jäger bzw. gewählte Landebahn: was HUD und TGT-Seite zeigen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PickedInfo {
    pub kind: MarkKind,
    /// Nummer des Jägers bzw. Index der Landebahn.
    pub index: usize,
    pub label: &'static str,
    pub side: Side,
    /// Schrägentfernung (m) und Annäherung (m/s, positiv = kommt näher).
    pub range: f64,
    pub closure_mps: f64,
    /// Peilung (rad, rechtweisend) und Richtung in Körperachsen (normiert).
    pub bearing: f64,
    pub direction_body: DVec3,
    pub seen: Seen,
    /// Erfasst: im Erfassungskreis der Zielart.
    pub locked: bool,
}

/// Zielart einer HUD-Markierung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkKind {
    Ground,
    Air,
    Runway,
}

impl MarkKind {
    /// Halber Öffnungswinkel des Erfassungskreises (Landebahnen: keiner).
    pub fn circle(self) -> Option<f64> {
        match self {
            MarkKind::Ground => Some(sensors::GROUND_CIRCLE_RAD),
            MarkKind::Air => Some(sensors::AIR_CIRCLE_RAD),
            MarkKind::Runway => None,
        }
    }

    /// Liegt die Richtung `direction_body` im Erfassungskreis?
    pub fn in_circle(self, direction_body: DVec3) -> bool {
        self.circle().is_some_and(|c| detmath::acos(direction_body.x.clamp(-1.0, 1.0)) <= c)
    }
}

/// Stufe eines bekannten Ziels; dazu kommen „gewählt“ und „erfasst“.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Nur vom AWACS gemeldet: sichtbar, nicht wählbar.
    Reported,
    /// Von eigenem Radar oder optischem System entdeckt: wählbar.
    Detected,
}

/// Ein bekanntes Ziel (Viereck in HUD und Karte).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mark {
    pub kind: MarkKind,
    /// NED (m) und Richtung in Körperachsen (normiert).
    pub position: DVec3,
    pub direction_body: DVec3,
    pub stage: Stage,
    pub seen: Seen,
    /// Das gewählte Ziel seiner Art; erfasst von der Waffe (Bodenziel in
    /// Reichweite, Jäger im Erfassungskreis).
    pub selected: bool,
    pub locked: bool,
}

/// Ein feindlicher Jäger in der Luft (Karte, 3D, HUD).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FighterInfo {
    pub id: u32,
    pub label: &'static str,
    /// NED (m) und Geschwindigkeit (m/s).
    pub position: DVec3,
    pub velocity: DVec3,
    /// Kurs und Querlage (rad).
    pub heading: f64,
    pub bank: f64,
    /// Sein Radar sieht das Flugzeug gerade.
    pub pursuing: bool,
}

/// Ein Täuschkörper in der Luft (Karte).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DecoyInfo {
    pub kind: DecoyKind,
    /// NED (m).
    pub position: DVec3,
}

/// Suchkopfbild einer Waffe im Flug (WPN-CAM-Seite).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeaponCam {
    pub id: u32,
    pub kind: WeaponKind,
    /// NED (m).
    pub position: DVec3,
    /// Blickrichtung (NED, Einheitsvektor): zur Zielmitte, solange der
    /// Suchkopf das Ziel verfolgt, sonst in Flugrichtung.
    pub look: DVec3,
    /// Verfolgtes Ziel und Entfernung zu seiner Mitte (m).
    pub tracking: Option<(usize, f64)>,
}

impl Sim {
    /// Lage gegenüber den feindlichen Radaren für die Anzeigen.
    fn threat_info(&self) -> ThreatInfo {
        let x = self.exposure();
        let mut worst: Option<ThreatSite> = None;
        for (k, s) in self.radar.sites.iter().enumerate() {
            let t = &self.targets[s.target];
            if !t.is_intact() {
                continue;
            }
            let c = t.center_ned();
            let at = DVec3::new(c[0], c[1], c[2]);
            let range_m = (at - x.position).length();
            let signal = x.signal(&s.spec, at);
            if signal <= 0.0 {
                continue;
            }
            let site = ThreatSite {
                site: k,
                target: s.target,
                name: t.name,
                kind: s.spec.kind,
                signal,
                alert: s.alert,
                range_m,
            };
            if worst.is_none_or(|w| site.signal > w.signal) {
                worst = Some(site);
            }
        }
        ThreatInfo {
            stealth: x.stealth(),
            worst,
            max_alert: self.radar.max_alert(),
            last_ping: self.radar.last_ping.map(|p| PingInfo {
                tick: p.tick,
                age_s: self.tick.saturating_sub(p.tick) as f64 * DT,
                signal: p.signal,
                stealth: p.stealth,
                hit: p.hit,
                kind: self.radar.sites[p.site].spec.kind,
            }),
            ping_point: self.radar.latest_ping_point(),
            sites: self
                .radar
                .sites
                .iter()
                .filter(|s| self.targets[s.target].is_intact())
                .map(|s| {
                    let t = &self.targets[s.target];
                    RadarSiteInfo {
                        target: s.target,
                        pos: [t.north, t.east],
                        range_m: s.spec.range_m,
                        kind: s.spec.kind,
                        alert: s.alert,
                    }
                })
                .collect(),
        }
    }

    /// Alle bekannten Ziele für die Vierecke in HUD und Karte: Bodenziele
    /// und Jäger, die die eigenen Sensoren entdecken oder das AWACS meldet;
    /// Landebahnen immer (von der Karte).
    fn marks(&self) -> Vec<Mark> {
        let a = &self.aircraft;
        let awacs = self.config.spawn.awacs;
        let p = &self.picks;
        let zone_locked = matches!(self.launch_zone, Some((_, LaunchZone::Locked | LaunchZone::Good)));
        let ground = self.targets.iter().enumerate().filter(|(_, t)| t.is_intact());
        let ground = ground.map(|(i, t)| {
            let selected = p.ground.index == Some(i);
            (MarkKind::Ground, super::center(t), self.ground_seen[i], selected, selected && zone_locked)
        });
        let air = self.fighters.list.iter().filter(|f| f.airborne()).map(|f| {
            let selected = p.air.index == Some(f.id as usize);
            (
                MarkKind::Air,
                f.position,
                self.fighter_seen[f.id as usize],
                selected,
                selected && self.air_locked(f.position),
            )
        });
        let known = Seen { radar: false, optical: true };
        let runways = self.terrain.runways().iter().enumerate();
        let runways = runways
            .map(|(i, r)| (MarkKind::Runway, runway_point(r), known, p.runway.index == Some(i), false));
        ground
            .chain(air)
            .filter(|m| awacs || m.2.any())
            .chain(runways)
            .map(|(kind, position, seen, selected, locked)| Mark {
                kind,
                position,
                direction_body: a.attitude.inverse_rotate((position - a.position).normalize_or_zero()),
                stage: if seen.any() { Stage::Detected } else { Stage::Reported },
                seen,
                selected,
                locked,
            })
            .collect()
    }

    /// Angaben zum gewählten Ziel `index` der Art `kind` bei `position`
    /// (NED), das sich mit `velocity` bewegt.
    fn picked_info(
        &self,
        kind: MarkKind,
        index: usize,
        label: &'static str,
        side: Side,
        position: DVec3,
        velocity: DVec3,
    ) -> PickedInfo {
        let a = &self.aircraft;
        let to = position - a.position;
        let (range, dir) = (to.length(), to.normalize_or_zero());
        let direction_body = a.attitude.inverse_rotate(dir);
        PickedInfo {
            kind,
            index,
            label,
            side,
            range,
            closure_mps: -(velocity - a.velocity).dot(dir),
            bearing: detmath::wrap_two_pi(detmath::atan2(to.y, to.x)),
            direction_body,
            // Landebahnen sind von der Karte bekannt; optisch sichtbar, wenn
            // in Reichweite mit freier Sicht (Bild der TGT-Seite).
            seen: match kind {
                MarkKind::Air => self.picks.air.seen,
                MarkKind::Runway => Seen {
                    radar: false,
                    optical: self
                        .sensors
                        .sees(&self.terrain, a.position, position - DVec3::new(0.0, 0.0, 5.0))
                        .optical,
                },
                MarkKind::Ground => Seen::default(),
            },
            locked: if kind == MarkKind::Air {
                self.air_locked(position)
            } else {
                kind.in_circle(direction_body)
            },
        }
    }

    /// Erfasst die gewählte Waffe den gewählten Jäger bei `at`? Im
    /// Erfassungskreis und in Reichweite: Kanone bis [`gun::AIR_RANGE_M`],
    /// Luft-Luft-Rakete mit ihrem Suchkopf.
    fn air_locked(&self, at: DVec3) -> bool {
        let a = &self.aircraft;
        let to = at - a.position;
        if !MarkKind::Air.in_circle(a.attitude.inverse_rotate(to.normalize_or_zero())) {
            return false;
        }
        if self.stores.gun_selected {
            return to.length() <= gun::AIR_RANGE_M;
        }
        self.stores
            .selected_weapon()
            .is_some_and(|k| k.takes_aircraft(to.length(), self.picks.air.seen.optical))
    }

    /// Anzeige- und Autopilot-Daten für den aktuellen Zustand.
    pub fn flight_data(&self) -> FlightData {
        let a = &self.aircraft;
        let aero = &self.aero;
        let (yaw, pitch, roll) = a.attitude.to_euler();
        let ground = self.terrain.elevation(a.position.x, a.position.y);
        // Radarhöhe bezogen auf die Räder: am Boden 0.
        let radar_alt = (a.altitude() - ground - self.spec.gear_height_m).max(0.0);
        let vs = -a.velocity.z;
        let vel_body = a.attitude.inverse_rotate(a.velocity).normalize_or_zero();
        let velocity_dir_body = if vel_body == DVec3::ZERO { DVec3::X } else { vel_body };
        let crashed = matches!(self.status, Status::Crashed { .. });
        let airborne = !crashed && !a.on_ground;
        let mass = a.mass(&self.spec);
        let cm = &self.stores.countermeasures;
        let (ir_jammer_s, ir_cooldown_s) = cm.ir_left();

        let target = self.picks.ground.index.and_then(|i| {
            let seen = self.picks.ground.seen;
            let t = self.targets.get(i)?;
            let c = t.center_ned();
            let to_center = DVec3::new(c[0] - a.position.x, c[1] - a.position.y, c[2] - a.position.z);
            Some(TargetInfo {
                index: i,
                name: t.name,
                class: t.class,
                civilian: t.civilian,
                role: t.role,
                optical: seen.optical,
                radar: seen.radar,
                range: to_center.length(),
                bearing: detmath::wrap_two_pi(detmath::atan2(to_center.y, to_center.x)),
                direction_body: a.attitude.inverse_rotate(to_center).normalize_or_zero(),
                offset_ned: DVec3::new(
                    t.north - a.position.x,
                    t.east - a.position.y,
                    -t.elevation - a.position.z,
                ),
                heading: t.heading,
                zone: self.launch_zone.filter(|(l, _)| *l == (i, seen)).map(|(_, z)| z),
                damage: t.damage,
                destroyed: !t.is_intact(),
            })
        });

        let steerpoint = self.route.get(self.active_steerpoint).map(|sp| {
            let dn = sp.north - a.position.x;
            let de = sp.east - a.position.y;
            let to_point = DVec3::new(dn, de, -sp.elevation - a.position.z);
            SteerpointInfo {
                index: self.active_steerpoint,
                name: sp.name.clone(),
                bearing: detmath::wrap_two_pi(detmath::atan2(de, dn)),
                range: (dn * dn + de * de).sqrt(),
                direction_body: a.attitude.inverse_rotate(to_point).normalize_or_zero(),
                runway_heading: sp
                    .runway
                    .and_then(|i| self.terrain.runways().get(i))
                    .map(|rw| rw.heading_deg * (std::f64::consts::PI / 180.0)),
            }
        });

        let time_to_impact = if vs < -0.5 { radar_alt / -vs } else { f64::INFINITY };
        // Abrisswarnung: knapp über der (g-abhängigen) Abrissgeschwindigkeit,
        // bei hohem Anstellwinkel oder wenn der Abriss schon einsetzt.
        let n_demand = flight::demanded_load_factor(&self.spec, a, &self.last_controls, aero.tas, aero.qbar);
        let v_stall = flight::stall_speed(&self.spec, mass, aero.density, n_demand, a.flaps);

        // Treibstoff: Restzeit beim aktuellen Verbrauch und Bedarf bis nach Hause.
        let flow = if a.flamed_out() { 0.0 } else { flight::fuel_flow(&self.spec, a.engine, aero.density) };
        let endurance_s = if flow > 0.0 { a.fuel_kg / flow } else { 0.0 };
        let home_range_m = self.terrain.runways().first().map_or(0.0, |h| {
            let (dn, de) = (h.center[0] - a.position.x, h.center[1] - a.position.y);
            (dn * dn + de * de).sqrt()
        });
        // Rückweg mit mindestens Reisefahrt und mindestens halbem Schub gerechnet.
        let cruise_flow = flight::fuel_flow(&self.spec, a.engine.max(0.5), aero.density);
        let fuel_needed = home_range_m / aero.tas.max(150.0) * cruise_flow + FUEL_RESERVE_KG;
        let runway_here = self.terrain.runway_at(a.position.x, a.position.y);
        let stopped_on_runway = a.on_ground && runway_here.is_some() && a.velocity.length() < STOPPED_SPEED;
        // Landeanflug: tief, sinkend, langsam.
        let approach = radar_alt < 300.0
            && vs < -1.0
            && aero.tas < 1.6 * self.spec.stall_speed_1g(mass, aero.density, 0.0);

        let warnings = Warnings {
            stall: airborne
                && (a.stall > 0.02
                    || aero.wing_alpha.abs() >= self.spec.alpha_warning_rad
                    || aero.tas < STALL_WARNING_MARGIN * v_stall)
                && aero.tas > 1.0,
            // Mit ausgefahrenem Fahrwerk nur bei gefährlicher Sinkrate (Landeanflug).
            pull_up: airborne
                && radar_alt < 900.0
                && time_to_impact < 6.0
                && (!a.gear_down() || -vs > 2.0 * self.spec.max_landing_sink_mps),
            over_g: airborne && aero.load_factor > self.spec.g_limit_pos * 0.95,
            crashed,
            fuel_low: !crashed && a.fuel_kg < FUEL_LOW_FRACTION * self.spec.fuel_capacity_kg,
            bingo: airborne && a.fuel_kg < fuel_needed,
            flameout: !crashed && a.flamed_out(),
            gear_overspeed: airborne && a.gear > 0.05 && aero.tas > self.spec.gear_max_speed_mps,
            gear_up: airborne && !a.gear_down() && approach,
            missile: airborne
                && self
                    .stores
                    .in_flight
                    .iter()
                    .any(|w| w.owner.is_some() && w.target == Some(Track::Aircraft)),
        };

        FlightData {
            tick: self.tick,
            time_s: self.time_s(),
            status: self.status,
            position: a.position,
            altitude_m: a.altitude(),
            ground_elevation_m: ground,
            radar_altitude_m: radar_alt,
            heading: detmath::wrap_two_pi(yaw),
            pitch,
            roll,
            alpha: aero.wing_alpha,
            beta: aero.beta,
            tas_mps: aero.tas,
            mach: aero.mach,
            vertical_speed_mps: vs,
            load_factor: aero.load_factor,
            velocity_dir_body,
            throttle: self.last_controls.throttle_f(),
            engine: a.engine,
            thrust_n: aero.thrust,
            airbrake: a.airbrake,
            flaps: a.flaps,
            turbulence: self.gust.length(),
            fuel_kg: a.fuel_kg,
            fuel_fraction: a.fuel_kg / self.spec.fuel_capacity_kg,
            fuel_flow_kgps: flow,
            endurance_s,
            gear: a.gear,
            on_ground: a.on_ground,
            stopped_on_runway,
            touchdown: if a.on_ground { self.touchdown } else { None },
            runway_name: if a.on_ground { runway_here.map(|i| self.terrain.runways()[i].name) } else { None },
            home_range_m,
            bingo_fuel_kg: fuel_needed,
            mass_kg: mass,
            qbar_pa: aero.qbar,
            stall_speed_mps: self.spec.stall_speed_1g(mass, aero.density, a.flaps),
            steerpoint,
            radar_on: self.sensors.radar_on,
            ecm: cm.ecm,
            ir_jammer_s,
            ir_cooldown_s,
            look: self.picks.ground.look,
            target,
            stores: StoresInfo {
                bays: self.stores.bays,
                selected: self.stores.selected,
                gun_selected: self.stores.gun_selected,
                door: self.stores.door,
                in_flight: self.stores.in_flight.iter().filter(|w| w.owner.is_none()).count(),
                chaff: cm.chaff,
                flares: cm.flares,
                mass_kg: self.stores.mass_kg(),
            },
            weapon_messages: self.weapon_messages(),
            systems: self.systems,
            awacs: self.config.spawn.awacs,
            rules: self.rules().label,
            territory: self.territory(a.position),
            objectives: [0, 1].map(|k| {
                [self.mission.primary, self.mission.secondary][k]
                    .map(|_| (self.mission.tasks[k], self.objective_done(k)))
            }),
            roe_violations: self.roe_violations,
            debrief: if self.status == Status::Flying {
                Vec::new()
            } else {
                self.log
                    .iter()
                    .map(|&(tick, e)| {
                        let s = (tick as f64 * DT) as u64;
                        format!("{:02}:{:02} {}", s / 60, s % 60, self.log_text(e))
                    })
                    .collect()
            },
            last_release: self.stores.events.iter().rev().find_map(|&(tick, e)| match e {
                WeaponEvent::Released { kind, .. } => Some((tick, kind)),
                WeaponEvent::Photo { .. } => Some((tick, WeaponKind::Camera)),
                _ => None,
            }),
            camera: (self.stores.selected_weapon() == Some(WeaponKind::Camera)).then(|| {
                let active = self.stores.door_open() && !a.on_ground;
                CameraInfo {
                    photos: self.stores.photos,
                    active,
                    in_frame: active
                        && self
                            .picks
                            .ground
                            .index
                            .filter(|_| self.picks.ground.seen.optical)
                            .is_some_and(|i| weapons::in_photo(a.position, a.attitude, &self.targets[i])),
                }
            }),
            own_contacts: (0..self.targets.len())
                .filter(|&i| self.ground_seen[i].any() && self.targets[i].is_intact())
                .collect(),
            bomb_aim: self.bomb_aim(),
            gun: GunInfo {
                rounds: self.gun.rounds,
                firing: self.gun.trigger,
                lead: self
                    .picks
                    .air
                    .index
                    .and_then(|i| self.fighters.list.get(i))
                    .filter(|f| f.airborne() && (f.position - a.position).length() <= gun::AIR_RANGE_M * 1.5)
                    .map(|f| {
                        a.attitude.inverse_rotate(gun::lead(a.position, a.velocity, f.position, f.velocity()))
                    }),
                aim: self.gun_prediction.map(|(impact, _)| {
                    let to = impact - self.aircraft.position;
                    GunAim {
                        impact,
                        direction_body: self.aircraft.attitude.inverse_rotate(to).normalize_or_zero(),
                        range_m: to.length(),
                    }
                }),
            },
            threat: self.threat_info(),
            blast: self.blast(),
            munitions: self
                .stores
                .in_flight
                .iter()
                .map(|w| MunitionInfo {
                    position: w.position,
                    velocity: w.velocity,
                    missile: w.kind.spec().cruise_mps > 0.0,
                    hostile: w.owner.is_some(),
                    optical: w.kind.spec().seeker == weapons::Seeker::Optical,
                })
                .collect(),
            decoys: cm.decoys.iter().map(|d| DecoyInfo { kind: d.kind, position: d.position }).collect(),
            air_target: self
                .picks
                .air
                .index
                .and_then(|i| self.fighters.list.get(i))
                .filter(|f| f.airborne())
                .map(|f| {
                    self.picked_info(
                        MarkKind::Air,
                        f.id as usize,
                        f.spec().label,
                        Side::Hostile,
                        f.position,
                        f.velocity(),
                    )
                }),
            runway_target: self.picks.runway.index.map(|i| {
                let r = &self.terrain.runways()[i];
                let side = match r.kind {
                    AirfieldKind::Home => Side::Friendly,
                    AirfieldKind::Enemy => Side::Hostile,
                };
                self.picked_info(MarkKind::Runway, i, r.name, side, runway_point(r), DVec3::ZERO)
            }),
            marks: self.marks(),
            fighters: self
                .fighters
                .list
                .iter()
                .filter(|f| {
                    f.airborne() && (self.config.spawn.awacs || self.fighter_seen[f.id as usize].any())
                })
                .map(|f| FighterInfo {
                    id: f.id,
                    label: f.spec().label,
                    position: f.position,
                    velocity: f.velocity(),
                    heading: f.heading,
                    bank: f.bank,
                    pursuing: f.mode == crate::fighters::Mode::Pursue,
                })
                .collect(),
            weapon_cam: self
                .stores
                .in_flight
                .iter()
                .rev()
                .find(|w| w.owner.is_none() && w.kind.has_camera())
                .map(|w| {
                    let aim = w.aim_point(&self.targets).map(|p| p - w.position);
                    WeaponCam {
                        id: w.id,
                        kind: w.kind,
                        position: w.position,
                        look: aim.unwrap_or(w.velocity).normalize_or_zero(),
                        tracking: w.ground_target().zip(aim).map(|(i, d)| (i, d.length())),
                    }
                }),
            destroyed_targets: (0..self.targets.len()).filter(|&i| !self.targets[i].is_intact()).collect(),
            warnings,
        }
    }

    /// Jüngste Detonation der letzten halben Sekunde: Treffer am Ziel,
    /// Fehlschuss (auch feindliche Raketen) oder Treffer am eigenen Flugzeug.
    fn blast(&self) -> Option<BlastInfo> {
        let recent = self.tick.saturating_sub((0.5 / DT) as u64);
        self.stores.events.iter().rev().take_while(|(t, _)| *t >= recent).find_map(|&(tick, e)| {
            let (id, kind, at) = match e {
                WeaponEvent::Impact { id, position, result, .. } => {
                    let hit = matches!(result, ImpactResult::Destroyed { .. } | ImpactResult::Damaged(_));
                    (id, if hit { BlastKind::TargetHit } else { BlastKind::Miss }, position)
                }
                WeaponEvent::AircraftHit { id, .. } => (id, BlastKind::AircraftHit, self.aircraft.position),
                WeaponEvent::FighterHit { id, fighter, .. } => {
                    (id, BlastKind::TargetHit, self.fighters.list[fighter].position)
                }
                _ => return None,
            };
            let distance_m = (at - self.aircraft.position).length();
            Some(BlastInfo { id: tick << 16 | u64::from(id & 0xFFFF), kind, distance_m })
        })
    }

    /// Zielhilfe aus der zuletzt berechneten Aufschlagvorhersage.
    fn bomb_aim(&self) -> Option<BombAim> {
        let kind = self.stores.selected_weapon().filter(|k| k.is_bomb())?;
        let (impact, fall_time_s) = self.bomb_prediction?;
        let a = &self.aircraft;
        let to = impact - a.position;
        // Wo das Flugzeug beim Einschlag wäre (geradeaus weiter).
        let later = a.position + a.velocity * fall_time_s;
        let too_low = (later - impact).length() < kind.spec().frag_radius_m;
        let h = DVec3::new(a.velocity.x, a.velocity.y, 0.0).normalize_or_zero();
        let right = DVec3::new(-h.y, h.x, 0.0);
        let target_offset = self.picks.ground.index.and_then(|i| {
            let t = self.targets.get(i).filter(|t| t.is_intact())?;
            let d = DVec3::new(t.north - impact.x, t.east - impact.y, 0.0);
            Some((d.dot(h), d.dot(right)))
        });
        Some(BombAim {
            impact,
            direction_body: a.attitude.inverse_rotate(to).normalize_or_zero(),
            fall_time_s,
            too_low,
            target_offset,
            lethal_radius_m: kind.spec().lethal_radius_m,
        })
    }

    /// Meldungen der letzten [`WEAPON_MESSAGE_S`] Sekunden, neueste zuletzt.
    fn weapon_messages(&self) -> Vec<WeaponMessage> {
        let name = |i: usize| self.targets[i].name;
        let window = (WEAPON_MESSAGE_S / DT) as u64;
        let message = |tick: u64, (text, warning): (String, bool)| {
            (tick, WeaponMessage { text, warning, age_s: self.tick.saturating_sub(tick) as f64 * DT })
        };
        let mut all: Vec<(u64, WeaponMessage)> = self
            .stores
            .events
            .recent(self.tick, window)
            .map(|&(tick, e)| {
                let text = match e {
                    WeaponEvent::Released { kind, .. } => (format!("{} AWAY", kind.label()), false),
                    WeaponEvent::BayClosed => ("BAY CLOSED".to_string(), true),
                    WeaponEvent::BayFault => ("BAY FAULT".to_string(), true),
                    WeaponEvent::BayEmpty => ("BAY EMPTY".to_string(), true),
                    WeaponEvent::BankLimit => ("BANK LIMIT".to_string(), true),
                    WeaponEvent::Busy => ("WEAPONS BUSY".to_string(), true),
                    WeaponEvent::FragWarning => ("FRAG WARNING".to_string(), true),
                    WeaponEvent::Expired { kind, .. } => (format!("{} LOST", kind.label()), false),
                    WeaponEvent::SamLaunch { kind, site, .. } => {
                        (format!("MISSILE LAUNCH {} - {}", kind.label(), name(site)), true)
                    }
                    WeaponEvent::AirLaunch { kind, fighter, .. } => {
                        let by = self.fighters.list[fighter].spec().label;
                        (format!("MISSILE LAUNCH {} - {by}", kind.label()), true)
                    }
                    WeaponEvent::AircraftHit { kind, .. } => (format!("{} HIT", kind.label()), true),
                    WeaponEvent::FighterHit { kind, .. } => (format!("{} HIT", kind.label()), false),
                    WeaponEvent::Photo { n, .. } => (format!("PHOTO {n}"), false),
                    WeaponEvent::Decoyed { kind, .. } => (format!("{} DECOYED", kind.label()), false),
                    WeaponEvent::DecoyEmpty(kind) => (format!("{} EMPTY", kind.label()), true),
                    WeaponEvent::Acquired { kind, target, .. } => {
                        (format!("{} LOCK {}", kind.label(), name(target)), self.targets[target].civilian)
                    }
                    WeaponEvent::LaserLost { kind, .. } => (format!("{} LASER LOST", kind.label()), true),
                    WeaponEvent::Impact { kind, result, .. } => match result {
                        ImpactResult::Destroyed { target: i, .. } if self.targets[i].civilian => {
                            (format!("CIVILIAN {} DESTROYED", name(i)), true)
                        }
                        ImpactResult::Destroyed { target: i, critical: true } => {
                            (format!("CRITICAL HIT {} DESTROYED", name(i)), false)
                        }
                        ImpactResult::Destroyed { target: i, .. } => {
                            (format!("{} DESTROYED", name(i)), false)
                        }
                        ImpactResult::Damaged(i) => (format!("{} DAMAGED", name(i)), false),
                        ImpactResult::Ineffective(i) => (format!("INEFFECTIVE ON {}", name(i)), true),
                        ImpactResult::Ground => (format!("{} MISSED", kind.label()), false),
                    },
                };
                message(tick, text)
            })
            .chain(self.radar.events.recent(self.tick, window).filter_map(|&(tick, e)| {
                let site_name = |k: usize| self.targets[self.radar.sites[k].target].name;
                let text = match e {
                    RadarEvent::Alert { site, level: 1 } => {
                        (format!("RADAR DETECTED - {}", site_name(site)), true)
                    }
                    RadarEvent::Alert { site, level: 2 } => (format!("TRACKED - {}", site_name(site)), true),
                    RadarEvent::Alert { site, .. } => (format!("RADAR ID - {}", site_name(site)), true),
                    RadarEvent::Lost { site } => (format!("{} LOST CONTACT", site_name(site)), false),
                    RadarEvent::Fire { .. } => return None,
                };
                Some(message(tick, text))
            }))
            .chain(self.gun.events.recent(self.tick, window).filter_map(|&(tick, e)| {
                let text = match e {
                    GunEvent::Destroyed(i) if self.targets[i].civilian => {
                        (format!("CIVILIAN {} DESTROYED", name(i)), true)
                    }
                    GunEvent::Destroyed(i) => (format!("GUN {} DESTROYED", name(i)), false),
                    GunEvent::Empty => ("GUN EMPTY".to_string(), true),
                    GunEvent::AirHit(_) => return None,
                };
                Some(message(tick, text))
            }))
            .chain(self.mission_events.recent(self.tick, window).map(|&(tick, e)| {
                let text = match e {
                    MissionEvent::Photographed(0) => ("PRIMARY PHOTOGRAPHED".to_string(), false),
                    MissionEvent::Photographed(_) => ("SECONDARY PHOTOGRAPHED".to_string(), false),
                    MissionEvent::RoeViolation => ("ROE VIOLATION".to_string(), true),
                };
                message(tick, text)
            }))
            .chain(self.fighters.events.recent(self.tick, window).map(|&(tick, e)| {
                let base = |b: usize| self.terrain.runways()[b].name;
                let text = match e {
                    FighterEvent::Takeoff { kind, base: b } => {
                        (format!("{} - {} TAKEOFF", base(b), kind.spec().label), true)
                    }
                    FighterEvent::Contact { kind } => (format!("{} RADAR CONTACT", kind.spec().label), true),
                    FighterEvent::Landed { kind, base: b } => {
                        (format!("{} - {} LANDED", base(b), kind.spec().label), false)
                    }
                    FighterEvent::Destroyed { kind } => (format!("{} DESTROYED", kind.spec().label), false),
                    FighterEvent::GunHit { kind } => (format!("{} GUN HIT", kind.spec().label), true),
                };
                message(tick, text)
            }))
            .collect();
        // Waffen und Kanone zeitlich mischen (stabil: gleiche Ticks in Reihenfolge).
        all.sort_by_key(|(tick, _)| *tick);
        let mut out: Vec<WeaponMessage> = all.into_iter().map(|(_, m)| m).collect();
        let skip = out.len().saturating_sub(3);
        out.drain(..skip);
        out
    }
}
