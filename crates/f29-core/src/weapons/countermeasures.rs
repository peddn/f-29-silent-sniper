//! Selbstschutz: Täuschkörper und Störer.

use super::*;

/// Art eines Täuschkörpers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecoyKind {
    /// Düppel gegen Radarlenkung.
    Chaff,
    /// Fackel gegen Infrarot- bzw. optische Lenkung.
    Flare,
}

impl DecoyKind {
    pub fn label(self) -> &'static str {
        match self {
            DecoyKind::Chaff => "CHAFF",
            DecoyKind::Flare => "FLARES",
        }
    }
}

/// Ein Täuschkörper: steht am Abwurfort, bis er ausgebrannt ist.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Decoy {
    pub id: u32,
    pub kind: DecoyKind,
    /// Wer ihn geworfen hat: `None` das Flugzeug, sonst die Gegenseite.
    pub owner: Option<Launcher>,
    /// NED (m).
    pub position: DVec3,
    pub age_s: f64,
    /// Zufallswert beim Abwurf: entscheidet mit der Raketennummer, ob eine
    /// Rakete ihn übergeht ([`WeaponSpec::decoy_resistance`]).
    pub salt: u32,
}

impl Decoy {
    /// Lenkt dieser Täuschkörper die Rakete `w` ab? Nur Raketen gegen die
    /// Seite, die ihn geworfen hat, mit passender Lenkart, und nur, wenn
    /// sie ihm nicht widersteht – je Paar fest gewürfelt.
    pub fn lures(&self, w: &Weapon) -> bool {
        let s = w.kind.spec();
        let against_owner = self.owner.is_none() == w.owner.is_some();
        against_owner && s.seeker.lured_by(self.kind) && unit(self.salt, w.id) >= s.decoy_resistance
    }
}

/// Gleichverteilter Wert in [0, 1) aus zwei Zahlen (SplitMix64-Mischung).
fn unit(a: u32, b: u32) -> f64 {
    let mut x = (u64::from(a) << 32 | u64::from(b)).wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    (x >> 11) as f64 / (1_u64 << 53) as f64
}

/// Selbstschutz: Täuschkörper und Störer.
#[derive(Debug, Clone, PartialEq)]
pub struct Countermeasures {
    /// Vorrat an Düppeln und Fackeln.
    pub chaff: u8,
    pub flares: u8,
    /// Täuschkörper in der Luft.
    pub decoys: Vec<Decoy>,
    /// Störsender (gegen Radarlenkung) an.
    pub ecm: bool,
    /// Infrarotstörer: Restzeit aus Wirkdauer und anschließender
    /// Abklingzeit (s, 0 = bereit); er wirkt, solange sie über der
    /// Abklingzeit liegt.
    pub ir_jammer_s: f64,
}

impl Default for Countermeasures {
    fn default() -> Self {
        Self { chaff: DECOY_STOCK, flares: DECOY_STOCK, decoys: Vec::new(), ecm: false, ir_jammer_s: 0.0 }
    }
}

impl Countermeasures {
    /// Störer bedienen (ECM umschalten, Infrarotstörer starten, wenn
    /// bereit) und einen Zeitschritt weiterzählen.
    pub fn jammers(&mut self, toggle_ecm: bool, start_ir: bool, dt: f64) {
        self.ecm ^= toggle_ecm;
        if start_ir && self.ir_jammer_s <= 0.0 {
            self.ir_jammer_s = IR_JAMMER_S + IR_COOLDOWN_S;
        }
        self.ir_jammer_s = (self.ir_jammer_s - dt).max(0.0);
    }

    /// Der Infrarotstörer wirkt.
    pub fn ir_active(&self) -> bool {
        self.ir_jammer_s > IR_COOLDOWN_S
    }

    /// Restliche Wirkdauer und Abklingzeit des Infrarotstörers (s).
    pub fn ir_left(&self) -> (f64, f64) {
        if self.ir_active() { (self.ir_jammer_s - IR_COOLDOWN_S, 0.0) } else { (0.0, self.ir_jammer_s) }
    }
}
