//! Flug und Lenkung: Suchkopf, Vorhalt, Bombenflug, Abwurfvorhersage.

use super::*;

/// Wie ein Flugschritt einer Waffe endet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Fate {
    Flying,
    /// Annäherungszünder am verfolgten Ziel (Detonationspunkt).
    Fuze(DVec3),
    /// Aufschlag auf Gelände oder Wasser.
    Ground(DVec3),
    /// Flugzeit überschritten.
    Expired,
}

/// Was ein Suchkopf von der Welt sieht.
pub(super) struct View<'a> {
    pub(super) targets: &'a [Target],
    pub(super) terrain: &'a Terrain,
    /// Gewähltes, optisch entdecktes Ziel des Flugzeugs (Laserbeleuchtung).
    pub(super) designated: Option<usize>,
    /// Ort und Geschwindigkeit des eigenen Flugzeugs, solange es fliegt.
    pub(super) aircraft: Option<(DVec3, DVec3)>,
    pub(super) decoys: &'a [Decoy],
    /// Die Stellung dieser (feindlichen) Rakete hat das Flugzeug beim letzten
    /// Umlauf erfasst (halbaktive Beleuchtung).
    pub(super) painted: bool,
    /// Das Flugzeug stört Radar- bzw. Infrarotlenkung.
    pub(super) jammed: (bool, bool),
    /// Sichtlinie prüfen (die Vorausberechnung spart sie sich).
    pub(super) los: bool,
}

impl View<'_> {
    /// Ort und Geschwindigkeit eines verfolgten Ziels (Bodenziele stehen);
    /// `None`, wenn es nicht mehr da ist.
    pub(super) fn locate(&self, track: Track) -> Option<(DVec3, DVec3)> {
        match track {
            Track::Ground(i) => {
                let c = self.targets.get(i)?.center_ned();
                Some((DVec3::new(c[0], c[1], c[2]), DVec3::ZERO))
            }
            Track::Aircraft => self.aircraft,
        }
    }
}

/// Peilungsabweichung (waagerecht, rad) der Richtung `to` von der
/// Flugrichtung – wie im Original prüfen Raketen gegen Luftziele nur die
/// Peilung, so findet auch eine steil gestartete ihr Ziel.
pub(super) fn bearing_off(velocity: DVec3, to: DVec3) -> f64 {
    let flat = |v: DVec3| DVec3::new(v.x, v.y, 0.0);
    if flat(velocity).length() < 1.0 { 0.0 } else { angle_between(flat(velocity), flat(to)) }
}

/// Suchkopf prüfen (gemeinsam für den echten Flug und die Vorausberechnung).
/// Ein Täuschkörper der passenden Art im Suchkegel blendet die Rakete (wie im
/// Original): Sie fliegt geradeaus, hält aber ihr Ziel. Sonst je Ziel:
/// - **Bodenziel:** Laserbombe – wird es noch beleuchtet? Rakete – liegt es
///   noch in Suchkegel und Reichweite (optisch und mit `los` auch in Sicht)?
///   Weit hinter ihr gibt sie auf.
/// - **Flugzeug:** Peilung im Suchkegel, sonst verloren; halbaktiv weiter als
///   [`SEMI_ACTIVE_M`] nur mit Beleuchtung durch die Stellung, optisch
///   (Infrarot) nur in Reichweite und mit freier Sicht.
/// - **Keines:** sucht selbst das nächste passende Bodenziel vor dem Suchkopf
///   (auch zivile), wenn sie ohne Übergabe abgefeuert wurde.
///
/// Liefert ein neues Ziel oder den Beginn einer Blendung als Ereignis.
pub(super) fn seek(w: &mut Weapon, v: &View<'_>) -> Option<WeaponEvent> {
    let s = w.kind.spec();
    // Erst fallen (eigene) bzw. im Boost steigen (feindliche).
    if w.age_s < s.boost_s.max(GUIDANCE_DELAY_S) {
        return None;
    }
    let blinded = w.target.is_some()
        && v.decoys.iter().any(|d| {
            let to = d.position - w.position;
            d.lures(w) && to.length() <= s.max_range_m && bearing_off(w.velocity, to) <= s.seeker_cone_rad
        });
    let newly = blinded && !w.blinded;
    w.blinded = blinded;
    if blinded {
        w.tracking = false;
        return newly.then_some(WeaponEvent::Decoyed { kind: w.kind, id: w.id });
    }
    match w.target {
        Some(Track::Ground(i)) => {
            let t = &v.targets[i];
            if s.seeker == Seeker::Laser {
                w.tracking = v.designated == Some(i) && t.is_intact();
                return None;
            }
            let c = t.center_ned();
            if angle_between(w.velocity, DVec3::new(c[0], c[1], c[2]) - w.position) > GIVE_UP_RAD {
                w.target = None;
                w.tracking = false;
                return None;
            }
            w.tracking =
                seeker_sees(w.kind, w.position, w.velocity, s.seeker_cone_rad, t, v.terrain, v.los).is_some();
            None
        }
        Some(track) => {
            let Some((p, _)) = v.locate(track) else {
                w.target = None;
                w.tracking = false;
                return None;
            };
            let to = p - w.position;
            // Suchkegel um die Richtung zum Ziel: Mit Vorhalt liegt ein
            // querab fliegendes Flugzeug neben der Nase; wird der
            // Vorhaltwinkel größer als der Kegel, verliert die Rakete es
            // (Querflug schüttelt ab, Spielerwunsch v0.2.9).
            let off = bearing_off(w.velocity, to);
            // Gestört: außerhalb kurzer Entfernung keine Lenkung auf das Flugzeug.
            let (ecm, ir) = v.jammed;
            let jammed = to.length() > BURN_THROUGH_M
                && match s.seeker {
                    Seeker::SemiActive => ecm,
                    Seeker::Optical => ir,
                    _ => false,
                };
            let sees = !jammed
                && match s.seeker {
                    Seeker::SemiActive => v.painted || to.length() <= SEMI_ACTIVE_M,
                    _ => {
                        to.length() <= s.max_range_m
                            && (!v.los
                                || line_of_sight(
                                    |n, e| v.terrain.elevation(n, e),
                                    [w.position.x, w.position.y, -w.position.z],
                                    [p.x, p.y, -p.z],
                                ))
                    }
                };
            w.tracking = off <= s.seeker_cone_rad && sees;
            // Fällt das Flugzeug aus dem Suchkegel, ist es verloren: Querflug
            // schüttelt ab (Spielerwunsch v0.2.9). Stören, fehlende
            // Beleuchtung und Täuschkörper wirken dagegen nur vorübergehend.
            if off > s.seeker_cone_rad {
                w.target = None;
            }
            None
        }
        None if w.searching => {
            let (_, i) = v
                .targets
                .iter()
                .enumerate()
                .filter_map(|(i, t)| {
                    seeker_sees(w.kind, w.position, w.velocity, s.seeker_cone_rad, t, v.terrain, v.los)
                        .map(|r| (r, i))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))?;
            w.target = Some(Track::Ground(i));
            w.tracking = true;
            w.searching = false;
            Some(WeaponEvent::Acquired { kind: w.kind, id: w.id, target: i })
        }
        None => None,
    }
}

/// Ein Zeitschritt einer Waffe (gemeinsam für den echten Flug und die
/// Vorausberechnung): bewegen, dann Annäherungszünder (Raketen: kürzester
/// Abstand der Strecke zum Zielpunkt `aim` unter dem Wirkradius),
/// Bodenaufschlag und Flugzeit prüfen. `aim` ist der Punkt, den der Suchkopf
/// gerade hat (Zielmitte bzw. Flugzeug) mit seiner Geschwindigkeit, sonst
/// `None`; gelenkt wird auf den Treffpunkt mit Vorhalt.
pub(super) fn advance(w: &mut Weapon, aim: Option<(DVec3, DVec3)>, terrain: &Terrain, dt: f64) -> Fate {
    let before = w.position;
    let s = w.kind.spec();
    let steer = aim.map(|(p, v)| intercept(w, p, v));
    match s.seeker {
        Seeker::None => fly(w, dt),
        Seeker::Laser => steer_bomb(w, steer, dt),
        _ => guide(w, steer, dt),
    }
    if s.cruise_mps > 0.0
        && let Some((aim, _)) = aim
    {
        let d = w.position - before;
        let k = if d.length() > 1e-9 { ((aim - before).dot(d) / d.dot(d)).clamp(0.0, 1.0) } else { 0.0 };
        let closest = before + d * k;
        if (aim - closest).length() < s.lethal_radius_m {
            return Fate::Fuze(closest);
        }
    }
    if let Some(p) = ground_crossing(terrain, before, w.position) {
        return Fate::Ground(p);
    }
    if w.age_s > s.life_s {
        return Fate::Expired;
    }
    Fate::Flying
}

/// Schneidet die Strecke `a`→`b` das Gelände (bzw. die Wasseroberfläche)?
/// Liefert den Schnittpunkt auf der Oberfläche.
pub(crate) fn ground_crossing(terrain: &Terrain, a: DVec3, b: DVec3) -> Option<DVec3> {
    let ground = terrain.elevation(b.x, b.y);
    if -b.z > ground {
        return None;
    }
    let h0 = -a.z - terrain.elevation(a.x, a.y);
    let h1 = -b.z - ground;
    let f = if h0 > h1 { (h0 / (h0 - h1)).clamp(0.0, 1.0) } else { 1.0 };
    let mut p = a + (b - a) * f;
    p.z = -terrain.elevation(p.x, p.y);
    Some(p)
}

/// Vorausberechneter Aufschlag einer jetzt ausgelösten ungelenkten Waffe:
/// gleiche Startwerte und Flugbahn wie beim echten Abwurf. Liefert Punkt
/// (NED, auf der Oberfläche) und Fallzeit (s); `None`, wenn die Waffe vor
/// [`MAX_FALL_S`] nicht aufschlägt.
pub fn predict_release(
    kind: WeaponKind,
    position: DVec3,
    velocity: DVec3,
    attitude: DQuat,
    terrain: &Terrain,
    dt: f64,
) -> Option<(DVec3, f64)> {
    let down = attitude.rotate(DVec3::new(0.0, 0.0, 1.0));
    let mut w = Weapon::new(0, kind, position + down * EJECT_OFFSET_M, velocity + down * EJECT_SPEED_MPS);
    // Grobe Schritte reichen für die Anzeige: 8 Simulationsschritte auf einmal,
    // mit gemittelter Geschwindigkeit (exakt für die Schwerkraft).
    let dt = dt * 8.0;
    // Oberhalb der höchsten möglichen Geländehöhe ist keine Abfrage nötig;
    // hoch über dem zuletzt gemessenen Boden reicht jeder vierte Schritt (in
    // vier groben Schritten kommt die Bombe kaum 200 m weit).
    let ceiling = terrain.max_elevation();
    let mut last_ground = f64::MIN;
    let mut since_check = 0;
    let mut density = 0.0;
    let mut steps = 0_u32;
    while w.age_s < MAX_FALL_S {
        let before = w;
        // Luftdichte ändert sich über wenige hundert Meter kaum: nur jeden
        // vierten Schritt neu berechnen.
        if steps.is_multiple_of(4) {
            density = atmosphere::isa(-w.position.z).density;
        }
        steps += 1;
        let v0 = w.velocity;
        fly_with_density(&mut w, dt, density);
        w.position = before.position + (v0 + w.velocity) * (0.5 * dt);
        let alt = -w.position.z;
        since_check += 1;
        if alt > ceiling || (alt - last_ground > PREDICT_MARGIN_M && since_check < 4) {
            continue;
        }
        since_check = 0;
        last_ground = terrain.elevation(w.position.x, w.position.y);
        if let Some(p) = ground_crossing(terrain, before.position, w.position) {
            let f = (p - before.position).length() / (w.position - before.position).length().max(1e-9);
            return Some((p, before.age_s + (w.age_s - before.age_s) * f));
        }
    }
    None
}

/// Ein Zeitschritt Bombenflugbahn: Schwerkraft und Widerstand (semi-implizit,
/// stabil bei 120 Hz).
pub(super) fn fly(w: &mut Weapon, dt: f64) {
    let density = atmosphere::isa(-w.position.z).density;
    fly_with_density(w, dt, density);
}

/// Wie [`fly`] mit vorgegebener Luftdichte.
pub(super) fn fly_with_density(w: &mut Weapon, dt: f64, density: f64) {
    let s = w.kind.spec();
    let speed = w.velocity.length();
    let drag_area = if w.chute_open() { s.deployed_drag_area_m2 } else { s.drag_area_m2 };
    let mut accel = DVec3::new(0.0, 0.0, G0);
    if speed > 0.0 {
        accel -= w.velocity / speed * (0.5 * density * speed * speed * drag_area / s.mass_kg);
    }
    w.velocity += accel * dt;
    w.position += w.velocity * dt;
    w.age_s += dt;
}

/// Dreht die Einheitsrichtung `from` um höchstens `max_angle` zu `to`.
pub(super) fn turn_towards(from: DVec3, to: DVec3, max_angle: f64) -> DVec3 {
    let angle = angle_between(from, to);
    if angle <= max_angle || angle < 1e-9 {
        return to;
    }
    let axis = from.cross(to);
    let axis = if axis.length() < 1e-9 { DVec3::new(0.0, 0.0, 1.0).cross(from) } else { axis };
    DQuat::from_axis_angle(axis.normalize_or_zero(), max_angle).rotate(from).normalize_or_zero()
}

/// Laserbombe: fällt physikalisch (Schwerkraft, Widerstand) und dreht mit
/// ihren Steuerflächen die Flugbahn mit begrenzter Rate zum Ziel, solange es
/// beleuchtet ist; sonst fällt sie wie eine normale Bombe.
pub(super) fn steer_bomb(w: &mut Weapon, aim: Option<DVec3>, dt: f64) {
    fly(w, dt);
    if w.age_s < GUIDANCE_DELAY_S {
        return;
    }
    if let Some(aim) = aim {
        let speed = w.velocity.length();
        let dir = w.velocity / speed.max(1.0);
        let wanted = (aim - w.position).normalize_or_zero();
        w.velocity = turn_towards(dir, wanted, w.kind.spec().turn_rate_rad * dt) * speed;
    }
}

/// Treffpunkt mit Vorhalt: wo das Ziel (bei `aim`, Geschwindigkeit `vel`)
/// nach der Restflugzeit sein wird. Für stehende Ziele die Zielmitte (reine
/// Verfolgung wie im Original).
pub(super) fn intercept(w: &Weapon, aim: DVec3, vel: DVec3) -> DVec3 {
    let s = w.kind.spec();
    let to = aim - w.position;
    let range = to.length();
    // Bogen: um einen Anteil der Entfernung über das Ziel zielen.
    let aim = aim + DVec3::new(0.0, 0.0, -s.loft * range);
    if vel == DVec3::ZERO {
        return aim;
    }
    let closing = (w.velocity - vel).dot(to / range.max(1.0)).max(1.0);
    aim + vel * (range / closing).min(MAX_LEAD_S)
}

/// Ein Zeitschritt Rakete: erst frei aus dem Schacht fallen, dann mit
/// begrenzter Drehrate auf das Ziel drehen (reine Verfolgung wie im Original)
/// und auf Marschfahrt gehen. Ohne Ziel fliegt sie geradeaus in ihrer
/// Abschusslage (`boresight`).
pub(super) fn guide(w: &mut Weapon, aim: Option<DVec3>, dt: f64) {
    // Eigene fallen aus dem Schacht; Boden-Luft-Raketen starten mit Schub
    // (im Boost ohne Ziel geradeaus).
    if w.owner.is_none() && w.age_s < GUIDANCE_DELAY_S {
        fly(w, dt);
        return;
    }
    let s = w.kind.spec();
    let speed = w.velocity.length().max(1.0);
    let dir = w.velocity / speed;
    let wanted = match aim {
        Some(aim) => (aim - w.position).normalize_or_zero(),
        None if w.target.is_some() || w.owner.is_some() => dir,
        None => w.boresight,
    };
    let new_dir = turn_towards(dir, wanted, s.turn_rate_rad * dt);
    let new_speed = speed + (s.cruise_mps - speed).clamp(-s.accel_mps2 * dt, s.accel_mps2 * dt);
    w.velocity = new_dir * new_speed;
    w.position += w.velocity * dt;
    w.age_s += dt;
}
