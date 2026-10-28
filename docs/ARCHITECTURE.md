# Architektur

## Überblick

```
                ┌──────────────┐
                │   f29-data   │  Flugzeugparameter (später: Waffen, Bedrohungen, Szenarien)
                └──────┬───────┘
┌──────────────┐       │
│ f29-terrain  │       │         deterministisches Gelände: (seed, x, y) → Höhe
└──────┬───────┘       │
       └───────┬───────┘
        ┌──────▼──────┐
        │  f29-core   │          Simulation: fester Tick, deterministisch, keine Engine
        └──┬───────┬──┘
   ┌───────▼──┐ ┌──▼─────────┐
   │ f29-hud  │ │ f29-render │   engine-neutral: Display-Liste / Meshes, Palette, Koordinaten
   └───┬──────┘ └──┬─────────┘
       │  ┌────────┴──────────────┐
  ┌────▼──▼─────┐          ┌──────▼───────┐
  │   f29-app   │          │ f29-headless │
  │   (Bevy)    │          │   (CLI)      │
  └─────────────┘          └──────────────┘
```

Abhängigkeiten zeigen nur nach unten. **Nur `f29-app` kennt Bevy.** Alles andere
kompiliert und testet ohne Grafikkarte, ohne Fenster und ohne externe Crates.

| Crate | Aufgabe |
|---|---|
| `f29-data` | Spieldaten als Typen + Plausibilitätsprüfung |
| `f29-terrain` | Rauschen, Höhenfunktion, Oberflächenklassen, Gitterabtastung |
| `f29-core` | Flugmodell, Atmosphäre, Sim-Schleife, Navigation, Replays, Autopilot, deterministische Mathematik |
| `f29-hud` | HUD und Instrumente als Liniensegmente, Vektorfont, SVG-Export |
| `f29-render` | Terrain-Kachel-Meshes, Flugzeugmodell, Palette, NED → Y-up |
| `f29-headless` | Simulation ohne Grafik: Demo, Replays, HUD als SVG, Höhenkarte, Benchmark |
| `f29-app` | Fenster, Eingabe, Kameras, Terrain-Streaming, Post-Processing |

## Koordinatensysteme

| System | Achsen | Wo |
|---|---|---|
| **Welt (NED)** | x = Nord, y = Ost, z = unten; Ursprung auf Meereshöhe | `f29-core`, `f29-terrain` |
| **Körper** | x = Nase, y = rechte Fläche, z = unten | Flugmodell, HUD-Projektion |
| **Darstellung (Y-up)** | x = Ost, y = oben, z = Süd; Modellnase = −z | `f29-render`, `f29-app` |

Die Umrechnung NED → Y-up ist eine echte Drehung (`coords.rs`). Dieselbe Abbildung
bringt Körperachsen auf Modellachsen, daher braucht das Modell keine Extra-Korrektur.

**Floating Origin:** Bevy rechnet in `f32`. Der Ursprung der Darstellung liegt in jedem
Frame im Flugzeug; alle Positionen werden in `f64` relativ dazu gebildet und erst dann
gerundet. Das hält die Darstellung auch hunderte Kilometer vom Weltursprung stabil.

## Simulation

- **Fester Zeitschritt:** 120 Hz (`TICK_HZ`). Das Frontend sammelt die echte Zeit in
  einem Akkumulator und führt so viele Ticks aus wie nötig. Dargestellt wird zwischen
  den letzten beiden Ticks interpoliert (Position linear, Lage normiert).
- **Eingaben** werden pro Tick als `ControlFrame` übergeben und dabei **quantisiert**
  (i16-Achsen, u16-Schub, Bitflags). Einmalige Ereignisse wie „nächster Steerpoint“
  sind Flags im Tick, in dem sie auftreten. Dadurch ist jeder Flug als Eingabestrom
  vollständig beschrieben.
- **Integrator:** semi-implizites Euler-Verfahren für die Translation;
  Lage-Quaternion über den exakten Rotationsvektor, nach jedem Schritt normiert.
- **Flugmodell:** siehe `docs/MECHANICS.md` (M-001) und `flight.rs`.

### Determinismus

Gleicher Seed + gleiche Eingaben ⇒ bitgleicher Zustand, auf jeder Plattform. Regeln
für `f29-core` und `f29-terrain`:

1. Transzendente Funktionen (`sin`, `atan2`, `exp`, `powf` …) **nur aus
   `f29_core::detmath`**. Die Plattform-Mathebibliotheken dürfen um einige ULP
   abweichen; `detmath` verwendet nur die von IEEE 754 exakt spezifizierten
   Grundrechenarten und `sqrt`. Rust kontrahiert Ausdrücke nicht eigenmächtig zu FMA.
2. Zufall nur über `Pcg32` mit **eigenem Strom pro Subsystem**, damit Änderungen in
   einem Subsystem die Zufallsfolge eines anderen nicht verschieben.
3. Keine Systemzeit, kein Iterieren über `HashMap`/`HashSet`, keine Threads im
   Sim-Schritt.
4. `Sim::state_hash()` hasht den vollständigen Zustand (FNV-1a über die Bitmuster).

Das Frontend darf beliebig nicht-deterministisch sein (Rendering, Interpolation,
Terrain-Streaming in Threads), solange es den Simulationszustand nur liest.

## Teststrategie

| Ebene | Was | Wo |
|---|---|---|
| Unit | detmath vs. `std` (≤ 1e-14), Quaternionen, ISA-Tabellenwerte, PCG32-Referenzfolge, Rauschen | `src/**` |
| Flugtests | Autopilot hält Höhe/Kurs/Fahrt, Rollrate, g-Begrenzer, stetiger Strömungsabriss (Abkippen, Erholung mit Fahrt, keine Sprünge), Abrisswarnung, Energieverlust in harter Kurve, Abriss im Steilflug, begrenzte Steigleistung, Höchstgeschwindigkeit, Luftbremse, Absturz, PULL-UP-Warnung | `f29-core/tests/flight.rs` |
| Determinismus | identische Eingaben ⇒ identischer Hash; eine Quantisierungsstufe Unterschied ⇒ anderer Hash; Replay-Textformat verlustfrei | `f29-core/tests/determinism.rs` |
| Golden Replay | 90-s-Flug mit festem Hash, läuft in CI auf Linux, Windows und macOS | `f29-core/tests/golden/patrol.replay` |
| Terrain | Seed-Stabilität, Kachelränder bitgleich, Stetigkeit, Normalen | `f29-terrain`, `f29-render` |
| HUD | alles bleibt bei 4:3 bis 21:9 im Bild, Projektion des Boresight, Leiter bewegt sich richtig, deterministisch | `f29-hud/src/tests.rs` |
| Daten | Flugzeugparameter plausibel; Shader-Palette = Rust-Palette | `f29-data`, `f29-render` |

**Golden-Hash aktualisieren** (nur nach *beabsichtigter* Verhaltensänderung):

```sh
F29_BLESS=1 cargo test -p f29-core --test determinism
```

## Darstellung (f29-app)

- **Kameras:** Eine 3D-Kamera mit Viewport auf den oberen Bildschirmteil; das
  Seitenverhältnis des Instrumentenbretts gibt `ScreenLayout` vor. Darüber liegt eine
  2D-Overlay-Kamera (eigener Render-Layer) für HUD und Instrumente.
- **Gelände:** Kacheln à 2048 m in drei Detailstufen (32/16/8 Zellen) bis 26 km
  Sichtweite. Sie werden im `AsyncComputeTaskPool` erzeugt; Schürzen verdecken Spalten
  zwischen Detailstufen.
- **Beleuchtung** ist in die Vertexfarben eingebacken (Lambert, flach), Materialien sind
  unbeleuchtet und ohne Tonemapping. Der Look hängt so nicht von Belichtungsparametern
  der Engine ab.
- **Retro-Look:** Post-Processing-Shader (`assets/shaders/retro.wesl`) quantisiert auf
  die 16-Farben-Palette mit Bayer-Dithering. Raster und Pixelierung beziehen sich auf
  eine *virtuelle* Zeilenzahl und skalieren mit der Fensterhöhe. Das HUD wird danach
  gezeichnet und bleibt gestochen scharf.
- **HUD:** `f29-hud` erzeugt Liniensegmente in Bildschirmeinheiten (Fensterhöhe = 1);
  die App multipliziert mit der Fensterhöhe und zeichnet 2D-Gizmo-Linien, deren Stärke
  mit der Auflösung wächst.

### Warum Bevy?

- Bevy bringt Fenster, Eingabe (inkl. Gamepad), einen wgpu-Renderer, Post-Processing,
  2D-Overlay und Asset-Handling mit. Ein eigener Renderer auf wgpu+winit würde Wochen
  Infrastruktur kosten, bevor das erste Flugzeug fliegt.
- Das ECS passt gut zu vielen gleichartigen Objekten (Bedrohungen, Ziele, Projektile)
  ab v0.2.
- Bevy erscheint etwa vierteljährlich mit Breaking Changes. Weil nur das dünne
  `f29-app` Bevy kennt, bleibt ein Upgrade eine überschaubare Arbeit. Im Notfall lässt
  sich das Frontend austauschen, ohne Simulation, HUD oder Geometrie anzufassen.
