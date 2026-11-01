//! Zustandshash für Determinismus und Referenz-Replays.

use super::*;

impl Sim {
    /// Hash über den vollständigen Simulationszustand (FNV-1a, 64 Bit).
    ///
    /// Gleicher Seed + gleiche Eingaben ⇒ gleicher Hash, auf jeder Plattform.
    pub fn state_hash(&self) -> u64 {
        let mut h = Fnv1a::new();
        h.u64(self.config.seed);
        h.u64(self.tick);
        match self.status {
            Status::Flying => h.u64(0),
            Status::Crashed { tick, cause } => {
                h.u64(1);
                h.u64(tick);
                h.u64(cause.code());
            }
        }
        let a = &self.aircraft;
        for v in [a.position, a.velocity, a.body_rates] {
            h.f64(v.x);
            h.f64(v.y);
            h.f64(v.z);
        }
        h.f64(a.attitude.w);
        h.f64(a.attitude.x);
        h.f64(a.attitude.y);
        h.f64(a.attitude.z);
        h.f64(a.engine);
        h.f64(a.airbrake);
        h.f64(a.stall);
        h.f64(a.flaps);
        h.f64(a.fuel_kg);
        h.f64(a.gear);
        h.u64(u64::from(a.on_ground));
        if let Some(td) = &self.touchdown {
            h.u64(td.tick);
            h.f64(td.sink_mps);
        }
        h.f64(self.gust.x);
        h.f64(self.gust.y);
        let (rs, ri) = self.turbulence_rng.raw_state();
        h.u64(rs);
        h.u64(ri);
        h.u64(self.active_steerpoint as u64);
        h.f64(a.stores_kg);
        let st = &self.stores;
        for b in &st.bays {
            h.u64(b.kind.map_or(99, |k| k as u64));
            h.u64(u64::from(b.count));
        }
        h.u64(st.selected as u64 | u64::from(st.gun_selected) << 8);
        for (kind, value) in self.systems.hash_parts() {
            h.u64(kind);
            h.f64(value);
        }
        let (ws, wi) = st.rng_state();
        h.u64(ws);
        h.u64(wi);
        h.f64(st.door);
        h.u64(u64::from(st.door_cmd));
        for w in &st.in_flight {
            h.u64(u64::from(w.id));
            for v in [w.position, w.velocity] {
                h.f64(v.x);
                h.f64(v.y);
                h.f64(v.z);
            }
            h.f64(w.age_s);
            h.u64(match w.target {
                None => u64::MAX,
                Some(Track::Ground(i)) => i as u64,
                Some(Track::Aircraft) => u64::MAX - 1,
            });
            h.u64(u64::from(w.tracking) | u64::from(w.searching) << 1 | u64::from(w.blinded) << 2);
            h.u64(w.owner.map_or(u64::MAX, |o| o as u64));
        }
        let cm = &st.countermeasures;
        h.u64(u64::from(cm.chaff) | u64::from(cm.flares) << 8 | u64::from(cm.ecm) << 16);
        h.f64(cm.ir_jammer_s);
        for d in &cm.decoys {
            h.u64(u64::from(d.id) | u64::from(d.salt) << 32);
            h.u64(d.owner.map_or(u64::MAX, |o| o as u64));
            h.f64(d.age_s);
        }
        for p in [self.air, self.runway] {
            h.u64(p.index.map_or(u64::MAX, |i| i as u64) ^ u64::from(p.look.code()) << 56);
        }
        for f in &self.fighters.list {
            h.u64(u64::from(f.id) | (f.mode as u64) << 32 | (f.base as u64) << 40);
            for v in [f.position, f.goal] {
                h.f64(v.x);
                h.f64(v.y);
                h.f64(v.z);
            }
            for v in [f.heading, f.pitch, f.bank, f.speed, f.flight_s] {
                h.f64(v);
            }
            h.u64(f.contact_tick.unwrap_or(u64::MAX));
        }
        for s in &self.radar.sites {
            h.u64(u64::from(s.alert));
            h.u64(s.last_change_tick | u64::from(s.painted) << 63);
            if let Some(p) = s.ping_point {
                h.f64(p.x);
                h.f64(p.y);
                h.f64(p.z);
            }
        }
        let g = &self.gun;
        h.u64(u64::from(g.rounds) | u64::from(g.trigger) << 16);
        h.f64(g.pending());
        let (gs, gi) = g.rng_state();
        h.u64(gs);
        h.u64(gi);
        for r in &g.in_flight {
            for v in [r.position, r.velocity] {
                h.f64(v.x);
                h.f64(v.y);
                h.f64(v.z);
            }
            h.f64(r.age_s);
        }
        h.u64(u64::from(self.sensors.radar_on) | u64::from(self.sensors.night) << 1);
        h.u64(u64::from(self.sensors.look.code()));
        if let Some(l) = self.sensors.lock {
            h.u64(l.target as u64);
            h.u64(u64::from(l.optical) | u64::from(l.radar) << 1);
        }
        for t in &self.targets {
            h.u64(t.state as u64);
            h.f64(t.damage);
        }
        h.finish()
    }
}

/// FNV-1a über 64-Bit-Wörter.
#[derive(Debug, Clone)]
pub struct Fnv1a(u64);

impl Fnv1a {
    pub fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    pub fn u64(&mut self, v: u64) {
        for b in v.to_le_bytes() {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }

    /// Hasht das Bitmuster; -0.0 und 0.0 werden gleich behandelt.
    pub fn f64(&mut self, v: f64) {
        let v = if v == 0.0 { 0.0 } else { v };
        self.u64(v.to_bits());
    }

    pub fn finish(&self) -> u64 {
        self.0
    }
}

impl Default for Fnv1a {
    fn default() -> Self {
        Self::new()
    }
}
