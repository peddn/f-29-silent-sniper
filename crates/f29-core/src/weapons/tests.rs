use super::*;

#[test]
fn loadout_text_roundtrip() {
    let l = Loadout::default();
    assert_eq!(l.to_text(), "fall,chute,lgb,agm");
    assert_eq!(Loadout::parse(&l.to_text()), Some(l));
    let e = Loadout::parse("arm,-,ship,fall").unwrap();
    assert_eq!(e.0[1], None);
    assert_eq!(Loadout::parse("arm,ship"), None);
    assert_eq!(Loadout::parse("arm,ship,x,fall"), None);
}

#[test]
fn damage_per_hit_matrix() {
    let k = WeaponKind::FreeFall;
    assert_eq!(k.effectiveness(TargetClass::Soft, 0.5), 1.0);
    assert_eq!(k.effectiveness(TargetClass::Bridge, 0.5), 0.5, "Brücke: zwei Bomben");
    assert_eq!(k.effectiveness(TargetClass::Hardened, deg(40.0)), 0.0);
    assert_eq!(k.effectiveness(TargetClass::Hardened, deg(70.0)), 0.5);
    assert_eq!(WeaponKind::AntiRadar.effectiveness(TargetClass::Soft, 1.0), 0.0);
    assert_eq!(WeaponKind::AntiShip.effectiveness(TargetClass::Building, 1.0), 0.0);
    for w in WeaponKind::ALL {
        for c in TargetClass::ALL {
            let e = w.effectiveness(c, 1.2);
            assert!((0.0..=1.0).contains(&e), "{w:?} gegen {c:?}");
        }
    }
}

fn bridge() -> Target {
    Target {
        name: "BRIDGE",
        class: TargetClass::Bridge,
        civilian: false,
        role: crate::targets::TargetRole::None,
        north: 0.0,
        east: 0.0,
        elevation: 0.0,
        heading: 0.0,
        state: TargetState::Intact,
        damage: 0.0,
    }
}

#[test]
fn damage_accumulates_and_critical_destroys_at_once() {
    let e = WeaponKind::FreeFall.effectiveness(TargetClass::Bridge, 0.5);
    let mut t = bridge();
    assert_eq!(Munitions::apply(&mut t, 7, e, false), ImpactResult::Damaged(7));
    assert_eq!(t.state, TargetState::Damaged);
    assert!(t.is_intact());
    assert_eq!(Munitions::apply(&mut t, 7, e, false), ImpactResult::Destroyed { target: 7, critical: false });
    assert!(!t.is_intact());
    let mut t = bridge();
    assert_eq!(Munitions::apply(&mut t, 7, e, true), ImpactResult::Destroyed { target: 7, critical: true });
    // Drei Drittel zählen als ganz.
    let mut t = bridge();
    for _ in 0..3 {
        Munitions::apply(&mut t, 7, 0.34 - 0.0067, false);
    }
    assert_eq!(t.state, TargetState::Damaged, "0,99 ist noch nicht zerstört");
    let mut t = bridge();
    for _ in 0..3 {
        Munitions::apply(&mut t, 7, 1.0 / 3.0, false);
    }
    assert_eq!(t.state, TargetState::Destroyed);
}

/// Auslösen auf ein Ziel 5 km voraus, 3 km darunter, Nase genau aufs Ziel.
fn launch_at_target(lock_optical: bool) -> (Weapon, Vec<Target>, Terrain) {
    let terrain = Terrain::new(1);
    let elevation = terrain.elevation(5_000.0, 0.0);
    let mut t = bridge();
    t.class = TargetClass::Soft;
    t.north = 5_000.0;
    t.elevation = elevation;
    let targets = vec![t];
    let alt = elevation + 3_000.0 + TargetClass::Soft.height() * 0.5;
    let pitch = -detmath::atan2(3_000.0, 5_000.0);
    let attitude = DQuat::from_euler(0.0, pitch, 0.0);
    let launch = Launch {
        tick: 1,
        position: DVec3::new(0.0, 0.0, -alt),
        velocity: attitude.rotate(DVec3::new(200.0, 0.0, 0.0)),
        attitude,
        lock: Some(0),
        lock_optical,
        air: None,
        targets: &targets,
        terrain: &terrain,
        bay_ok: true,
        dt: 1.0 / 120.0,
    };
    let w = Weapon::launched(1, WeaponKind::AirToGround, &launch);
    (w, targets, terrain)
}

/// Sicht auf Bodenziele für die Tests (ohne Flugzeug, ohne Täuschkörper).
fn ground_view<'a>(targets: &'a [Target], terrain: &'a Terrain) -> View<'a> {
    View {
        targets,
        terrain,
        designated: None,
        aircraft: None,
        fighters: &[],
        decoys: &[],
        painted: false,
        jammed: (false, false),
        los: true,
    }
}

#[test]
fn optical_seeker_takes_only_an_optical_lock() {
    let (w, _, _) = launch_at_target(true);
    assert_eq!(w.target, Some(Track::Ground(0)));
    assert!(!w.searching && w.tracking);
    let (w, _, _) = launch_at_target(false);
    assert_eq!(w.target, None, "nur vom Radar entdecktes Ziel wird nicht übernommen");
    assert!(w.searching);
}

/// Mit Übergabe fliegt die Rakete das Ziel an und zündet daran.
#[test]
fn missile_pursues_and_fuzes_at_the_target() {
    let (mut w, targets, terrain) = launch_at_target(true);
    let mut fate = Fate::Flying;
    for k in 0..(120 * 30) {
        if k % 12 == 0 {
            seek(&mut w, &ground_view(&targets, &terrain));
        }
        let aim = w.aim_point(&targets).map(|p| (p, DVec3::ZERO));
        fate = advance(&mut w, aim, &terrain, 1.0 / 120.0);
        if fate != Fate::Flying {
            break;
        }
    }
    let Fate::Fuze(p) = fate else { panic!("kein Zünder: {fate:?}") };
    let c = targets[0].center();
    assert!((p - c).length() < WeaponKind::AirToGround.spec().lethal_radius_m);
}

#[test]
fn unlocked_missile_flies_on_in_launch_attitude() {
    let (mut w, _, terrain) = launch_at_target(false);
    // Ohne Ziele in der Welt: nichts zu finden, nur geradeaus.
    let none: Vec<Target> = Vec::new();
    let nose = w.boresight;
    // Lange genug, um auf Marschfahrt zu kommen (gemächliche Beschleunigung).
    for _ in 0..(120 * 12) {
        seek(&mut w, &ground_view(&none, &terrain));
        guide(&mut w, None, 1.0 / 120.0);
    }
    let dir = w.velocity.normalize_or_zero();
    assert!(angle_between(dir, nose) < 0.01, "weicht {:.3} rad ab", angle_between(dir, nose));
    assert!((w.velocity.length() - w.kind.spec().cruise_mps).abs() < 1.0);
}

/// Feindliche Rakete auf das Flugzeug bei `at`, nach dem Start.
fn sam(kind: WeaponKind) -> Weapon {
    let mut w = Weapon::new(0, kind, DVec3::ZERO, DVec3::new(700.0, 0.0, -1.0));
    w.owner = Some(Launcher::Site(0));
    w.target = Some(Track::Aircraft);
    w.age_s = 10.0;
    w
}

fn air_view<'a>(terrain: &'a Terrain, at: DVec3, painted: bool, decoys: &'a [Decoy]) -> View<'a> {
    View {
        targets: &[],
        terrain,
        designated: None,
        aircraft: Some((at, DVec3::ZERO)),
        fighters: &[],
        decoys,
        painted,
        jammed: (false, false),
        los: true,
    }
}

/// Störer wie im Original: Die gestörte Lenkart verliert außerhalb kurzer
/// Entfernung die Lenkung, die andere nicht.
#[test]
fn jammers_break_their_kind_of_guidance_beyond_burn_through() {
    let terrain = Terrain::new(1);
    let (far, near) = (DVec3::new(5_000.0, 0.0, -9_000.0), DVec3::new(1_500.0, 0.0, -9_000.0));
    let check = |kind, at: DVec3, jammed| {
        let mut w = sam(kind);
        w.position = DVec3::new(0.0, 0.0, -9_000.0);
        seek(&mut w, &View { jammed, ..air_view(&terrain, at, true, &[]) });
        w.tracking
    };
    let (radar, ir) = (WeaponKind::MediumRangeSam, WeaponKind::ShortRangeSam);
    assert!(!check(radar, far, (true, false)), "ECM stört die Radarrakete");
    assert!(check(radar, near, (true, false)), "nah brennt sie durch");
    assert!(check(ir, far, (true, false)), "ECM stört kein Infrarot");
    assert!(!check(ir, far, (false, true)), "Infrarotstörer stört Infrarot");
    assert!(check(radar, far, (false, true)));
}

/// Halbaktiv: Weit weg lenkt die Boden-Luft-Rakete nur, solange ihre
/// Stellung das Flugzeug erfasst; nah mit eigenem Suchkopf. Fällt es aus dem
/// Suchkegel, hat sie es verloren (Querflug schüttelt ab).
#[test]
fn sam_guidance_is_semi_active() {
    let terrain = Terrain::new(1);
    let mut w = sam(WeaponKind::MediumRangeSam);
    let far = DVec3::new(10_000.0, 0.0, 0.0);
    seek(&mut w, &air_view(&terrain, far, true, &[]));
    assert!(w.tracking);
    seek(&mut w, &air_view(&terrain, far, false, &[]));
    assert!(!w.tracking && w.target.is_some(), "ohne Beleuchtung blind, aber noch dran");
    seek(&mut w, &air_view(&terrain, DVec3::new(3_000.0, 0.0, 0.0), false, &[]));
    assert!(w.tracking, "nah: eigener Suchkopf");
    seek(&mut w, &air_view(&terrain, DVec3::new(3_000.0, 3_000.0, 0.0), true, &[]));
    assert!(!w.tracking && w.target.is_none(), "45° neben der Flugrichtung: verloren");
    seek(&mut w, &air_view(&terrain, far, true, &[]));
    assert!(!w.tracking, "verloren bleibt verloren");
}

/// Täuschkörper wie im Original: Ein Düppel im Suchkegel blendet die
/// Radarrakete, nicht die Infrarotrakete – bei der Fackel umgekehrt. Die
/// Rakete hält dabei ihr Ziel und sieht es wieder, sobald der Täuschkörper
/// fort ist.
#[test]
fn decoys_lure_only_their_kind_of_seeker() {
    let terrain = Terrain::new(1);
    // Hoch über allem Gelände: freie Sicht
    let at = DVec3::new(8_000.0, 0.0, -9_000.0);
    let place = DVec3::new(4_000.0, 200.0, -9_000.0);
    let start = |kind| {
        let mut w = sam(kind);
        w.position = DVec3::new(0.0, 0.0, -9_000.0);
        w
    };
    // Täuschkörper, dem diese Rakete nicht widersteht (Resistenz ausgeklammert).
    let decoy = |id, kind, w: &Weapon| {
        let d = Decoy { id, kind, owner: None, position: place, age_s: 0.0, salt: 0 };
        (0..).map(|salt| Decoy { salt, ..d }).find(|d| d.lures(w) || d.salt > 1_000).unwrap()
    };
    let chaff = [decoy(7, DecoyKind::Chaff, &start(WeaponKind::MediumRangeSam))];
    let flare = [decoy(8, DecoyKind::Flare, &start(WeaponKind::ShortRangeSam))];
    let mut radar = start(WeaponKind::MediumRangeSam);
    assert!(seek(&mut radar, &air_view(&terrain, at, true, &flare)).is_none());
    assert_eq!(radar.target, Some(Track::Aircraft));
    assert!(matches!(
        seek(&mut radar, &air_view(&terrain, at, true, &chaff)),
        Some(WeaponEvent::Decoyed { .. })
    ));
    assert!(radar.blinded && !radar.tracking && radar.target == Some(Track::Aircraft));
    assert!(seek(&mut radar, &air_view(&terrain, at, true, &chaff)).is_none(), "nur einmal gemeldet");
    let mut ir = start(WeaponKind::ShortRangeSam);
    seek(&mut ir, &air_view(&terrain, DVec3::new(5_000.0, 0.0, -9_000.0), false, &chaff));
    assert_eq!(ir.target, Some(Track::Aircraft));
    assert!(ir.tracking, "Infrarot braucht keine Beleuchtung");
    seek(&mut ir, &air_view(&terrain, at, false, &flare));
    assert!(ir.blinded && !ir.tracking);
    // Ausgebrannt, Flugzeug noch im Kegel: Sie sieht es wieder.
    seek(&mut ir, &air_view(&terrain, at, false, &[]));
    assert!(!ir.blinded && ir.tracking && ir.target == Some(Track::Aircraft));
}

/// Vorhalt: stehendes Ziel → Zielmitte; bewegtes Ziel → Treffpunkt
/// voraus, voll bei Faktor 1.
#[test]
fn intercept_leads_moving_targets_only() {
    let w = Weapon::new(0, WeaponKind::ShortRangeSam, DVec3::ZERO, DVec3::new(700.0, 0.0, 0.0));
    let aim = DVec3::new(7_000.0, 0.0, 0.0);
    assert_eq!(intercept(&w, aim, DVec3::ZERO), aim);
    // Ziel quer mit 200 m/s, Annäherung 700 m/s ⇒ 10 s ⇒ 2 km voraus
    let p = intercept(&w, aim, DVec3::new(0.0, 200.0, 0.0));
    assert!((p - DVec3::new(7_000.0, 2_000.0, 0.0)).length() < 1e-6, "{p:?}");
    // Bogen: Die SA-5 zielt ein Viertel der Entfernung über das Ziel.
    let w = Weapon { kind: WeaponKind::LongRangeSam, ..w };
    assert_eq!(intercept(&w, aim, DVec3::ZERO), aim + DVec3::new(0.0, 0.0, -1_750.0));
}

/// Wie im Original: Ein Ziel weit hinter der Rakete gibt sie auf.
#[test]
fn missile_gives_up_a_target_behind_it() {
    let (mut w, mut targets, terrain) = launch_at_target(true);
    w.age_s = 1.0;
    targets[0].north = -20_000.0;
    seek(&mut w, &ground_view(&targets, &terrain));
    assert_eq!(w.target, None);
    assert!(!w.tracking);
}

#[test]
fn turning_is_rate_limited() {
    let a = DVec3::new(1.0, 0.0, 0.0);
    let b = DVec3::new(0.0, 1.0, 0.0);
    let c = turn_towards(a, b, 0.1);
    assert!((angle_between(a, c) - 0.1).abs() < 1e-9);
    assert_eq!(turn_towards(a, b, 2.0), b);
}

#[test]
fn retarded_bomb_falls_slower_than_free_fall() {
    let start = |kind| Weapon::new(0, kind, DVec3::new(0.0, 0.0, -3_000.0), DVec3::new(250.0, 0.0, 0.0));
    let (mut a, mut b) = (start(WeaponKind::FreeFall), start(WeaponKind::Retarded));
    for _ in 0..(120 * 5) {
        fly(&mut a, 1.0 / 120.0);
        fly(&mut b, 1.0 / 120.0);
    }
    // Nach 5 s: Freifall fast ungebremst vorwärts, Fallschirm weit zurück.
    assert!(a.position.x > 1_150.0, "Freifall {:.0} m", a.position.x);
    // Das Flugzeug (250 m/s) ist nach 5 s bei 1250 m: Die Fallschirmbombe
    // liegt außerhalb des Splitterbereichs zurück, die Freifallbombe nicht.
    let spec = WeaponKind::Retarded.spec();
    assert!(1_250.0 - b.position.x > spec.frag_radius_m, "Fallschirm {:.0} m", b.position.x);
    assert!(1_250.0 - a.position.x < spec.frag_radius_m, "Freifall {:.0} m", a.position.x);
    assert!(b.velocity.length() < a.velocity.length() * 0.6);
    // Freifall ohne Widerstand wäre 0,5·g·t² ≈ 123 m gefallen.
    let fell = a.position.z + 3_000.0;
    assert!(fell > 110.0 && fell < 125.0, "gefallen {fell:.1} m");
}

/// Resistenz: Je Paar aus Rakete und Täuschkörper fest gewürfelt, im Mittel
/// so oft übergangen wie eingestellt; eigene Täuschkörper lenken eigene
/// Waffen nie ab.
#[test]
fn decoy_resistance_matches_the_spec() {
    for kind in [WeaponKind::ShortRangeSam, WeaponKind::MediumRangeSam, WeaponKind::ModernSam] {
        let mut w = sam(kind);
        let decoy = DecoyKind::Chaff;
        let decoy = if w.kind.spec().seeker == Seeker::Optical { DecoyKind::Flare } else { decoy };
        let mut resisted = 0;
        for id in 0..2_000 {
            w.id = id;
            let d = Decoy {
                id: 9_999,
                kind: decoy,
                owner: None,
                position: DVec3::ZERO,
                age_s: 0.0,
                salt: id * 7,
            };
            resisted += usize::from(!d.lures(&w));
        }
        let rate = resisted as f64 / 2_000.0;
        let want = kind.spec().decoy_resistance;
        assert!((rate - want).abs() < 0.03, "{kind:?}: {rate:.3} statt {want}");
    }
    let own = Weapon::new(1, WeaponKind::AirToGround, DVec3::ZERO, DVec3::new(300.0, 0.0, 0.0));
    let flare =
        Decoy { id: 2, kind: DecoyKind::Flare, owner: None, position: DVec3::ZERO, age_s: 0.0, salt: 0 };
    assert!(
        !(0..100).any(|salt| Decoy { salt, ..flare }.lures(&own)),
        "eigene Fackel lenkt eigene Maverick ab"
    );
}
