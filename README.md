# F-29 Silent Sniper

Ein eigenständiger Stealth-Flugsimulator in Rust, inspiriert von *F-19 Stealth Fighter*
(MicroProse, 1988): Tarnkappen-Angriffsflüge bei Nacht, Radarlücken im Gelände und
Präzisionsangriffe, dazu eine Karriere mit Szenarien, Rängen und Orden. Alles wird neu
gebaut, mit eigener Physik, eigenen Werten und eigener Grafik im Retro-Stil, die in
jeder Auflösung scharf bleibt.

> **Clean Room:** Dieses Projekt enthält keinen Code, keine Daten und keine Assets des
> Originals. Mechaniken werden nach Verhaltensbeschreibung nachgebaut; das Vorgehen ist
> in [`docs/MECHANICS.md`](docs/MECHANICS.md) festgelegt und protokolliert.

**Status:** v0.1 abgeschlossen: Freiflug über prozeduralem Gelände mit HUD und zwei
MFDs, Start und Landung auf der Heimatbasis und geheimen Pisten, Treibstoff, Klang.
v0.2 (Angriff und Tarnung) in Arbeit: Bodenziele und Zielerfassung sind da, Waffen
folgen.

[![CI](https://github.com/peddn/f-29-silent-sniper/actions/workflows/ci.yml/badge.svg)](https://github.com/peddn/f-29-silent-sniper/actions/workflows/ci.yml)

| HUD und MFDs (`f29-headless hud-svg`) | Prozedurales Gelände, 120 × 120 km (`f29-headless terrain`) |
|---|---|
| ![HUD](docs/images/hud.svg) | ![Gelände](docs/images/terrain.png) |

## Schnellstart

Voraussetzung: aktuelles stabiles Rust (≥ 1.97).

```sh
# Spiel starten (beim ersten Mal dauert der Build einige Minuten)
cargo run --release -p f29-app

# anderer Seed = anderes Gelände, Start in der Verfolgeransicht
cargo run --release -p f29-app -- --seed 1234 --view chase

# Luftstart statt Start auf der Bahn
cargo run --release -p f29-app -- --start air

# Lautstärke in Prozent (Standard 80) bzw. ganz ohne Ton
cargo run --release -p f29-app -- --volume 50
cargo run --release -p f29-app -- --no-sound

# andere Beladung der vier Schächte: fall, chute, glide, agm, arm, ship oder -
cargo run --release -p f29-app -- --loadout fall,fall,agm,arm

# nach 10 s automatisch einen Screenshot speichern und beenden
cargo run --release -p f29-app -- --screenshot bild.png --after 10

# Tests (ohne Grafik, ohne Bevy – in Sekunden)
cargo test
```

Unter **Linux** braucht Bevy einige Systempakete, z. B. für Debian/Ubuntu:

```sh
sudo apt install libudev-dev libwayland-dev libxkbcommon-dev libasound2-dev pkg-config
```

## Steuerung

| Taste | Gamepad | Funktion |
|---|---|---|
| ↑ / W, ↓ / S | linker Stick vor/zurück | drücken / ziehen |
| ← / A, → / D | linker Stick links/rechts | rollen; am Boden lenken (auch im Stand) |
| Q / E | Trigger links/rechts | Seitenruder |
| `+` / `-` | rechter Stick vor/zurück | Schub stufenlos |
| 1 … 9, 0, `^`/`` ` `` | | Schub 10 … 90 %, 100 %, Leerlauf |
| B | B / Kreis | Luftbremse ein/aus; am Boden Radbremse |
| F | X / Quadrat | Klappen ein/aus – bei jeder Fahrt: langsam gut steuerbar, schnell weniger Fahrtverlust in Kurven |
| G | Y / Dreieck | Fahrwerk aus/ein (am Boden gesperrt) |
| N | A / Kreuz | nächster Navigationspunkt |
| V | rechter Stick drücken | Radar ein/aus (schaltet ohne Sichtlinie auf, erhöht später die eigene Sichtbarkeit) |
| O | linker Stick drücken | Schachtklappen auf/zu |
| Tab | linke Schultertaste | nächster Waffenschacht |
| Leertaste | rechte Schultertaste | Waffe auslösen (nur bei offenem Schacht) |
| I / L / K / J | Steuerkreuz | Blickrichtung der Zielerfassung: voraus / rechts / hinten / links (schaltet automatisch auf das nächste Ziel; dieselbe Richtung noch einmal = nächstweiteres Ziel; die Richtung folgt einem aufgeschalteten Ziel) |
| Shift+F1 / Shift+F2 | Select | Cockpit / Verfolger |
| Shift+F3 | | Retro-Look: voll → nur Palette → aus |
| Shift+F4 | | HUD ein/aus |
| F3 | | Karte (linkes MFD): Gitter ↔ Satellitenbild |
| F4 | | Rechtes MFD: nächste Seite (Mission → Ziel → Waffen → Triebwerk → Horizont → Systeme) |
| Y / X | | Karte näher / weiter (2, 5, 10, 20, 40 NM) |
| P | Start | Pause |
| F12 | | Screenshot nach `screenshots/` |
| R | | Neustart (Flug wird als Replay gesichert) |
| Esc | | Beenden (Flug wird als Replay gesichert) |

## Klang

Alle Klänge entstehen zur Laufzeit per Synthese im Retro-Stil, es gibt keine
Klangdateien. Die Abrisswarnung ist ein hoher, pulsierender Alarmton, die
Bodenwarnung eine Ansage „PULL UP“ – ohne eigene Aufnahme aus einem kleinen
eigenen Sprachsynthesizer. Eine Aufnahme kommt als PCM-WAV nach
`crates/f29-app/assets/sounds/pull_up.wav` (Details und Lizenzregeln in
`crates/f29-app/assets/sounds/README.md`). Hörproben ohne Spiel:
`cargo run -p f29-headless -- sound --out sounds`.

## Werkzeuge ohne Grafik

```sh
cargo run --release -p f29-headless -- demo --seconds 300 --record flug.replay
cargo run --release -p f29-headless -- replay flug.replay        # Hash-Prüfung
cargo run --release -p f29-headless -- replay replays/last.replay # letzter Flug aus dem Spiel
cargo run --release -p f29-headless -- hud-svg --seconds 30 --out hud.svg
cargo run --release -p f29-headless -- terrain --seed 7 --out karte.ppm
cargo run --release -p f29-headless -- airfields --seed 7          # Flugplätze und Route
cargo run --release -p f29-headless -- sound --out sounds          # Klänge als WAV-Hörproben
cargo run --release -p f29-headless -- bench
```

## Aufbau

```
crates/
  f29-data/      Spieldaten (Flugzeugparameter)
  f29-terrain/   deterministisches prozedurales Gelände
  f29-core/      Simulationskern: Flugmodell, fester Tick, Replays – keine Engine
  f29-hud/       HUD und Instrumente als Vektor-Display-Liste
  f29-render/    engine-neutrale Geometrie: Terrain-Meshes, Modell, Palette
  f29-audio/     prozedurale Klangsynthese und Sprachansagen – keine Engine
  f29-headless/  Kommandozeile: Demo, Replays, HUD-SVG, Höhenkarte, WAV-Hörproben
  f29-app/       Spiel (Bevy): Fenster, Eingabe, Kameras, Streaming, Retro-Shader
docs/
  ARCHITECTURE.md  Aufbau, Koordinatensysteme, Determinismus, Teststrategie
  MECHANICS.md     Clean-Room-Protokoll und Mechanik-Einträge
```

Nur `f29-app` hängt von Bevy ab. Simulation, Gelände, HUD und Geometrie sind ohne
Grafikkarte testbar. Die Simulation ist **bitgenau deterministisch**: Ein
Golden-Replay-Test prüft in der CI auf Linux, Windows und macOS denselben Zustands-Hash.
Details in [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Roadmap

### v0.1: Freiflug ✈️ *(abgeschlossen)*
- [x] Deterministischer Simulationskern, 120 Hz, Replays mit Hash-Prüfung
- [x] Flugmodell: Kräftebilanz + Fly-by-Wire (g-Begrenzer), ISA-Atmosphäre
- [x] Strömungsabriss mit Durchsacken, Fahrtverlust in Kurven, begrenzte Steigleistung
- [x] Prozedurales Gelände (Meer, Küsten, Gebirge, Wald) mit Kollision
- [x] HUD: Pitch-Ladder, Flight Path Marker, Kursband, Fahrt/Höhe, Querlage, Warnungen
- [x] Instrumente: Triebwerk, HSI mit Navigationspunkt, künstlicher Horizont
- [x] Navigationsroute mit Steerpoint-Anzeige in HUD und HSI
- [x] Retro-Look: 16-Farben-Palette, Dithering, auflösungsunabhängig
- [x] Cockpit- und Verfolgeransicht, Tastatur und Gamepad
- [x] Erster Spieltest auf echter Hardware (Linux, AMD Radeon, Vulkan)
- [x] Feinschliff Flugverhalten nach erstem Spielerfeedback (v0.1.1)
- [x] Radarhöhe über Grund immer im HUD
- [x] Turbulenz unter 1000 ft über Grund, Landeklappen, stärkere Schubwirkung (v0.1.4/0.1.5)
- [x] Treibstoff als Masse, quadratischer Verbrauch, Flameout, FUEL LOW / BINGO (v0.1.6)
- [x] Heimatbasis und geheime Pisten, Fahrwerk, Rollen und Wenden am Boden,
      Aufsetzbewertung, PAPI und Landehilfen im HUD, Start auf der Bahn (v0.1.6)
- [x] Instrumentenbrett mit zwei MFDs: Karte (Gitter/Satellit, Flugrichtung oben,
      zoombar) und Ressourcen (Treibstoff, Waffenstationen); Platz für die
      Stealth-/Radarimpuls-Anzeige (v0.1.10); Schubhebel, Konfiguration im HUD (v0.1.11)
- [x] Klang: prozedurale Retro-Synthese (Triebwerk, Fahrwerk, Klappen, Luftbremse,
      Aufsetzen, Absturz), gesprochene Abrisswarnung (v0.1.12); Abriss-Alarmton,
      PULL-UP-Ansage mit Platz für ein eigenes Sample (v0.1.14)
- [x] Feste Zeilen für GEAR, FLAPS und AIRBRAKE im HUD
- [x] Flugmodell nach Spieltests: Durchsacken im Langsamflug (v0.1.15), gefährlicher
      Abriss mit großem Höhenverlust (v0.1.16), vereinfachtes Abrissmodell und
      Kennwert-Tests mit `flight_report` (v0.1.17), mehr Schub, späterer
      Kurvenabriss, Klappen nicht mehr immun gegen Abriss (v0.1.18)
- [x] MFD-Seiten: Mission (Route, Heimweg, Ziele als Platzhalter), Waffen (vier
      Schächte, Platzhalter), Triebwerk mit Treibstoff, Reserve-Horizont,
      Systeme (v0.1.19)

### v0.2: Angriff und Tarnung *(in Arbeit)*
- [x] Bodenziele in neun eigenen Klassen, alle auf der Karte, Primär- und
      Sekundärziel; Zielerfassung optisch (mit Sichtlinie) und per Radar
      (abschaltbar, durch Berge), automatisch auf das nächste Ziel der
      Blickrichtung; HUD-Zielrahmen und MFD-Seite mit optischem Zielbild (v0.2.1)
- [x] Ziele einer Richtung durchschalten; Zielbild als gezoomte, verrauschte
      Kamera (v0.2.2); Aufschaltung bleibt beim Überflug, Blickrichtung folgt (v0.2.3)
- [x] Waffenschacht mit Klappen, Standardbeladung (`--loadout`), Bomben mit echter
      Ballistik (Freifall, Fallschirm), Wirkungsmatrix Waffe × Zielklasse mit
      Abstandsabfall, gehärtete Ziele nur bei steilem Eintreffen, Wracks,
      Splitterwarnung, WPN-Seite mit Bestand, Meldungen (v0.2.4)
- [x] Bordsysteme als Gerüst für das Schadensmodell: acht Systeme mit Zustand
      und Wirkung, Übersicht auf der SYS-Seite (v0.2.4)
- [x] Lenkwaffen mit eigenem Suchkopf (Fire and Forget): Gleitbombe,
      Luft-Boden-Rakete, Radarjäger, Seezielrakete; gewürfelter Endanflug nach
      Startgüte (Miss, Near Miss, Hit, 1 % kritisch), Schaden pro Treffer sammelt
      sich am Ziel, Zielrahmen zeigt Reichweite und gute Zone (v0.2.5)
- [ ] Zielhilfe im HUD: Aufschlagpunkt, Auslöseklammer, Warnung „zu tief“ (v0.2.6)
- [ ] Bodenziele und Boden-Luft-Bedrohungen (Radar-SAM, Flak)
- [ ] Radar-/Detektionsmodell: aspektabhängiger Radarquerschnitt, Geländemaskierung,
      Emissionen; Signaturanzeige im Cockpit
- [ ] Täuschkörper (Düppel, Fackeln)

### v0.3: Missionen
- [ ] Missionsdaten: Primär- und Sekundärziel, Route, Bedrohungslage
- [ ] Bewaffnung und Betankung vor dem Start (Zusatztanks), Missionsauswertung
      mit Landebewertung, Verbrauchsvorschau auf der Karte
- [ ] Flugzeugträger

### v0.4: Luftkampf
- [ ] Weitwinkelsicht ohne MFDs für den Luftkampf (Taste C)
- [ ] Luft-Luft-Raketen und Bordkanone
- [ ] Feindliche Jäger und Abfangjäger mit KI (aufbauend auf dem Autopiloten)
- [ ] Radarwarnempfänger

### v0.5: Karriere
- [ ] Pilot mit Laufbahn: Missionsfolge, Punkte, Ränge, Orden (eigene Namen)
- [ ] Mehrere eigene, fiktive Einsatzgebiete (Szenarien)
- [ ] Konfliktstufen von „Spannung“ bis „offener Krieg“ mit unterschiedlichen
      Einsatzregeln
- [ ] Verlauf: Abschuss, Gefangennahme, Rettung, Ruhestand; Spielstände

### v0.6: Präsentation
- [ ] Menüs, Briefing- und Debriefing-Bildschirme, Karte
- [ ] Klang: Waffen, Radarwarnung; Musik im Startbildschirm
- [ ] Einstellungen, frei belegbare Tasten, Joystick/HOTAS

### v1.0: Vollständige Karriere
- [ ] Karrieremodus mit mindestens vier Einsatzgebieten, kompletter Ordensliste und
      Ruhmeshalle
- [ ] Balancing über alle Schwierigkeitsgrade

## Lizenz

Wahlweise [MIT](LICENSE-MIT) oder [Apache 2.0](LICENSE-APACHE).

*F-19 Stealth Fighter* ist eine Marke ihrer jeweiligen Rechteinhaber. Dieses Projekt
ist ein unabhängiges Fanprojekt und steht in keiner Verbindung zu MicroProse oder
dessen Rechtsnachfolgern.
