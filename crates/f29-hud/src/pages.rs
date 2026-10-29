//! Seiten des rechten MFDs: Mission, Ziel, Waffen, Triebwerk mit Treibstoff,
//! künstlicher Horizont (Reserveinstrument) und Systeme.
//!
//! Alle Seiten zeichnen nur in ihre Bildfläche `screen` (das Clipping setzt
//! der Aufrufer) und lassen unten Platz für die Seitenleiste
//! ([`page_bar`]).

use crate::canvas::{Align, DisplayList, Ink, Rect};
use crate::heading_deg;
use crate::map::MapData;
use crate::mfd::{RightPage, WEAPON_STATIONS, fuel_bar};
use f29_core::targets::TargetRole;
use f29_core::units;
use f29_core::weapons::{Seeker, WeaponKind};
use f29_core::{DVec3, FlightData, TargetInfo};

/// Anteil der Bildhöhe, den die Seitenleiste unten belegt.
pub(crate) const PAGE_BAR: f32 = 0.075;

/// Maße einer Seite: Breite, Höhe, Schriftgröße, linker/rechter Rand.
struct Frame {
    w: f32,
    h: f32,
    txt: f32,
    x0: f32,
    x1: f32,
}

fn frame(screen: Rect) -> Frame {
    let w = screen.max[0] - screen.min[0];
    let h = screen.max[1] - screen.min[1];
    Frame { w, h, txt: w * 0.042, x0: screen.min[0] + w * 0.05, x1: screen.max[0] - w * 0.05 }
}

fn title(dl: &mut DisplayList, screen: Rect, f: &Frame, text: &str) {
    dl.text_mid([f.x0, screen.max[1] - f.txt * 1.2], f.txt, Align::Left, text, Ink::Mfd);
}

/// Beschriftete Zeile „NAME ……… WERT“.
fn row(dl: &mut DisplayList, f: &Frame, y: f32, name: &str, value: &str, ink: Ink) {
    dl.text_mid([f.x0, y], f.txt * 0.85, Align::Left, name, Ink::MfdDim);
    dl.text_mid([f.x1, y], f.txt * 0.85, Align::Right, value, ink);
}

/// Seitenleiste am unteren Rand: alle Seiten, die aktive gerahmt.
pub(crate) fn page_bar(dl: &mut DisplayList, screen: Rect, active: RightPage) {
    let w = screen.max[0] - screen.min[0];
    let h = screen.max[1] - screen.min[1];
    let y = screen.min[1] + h * PAGE_BAR * 0.5;
    let txt = w * 0.03;
    let pages = RightPage::ALL;
    for (k, p) in pages.iter().enumerate() {
        let x = screen.min[0] + w * (k as f32 + 0.5) / pages.len() as f32;
        if *p == active {
            dl.boxed_text([x, y], txt, Align::Center, p.label(), Ink::Mfd);
        } else {
            dl.text_mid([x, y], txt, Align::Center, p.label(), Ink::MfdDim);
        }
    }
}

/// Waffen: vier Waffenschächte (Platzhalter) als liegende Rechtecke, darunter
/// Angaben zum gewählten Schacht. Waffen kommen mit v0.2.
pub(crate) fn weapons(dl: &mut DisplayList, fd: &FlightData, screen: Rect) {
    let f = frame(screen);
    let st = &fd.stores;
    let selected = st.selected;
    title(dl, screen, &f, "WEAPONS");
    let (state, state_ink) = if st.door >= 1.0 {
        ("ARMED", Ink::Warning)
    } else if st.door > 0.0 {
        ("DOORS", Ink::Mfd)
    } else {
        ("SAFE", Ink::MfdDim)
    };
    dl.text_mid([f.x1, screen.max[1] - f.txt * 1.2], f.txt, Align::Right, state, state_ink);

    // 2 × 2 Schächte, je etwa doppelt so breit wie hoch
    let top = screen.max[1] - f.h * 0.13;
    let gap = f.w * 0.04;
    let bay_w = (f.x1 - f.x0 - gap) * 0.5;
    let bay_h = bay_w * 0.42;
    for (k, bay) in st.bays.iter().enumerate().take(WEAPON_STATIONS) {
        let (col, rowi) = (k % 2, k / 2);
        let x = f.x0 + col as f32 * (bay_w + gap);
        let y = top - rowi as f32 * (bay_h + gap) - bay_h;
        let r = Rect { min: [x, y], max: [x + bay_w, y + bay_h] };
        let sel = k == selected;
        let ink = if sel { Ink::Mfd } else { Ink::MfdDim };
        dl.rect(r, ink);
        if sel {
            let d = f.w * 0.012;
            dl.rect(Rect { min: [r.min[0] - d, r.min[1] - d], max: [r.max[0] + d, r.max[1] + d] }, Ink::Mfd);
        }
        let c = r.center();
        dl.text_mid(
            [r.min[0] + f.txt * 0.5, r.max[1] - f.txt * 0.9],
            f.txt * 0.8,
            Align::Left,
            &format!("BAY {}", k + 1),
            ink,
        );
        match bay.kind {
            Some(kind) if bay.count > 0 => {
                dl.text_mid([c[0], c[1] - f.txt * 0.4], f.txt * 0.9, Align::Center, kind.label(), ink);
                // Ein Strich je Waffe am unteren Rand
                let n = kind.spec().per_bay.max(1);
                let pip = (r.max[0] - r.min[0]) * 0.6 / f32::from(n);
                for p in 0..n {
                    let px = c[0] + (f32::from(p) - f32::from(n - 1) * 0.5) * pip;
                    let py = r.min[1] + f.txt * 0.45;
                    let pink = if p < bay.count { ink } else { Ink::MfdGrid };
                    dl.line([px - pip * 0.35, py], [px + pip * 0.35, py], pink);
                }
            }
            Some(kind) => {
                dl.text_mid(
                    [c[0], c[1] - f.txt * 0.4],
                    f.txt * 0.9,
                    Align::Center,
                    kind.label(),
                    Ink::MfdGrid,
                );
            }
            None => {
                dl.text_mid([c[0], c[1] - f.txt * 0.4], f.txt * 0.9, Align::Center, "EMPTY", Ink::MfdDim);
            }
        }
    }

    // Angaben zum gewählten Schacht
    let info_top = top - 2.0 * bay_h - gap - f.h * 0.05;
    let step = f.txt * 1.35;
    dl.text_mid([f.x0, info_top], f.txt, Align::Left, &format!("SELECTED BAY {}", selected + 1), Ink::Mfd);
    let bay = st.bays[selected];
    let release = if bay.count == 0 {
        ("EMPTY", Ink::MfdDim)
    } else if st.door >= 1.0 {
        ("READY", Ink::Warning)
    } else {
        ("BAY CLOSED", Ink::Mfd)
    };
    let (store, qty, weight, guidance) = match bay.kind {
        Some(k) => {
            let s = k.spec();
            let guidance = match s.seeker {
                Seeker::None if k == WeaponKind::Retarded => "RETARDED",
                Seeker::None => "NONE",
                Seeker::Optical => "TV",
                Seeker::Radiation => "ANTI-RADAR",
                Seeker::Naval => "RADAR",
            };
            let lb = units::kg_to_lb(s.mass_kg * f64::from(bay.count));
            (s.name.to_string(), bay.count.to_string(), format!("{lb:.0} LB"), guidance)
        }
        None => ("---".to_string(), "0".to_string(), "0 LB".to_string(), "---"),
    };
    let rows: [(&str, &str, Ink); 5] = [
        ("STORE", &store, Ink::Mfd),
        ("QTY", &qty, Ink::Mfd),
        ("WEIGHT", &weight, Ink::Mfd),
        ("GUIDANCE", guidance, Ink::Mfd),
        ("RELEASE", release.0, release.1),
    ];
    for (i, (name, value, ink)) in rows.iter().enumerate() {
        row(dl, &f, info_top - step * (i as f32 + 1.1), name, value, *ink);
    }
}

/// Triebwerk: Drehzahl als Rundinstrument, Schub, Verbrauch, Restflugzeit.
pub(crate) fn engine(dl: &mut DisplayList, fd: &FlightData, screen: Rect) {
    let f = frame(screen);
    title(dl, screen, &f, "ENGINE");
    let flameout = fd.warnings.flameout;
    if flameout {
        dl.text_mid([f.x1, screen.max[1] - f.txt * 1.2], f.txt, Align::Right, "FLAMEOUT", Ink::Warning);
    }

    // Drehzahl: Leerlauf 62 %, Vollgas 100 %
    let rpm = if flameout { 100.0 * fd.engine } else { 62.0 + 38.0 * fd.engine };
    let c = [screen.min[0] + f.w * 0.5, screen.max[1] - f.h * 0.3];
    let r = f.w * 0.18;
    let (a0, a1) = (210.0_f32.to_radians(), -30.0_f32.to_radians());
    let ang = |pct: f64| a0 + (a1 - a0) * (pct.clamp(0.0, 110.0) / 110.0) as f32;
    dl.arc(c, r, a1, a0, 32, Ink::Mfd);
    for pct in (0..=110).step_by(10) {
        let a = ang(f64::from(pct));
        let (s, co) = a.sin_cos();
        let inner = if pct % 50 == 0 { 0.8 } else { 0.88 };
        dl.line([c[0] + co * r * inner, c[1] + s * r * inner], [c[0] + co * r, c[1] + s * r], Ink::Mfd);
    }
    // Bereich über 100 % rot markieren
    dl.arc(c, r * 1.04, ang(110.0), ang(100.0), 4, Ink::Warning);
    let a = ang(rpm);
    let (s, co) = a.sin_cos();
    dl.line(c, [c[0] + co * r * 0.92, c[1] + s * r * 0.92], Ink::Needle);
    // Schubhebel-Marke außen
    let ta = ang(62.0 + 38.0 * fd.throttle);
    let (ts, tc) = ta.sin_cos();
    dl.line(
        [c[0] + tc * r * 1.04, c[1] + ts * r * 1.04],
        [c[0] + tc * r * 1.16, c[1] + ts * r * 1.16],
        Ink::Hud,
    );
    let ink = if flameout { Ink::Warning } else { Ink::Mfd };
    dl.text_mid([c[0], c[1] - r * 0.35], f.txt * 1.3, Align::Center, &format!("{rpm:.0}%"), ink);
    dl.text_mid([c[0], c[1] - r * 0.75], f.txt * 0.75, Align::Center, "RPM", Ink::MfdDim);

    // Werte
    let y0 = c[1] - r * 1.3;
    let step = f.txt * 1.45;
    let lb_min = units::kg_to_lb(fd.fuel_flow_kgps) * 60.0;
    let rows = [
        ("THROTTLE", format!("{:.0}%", fd.throttle * 100.0)),
        ("THRUST", format!("{:.0} KN", fd.thrust_n.max(0.0) / 1000.0)),
        ("FUEL FLOW", format!("{lb_min:.0} LB/MIN")),
    ];
    for (i, (name, value)) in rows.iter().enumerate() {
        row(dl, &f, y0 - step * i as f32, name, value, ink);
    }

    // Treibstoff
    let bar = Rect { min: [f.x0, screen.min[1] + f.h * 0.13], max: [f.x1, screen.min[1] + f.h * 0.19] };
    fuel_bar(dl, fd, bar, f.txt);
}

/// Mission: Ziele (Platzhalter bis v0.3), Route mit aktivem Punkt, Entfernung
/// und Flugzeit zum aktiven Punkt, Heimweg und BINGO.
pub(crate) fn mission(dl: &mut DisplayList, fd: &FlightData, screen: Rect, map: Option<&MapData>) {
    let f = frame(screen);
    title(dl, screen, &f, "MISSION");
    dl.text_mid([f.x1, screen.max[1] - f.txt * 1.2], f.txt, Align::Right, "STRIKE", Ink::MfdDim);
    let step = f.txt * 1.5;
    let mut y = screen.max[1] - f.h * 0.15;
    // Primär- und Sekundärziel mit Entfernung
    for (role, label) in [(TargetRole::Primary, "PRI"), (TargetRole::Secondary, "SEC")] {
        let t = map.and_then(|m| m.targets.iter().find(|t| t.role == role));
        let (name, dist) = match t {
            Some(t) => {
                let (dn, de) = (t.pos[0] - fd.position.x, t.pos[1] - fd.position.y);
                (t.name, format!("{:.1} NM", (dn * dn + de * de).sqrt() / units::M_PER_NM))
            }
            None => ("---", String::new()),
        };
        dl.text_mid([f.x0, y], f.txt * 0.85, Align::Left, &format!("{label} {name}"), Ink::Warning);
        dl.text_mid([f.x1, y], f.txt * 0.85, Align::Right, &dist, Ink::Mfd);
        y -= step;
    }
    y += step * 0.1;
    dl.line([f.x0, y], [f.x1, y], Ink::MfdDim);
    y -= step * 0.8;

    // Route: Punkt, Teilstrecke in NM; aktiver Punkt hell mit Raute
    let active = fd.steerpoint.as_ref().map(|s| s.index);
    if let Some(map) = map {
        let mut prev: Option<[f64; 2]> = None;
        for (k, p) in map.route.iter().enumerate() {
            let leg = prev.map(|q| ((p.pos[0] - q[0]).powi(2) + (p.pos[1] - q[1]).powi(2)).sqrt());
            prev = Some(p.pos);
            let is_active = Some(k) == active;
            let ink = if is_active { Ink::Mfd } else { Ink::MfdDim };
            if is_active {
                let d = f.txt * 0.35;
                let m = [f.x0 + d, y];
                dl.polyline(
                    &[[m[0], m[1] + d], [m[0] + d, m[1]], [m[0], m[1] - d], [m[0] - d, m[1]]],
                    true,
                    Ink::Needle,
                );
            }
            let name = format!("{} {}", k + 1, p.name);
            dl.text_mid([f.x0 + f.txt * 1.2, y], f.txt * 0.85, Align::Left, &name, ink);
            let leg = leg.map_or("---".to_string(), |m| format!("{:.1} NM", m / units::M_PER_NM));
            dl.text_mid([f.x1, y], f.txt * 0.85, Align::Right, &leg, ink);
            y -= step;
        }
    }
    dl.line([f.x0, y + step * 0.45], [f.x1, y + step * 0.45], Ink::MfdDim);
    y -= step * 0.35;

    // Zum aktiven Punkt
    if let Some(sp) = fd.steerpoint.as_ref() {
        let nm = sp.range / units::M_PER_NM;
        let ete = if fd.tas_mps > 1.0 { (sp.range / fd.tas_mps) as i64 } else { 0 };
        row(dl, &f, y, &format!("TO {}", sp.name), &format!("{nm:.1} NM"), Ink::Mfd);
        y -= step;
        let brg = format!("BRG {:03}  ETE {}:{:02}", heading_deg(sp.bearing), ete / 60, ete % 60);
        dl.text_mid([f.x1, y], f.txt * 0.85, Align::Right, &brg, Ink::Mfd);
        y -= step;
    }
    // Heimweg
    let home_nm = fd.home_range_m / units::M_PER_NM;
    let bingo_lb = (units::kg_to_lb(fd.bingo_fuel_kg) / 10.0).round() as i64 * 10;
    let bingo_ink = if fd.warnings.bingo { Ink::Warning } else { Ink::Mfd };
    row(dl, &f, y, "HOME", &format!("{home_nm:.1} NM"), Ink::Mfd);
    y -= step;
    row(dl, &f, y, "BINGO FUEL", &format!("{bingo_lb} LB"), bingo_ink);
}

/// Künstlicher Horizont als Reserveinstrument mit Fahrt, Höhe, Steigrate,
/// Kurs und g-Last.
pub(crate) fn attitude(dl: &mut DisplayList, fd: &FlightData, screen: Rect) {
    let f = frame(screen);
    title(dl, screen, &f, "STBY ADI");
    let c = [screen.min[0] + f.w * 0.5, screen.min[1] + f.h * 0.55];
    let r = f.w * 0.3;
    dl.circle(c, r, 48, Ink::Mfd);

    // Horizont und Nicklinien: drehen mit der Querlage, wandern mit der Nicklage
    let roll = fd.roll as f32;
    let pitch = fd.pitch as f32;
    let k = r / 40.0_f32.to_radians(); // 40° vom Mittelpunkt bis zum Rand
    let (sr, cr) = roll.sin_cos();
    // Bei Rechtsquerlage dreht sich der Horizont gegen den Uhrzeigersinn.
    let u = [cr, sr];
    let n = [-sr, cr];
    let chord = |d: f32, half: f32, ink: Ink, dl: &mut DisplayList| {
        if d.abs() >= r {
            return;
        }
        let max_half = (r * r - d * d).sqrt();
        let hl = half.min(max_half);
        let m = [c[0] + n[0] * d, c[1] + n[1] * d];
        dl.line([m[0] - u[0] * hl, m[1] - u[1] * hl], [m[0] + u[0] * hl, m[1] + u[1] * hl], ink);
    };
    chord(-pitch * k, r, Ink::Mfd, dl);
    for deg in [-30, -20, -10, 10, 20, 30] {
        let d = (deg as f32).to_radians() * k - pitch * k;
        let half = if deg % 20 == 0 { r * 0.3 } else { r * 0.18 };
        chord(d, half, if deg > 0 { Ink::Mfd } else { Ink::MfdDim }, dl);
    }
    // Boden schraffiert (kurze Striche unter dem Horizont)
    for i in 1..=4 {
        let d = -pitch * k - r * 0.18 * i as f32;
        let half = r * (0.75 - 0.12 * i as f32);
        if d.abs() < r {
            let m = [c[0] + n[0] * d, c[1] + n[1] * d];
            let hl = half.min((r * r - d * d).sqrt());
            dl.dashed(
                [m[0] - u[0] * hl, m[1] - u[1] * hl],
                [m[0] + u[0] * hl, m[1] + u[1] * hl],
                5,
                Ink::MfdDim,
            );
        }
    }
    // Feste Flugzeugmarke
    let s = r * 0.32;
    dl.polyline(
        &[
            [c[0] - s, c[1]],
            [c[0] - s * 0.35, c[1]],
            [c[0], c[1] - s * 0.3],
            [c[0] + s * 0.35, c[1]],
            [c[0] + s, c[1]],
        ],
        false,
        Ink::Hud,
    );
    // Querlagenskala oben mit Zeiger
    for deg in [-60, -30, -20, -10, 0, 10, 20, 30, 60] {
        let a = std::f32::consts::FRAC_PI_2 + (deg as f32).to_radians();
        let (s, co) = a.sin_cos();
        let len = if deg % 30 == 0 { 0.1 } else { 0.06 };
        dl.line(
            [c[0] + co * r, c[1] + s * r],
            [c[0] + co * r * (1.0 + len), c[1] + s * r * (1.0 + len)],
            Ink::Mfd,
        );
    }
    let a = std::f32::consts::FRAC_PI_2 + roll;
    let (s, co) = a.sin_cos();
    let tip = [c[0] + co * r * 0.97, c[1] + s * r * 0.97];
    let base = r * 0.88;
    let side = 0.05;
    dl.polyline(
        &[
            tip,
            [c[0] + (a + side).cos() * base, c[1] + (a + side).sin() * base],
            [c[0] + (a - side).cos() * base, c[1] + (a - side).sin() * base],
        ],
        true,
        Ink::Needle,
    );

    // Fahrt links, Höhe rechts, Steigrate und g unten, Kurs oben
    let kt = units::mps_to_kt(fd.tas_mps).round() as i64;
    let ft = (units::m_to_ft(fd.altitude_m) / 10.0).round() as i64 * 10;
    let side_y = c[1];
    dl.boxed_text([screen.min[0] + f.w * 0.04, side_y], f.txt, Align::Left, &format!("{kt}"), Ink::Mfd);
    dl.boxed_text([screen.max[0] - f.w * 0.04, side_y], f.txt, Align::Right, &format!("{ft}"), Ink::Mfd);
    dl.text_mid(
        [screen.min[0] + f.w * 0.04, side_y - f.txt * 1.8],
        f.txt * 0.7,
        Align::Left,
        "KT",
        Ink::MfdDim,
    );
    dl.text_mid(
        [screen.max[0] - f.w * 0.04, side_y - f.txt * 1.8],
        f.txt * 0.7,
        Align::Right,
        "FT",
        Ink::MfdDim,
    );
    let hdg = format!("HDG {:03}", heading_deg(fd.heading));
    dl.text_mid([f.x1, screen.max[1] - f.txt * 1.2], f.txt, Align::Right, &hdg, Ink::Mfd);
    let fpm = (units::mps_to_fpm(fd.vertical_speed_mps) / 100.0).round() as i64 * 100;
    let y = c[1] - r * 1.25;
    dl.text_mid([f.x0, y], f.txt * 0.85, Align::Left, &format!("VS {fpm:+}"), Ink::Mfd);
    let g_ink = if fd.warnings.over_g { Ink::Warning } else { Ink::Mfd };
    dl.text_mid([f.x1, y], f.txt * 0.85, Align::Right, &format!("G {:.1}", fd.load_factor), g_ink);
}

/// Systeme: Konfiguration, Masse, Abrissfahrt und Warnungen.
pub(crate) fn systems(dl: &mut DisplayList, fd: &FlightData, screen: Rect) {
    let f = frame(screen);
    title(dl, screen, &f, "SYSTEMS");
    let step = f.txt * 1.55;
    let mut y = screen.max[1] - f.h * 0.15;
    let gear = if fd.gear >= f29_core::flight::GEAR_LOCKED {
        "DOWN"
    } else if fd.gear <= 0.0 {
        "UP"
    } else {
        "TRANSIT"
    };
    let gear_ink = if fd.warnings.gear_overspeed || fd.warnings.gear_up { Ink::Warning } else { Ink::Mfd };
    let rows = [
        ("GEAR", gear.to_string(), gear_ink),
        ("FLAPS", format!("{:.0}%", fd.flaps * 100.0), Ink::Mfd),
        ("AIRBRAKE", format!("{:.0}%", fd.airbrake * 100.0), Ink::Mfd),
        ("GROSS WT", format!("{} LB", (units::kg_to_lb(fd.mass_kg) / 100.0).round() as i64 * 100), Ink::Mfd),
        ("STALL SPD", format!("{} KT", units::mps_to_kt(fd.stall_speed_mps).round() as i64), Ink::Mfd),
        ("MACH", format!("{:.2}", fd.mach), Ink::Mfd),
    ];
    for (name, value, ink) in &rows {
        row(dl, &f, y, name, value, *ink);
        y -= step;
    }

    // Warnfeld: alle Warnungen, aktive hell
    y -= step * 0.4;
    dl.line([f.x0, y + step * 0.5], [f.x1, y + step * 0.5], Ink::MfdDim);
    let w = &fd.warnings;
    let cautions = [
        ("STALL", w.stall),
        ("PULL UP", w.pull_up),
        ("OVER G", w.over_g),
        ("FUEL LOW", w.fuel_low),
        ("BINGO", w.bingo),
        ("FLAMEOUT", w.flameout),
        ("GEAR SPD", w.gear_overspeed),
        ("GEAR UP", w.gear_up),
    ];
    let col_w = (f.x1 - f.x0) * 0.5;
    for (i, (name, on)) in cautions.iter().enumerate() {
        let x = f.x0 + col_w * (i % 2) as f32 + col_w * 0.5;
        let yy = y - step * 0.6 - step * (i / 2) as f32;
        if *on {
            dl.boxed_text([x, yy], f.txt * 0.8, Align::Center, name, Ink::Warning);
        } else {
            dl.text_mid([x, yy], f.txt * 0.8, Align::Center, name, Ink::MfdDim);
        }
    }
}

/// Ziel: optisches Zielbild (Drahtgitter) des aufgeschalteten Ziels, sonst
/// Radarmarke oder „NO TARGET“; darunter Name, Klasse, Entfernung, Peilung.
/// Bildfeld der TGT-Seite im rechten MFD-Bildschirm `screen`.
pub fn target_image_rect(screen: Rect) -> Rect {
    let f = frame(screen);
    Rect { min: [f.x0, screen.max[1] - f.h * 0.53], max: [f.x1, screen.max[1] - f.h * 0.11] }
}

/// TGT-Seite. Mit `camera_feed` liegt ein gerendertes Kamerabild unter dem
/// Bildfeld; gezeichnet werden dann nur Rahmen und Fadenkreuz.
pub(crate) fn target(dl: &mut DisplayList, fd: &FlightData, screen: Rect, camera_feed: bool) {
    let f = frame(screen);
    title(dl, screen, &f, "TARGET");
    let status = match &fd.target {
        Some(t) if t.optical && t.radar => "OPT RDR",
        Some(t) if t.optical => "OPT",
        Some(_) => "RDR",
        None => "NO LOCK",
    };
    dl.text_mid([f.x1, screen.max[1] - f.txt * 1.2], f.txt, Align::Right, status, Ink::Mfd);

    // Bildfeld
    let img = target_image_rect(screen);
    dl.rect(img, Ink::MfdDim);
    let c = img.center();
    let half = (img.max[1] - img.min[1]) * 0.5;
    let tick = half * 0.12;
    for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
        let a = [c[0] + dx * half * 0.85, c[1] + dy * half * 0.85];
        dl.line(a, [a[0] - dx * tick, a[1] - dy * tick], Ink::MfdDim);
    }
    match &fd.target {
        // Bild kommt aus dem Frontend (gerenderte Zielkamera).
        Some(t) if t.optical && camera_feed => {}
        Some(t) if t.optical => {
            dl.set_clip(Some(img));
            target_image(dl, t, c, half);
            dl.set_clip(Some(screen));
        }
        Some(_) => {
            dl.rect(Rect::from_center(c, [half * 0.05, half * 0.05]), Ink::Mfd);
            dl.text_mid([c[0], c[1] - half * 0.3], f.txt * 0.85, Align::Center, "NO VISUAL", Ink::MfdDim);
        }
        None => {
            dl.text_mid(c, f.txt, Align::Center, "NO TARGET", Ink::MfdDim);
        }
    }

    // Angaben
    let step = f.txt * 1.35;
    let mut y = img.min[1] - step * 0.9;
    if let Some(t) = &fd.target {
        let name_ink = if t.civilian { Ink::Warning } else { Ink::Mfd };
        dl.text_mid([f.x0, y], f.txt, Align::Left, t.name, name_ink);
        let role = match t.role {
            TargetRole::Primary => "PRIMARY",
            TargetRole::Secondary => "SECONDARY",
            TargetRole::None => "",
        };
        dl.text_mid([f.x1, y], f.txt * 0.85, Align::Right, role, Ink::Warning);
        y -= step;
        if t.civilian {
            dl.text_mid([f.x0, y], f.txt * 0.85, Align::Left, "CIVILIAN - DO NOT ATTACK", Ink::Warning);
        } else {
            row(dl, &f, y, "CLASS", t.class.label(), Ink::Mfd);
        }
        y -= step;
        row(dl, &f, y, "RANGE", &format!("{:.1} NM", units::m_to_nm(t.range)), Ink::Mfd);
        y -= step;
        row(dl, &f, y, "BEARING", &format!("{:03}", heading_deg(t.bearing)), Ink::Mfd);
        y -= step;
    } else {
        y -= step * 4.0;
    }
    let radar = if fd.radar_on { "ON" } else { "OFF" };
    let ink = if fd.radar_on { Ink::Warning } else { Ink::MfdDim };
    row(dl, &f, y, &format!("LOOK {}", fd.look.label()), &format!("RADAR {radar}"), ink);
}

/// Drahtgitterbild des Ziels aus Sicht des Flugzeugs, so gezoomt, dass das
/// Ziel gut die Hälfte des Bildfelds füllt; dazu ein Bodengitter.
fn target_image(dl: &mut DisplayList, t: &TargetInfo, c: [f32; 2], half: f32) {
    let foot = t.offset_ned;
    let height = t.class.height();
    let center = foot + DVec3::new(0.0, 0.0, -height * 0.5);
    let range = center.length().max(1.0);
    let fwd = center / range;
    let right = fwd.cross(DVec3::new(0.0, 0.0, -1.0)).normalize_or_zero();
    let right = if right == DVec3::ZERO { DVec3::Y } else { right };
    let up = right.cross(fwd);
    let radius = t.class.radius().max(height);
    let zoom = f64::from(half) * 0.8 * range / radius;
    let (sh, ch) = (t.heading.sin(), t.heading.cos());
    // Zielachsen (vorn, rechts, oben) → NED relativ zum Flugzeug
    let to_ned = |p: [f64; 3]| foot + DVec3::new(p[0] * ch - p[1] * sh, p[0] * sh + p[1] * ch, -p[2]);
    let project = |v: DVec3| -> Option<[f32; 2]> {
        let z = v.dot(fwd);
        (z > 1.0).then(|| [c[0] + (v.dot(right) / z * zoom) as f32, c[1] + (v.dot(up) / z * zoom) as f32])
    };
    let seg = |dl: &mut DisplayList, a: [f64; 3], b: [f64; 3], ink: Ink| {
        if let (Some(pa), Some(pb)) = (project(to_ned(a)), project(to_ned(b))) {
            dl.line(pa, pb, ink);
        }
    };
    // Bodengitter
    let g = radius * 1.5;
    for k in -2..=2 {
        let o = g * f64::from(k) / 2.0;
        seg(dl, [o, -g, 0.0], [o, g, 0.0], Ink::MfdGrid);
        seg(dl, [-g, o, 0.0], [g, o, 0.0], Ink::MfdGrid);
    }
    // Quader
    for b in t.class.shape() {
        let [cx, cy, cz] = b.center.map(f64::from);
        let [hx, hy, hz] = b.half.map(f64::from);
        let corner = |i: usize| {
            [
                cx + if i & 1 != 0 { hx } else { -hx },
                cy + if i & 2 != 0 { hy } else { -hy },
                cz + if i & 4 != 0 { hz } else { -hz },
            ]
        };
        for i in 0..8 {
            for bit in [1, 2, 4] {
                if i & bit == 0 {
                    seg(dl, corner(i), corner(i | bit), Ink::Mfd);
                }
            }
        }
    }
}
