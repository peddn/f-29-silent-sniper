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

# Nachtflug: dunkle Darstellung, optische Erfassung nur noch 20 km
cargo run --release -p f29-app -- --night

# ohne freundliches AWACS: ferne Ziele fehlen auf der Karte
cargo run --release -p f29-app -- --no-awacs

# Testoption: Stealth-Level immer 0, jeder Radarping in Reichweite trifft
cargo run --release -p f29-app -- --no-stealth

# Lautstärke in Prozent (Standard 80) bzw. ganz ohne Ton
cargo run --release -p f29-app -- --volume 50
cargo run --release -p f29-app -- --no-sound

# andere Beladung der vier Schächte: fall, chute, lgb, agm, arm, ship oder -
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

| Taste | Joystick (Extreme 3D Pro) | Funktion |
|---|---|---|
| ↑ / W, ↓ / S | Knüppel vor/zurück | drücken / ziehen |
| ← / A, → / D | Knüppel links/rechts | rollen; am Boden lenken (auch im Stand) |
| Q / E | Drehgriff | Seitenruder |
| `+` / `-` | Schubregler | Schub stufenlos (der Regler gilt, sobald er bewegt wird) |
| 1 … 9, 0, `^`/`` ` `` | | Schub 10 … 90 %, 100 %, Leerlauf |
| B | 9 | Luftbremse ein/aus; am Boden Radbremse |
| F | 8 | Klappen ein/aus – bei jeder Fahrt: langsam gut steuerbar, schnell weniger Fahrtverlust in Kurven |
| G | 7 | Fahrwerk aus/ein (am Boden gesperrt) |
| N | 6 | nächster Navigationspunkt |
| V | 5 | Radar ein/aus (schaltet ohne Sichtlinie auf; eingeschaltet etwas sichtbarer für feindliche Radare, wie ein offener Schacht) |
| O | 4 | Schachtklappen auf/zu |
| Tab | 3 | nächste Waffe: GUN → BAY 1 → … → BAY 4 → GUN (Start: GUN) |
| Leertaste | 2 | gewählte Waffe auslösen (Schachtwaffen nur bei offenem Schacht; die Kanone feuert, solange die Taste gehalten wird) |
| Rücktaste (gehalten) | 1 (Abzug) | Bordkanone feuern, gleich welche Waffe gewählt ist (nicht am Boden) |
| H | 12 | HUD-Modus: NAV → AG (Luft-Boden) → AA (Luft-Luft) |
| I / L / K / J | Coolie-Hat | Blickrichtung der Zielerfassung: voraus / rechts / hinten / links (schaltet automatisch auf das nächste Ziel; dieselbe Richtung noch einmal = nächstweiteres Ziel; die Richtung folgt einem aufgeschalteten Ziel) |
| Shift+F1 / Shift+F2 | | Cockpit / Verfolger |
| Shift+F3 | | Retro-Look: voll → nur Palette → aus |
| Shift+F4 | | HUD ein/aus |
| F3 | | Karte (linkes MFD): Gitter ↔ Satellitenbild |
| F4 | 11 | Rechtes MFD: nächste Seite (Mission → Ziel → Waffenkamera → Waffen → Triebwerk → Horizont → Systeme) |
| Y / X | 10 (reihum) | Karte näher / weiter (5, 10, 20, 40, 80 km) |
| P | | Pause |
| F12 | | Screenshot nach `screenshots/` |
| R | | Neustart (Flug wird als Replay gesichert) |
| Esc | | Beenden (Flug wird als Replay gesichert) |

### Joystick

Unterstützt wird ein Joystick mit vielen Tasten unter **Linux**; voreingestellt
ist der **Logitech Extreme 3D Pro** (Belegung oben). Die Belegung steht als
Text in [`config/joystick.cfg`](config/joystick.cfg). Eine eigene Fassung als
`joystick.cfg` im Startverzeichnis (oder `--joystick DATEI`) ersetzt sie;
nicht belegte Aktionen wie `view` oder `pause` lassen sich dort ergänzen.

```sh
cargo run --release -p f29-app -- --joystick-debug
```

meldet beim Anstecken den Namen des Sticks und danach jede Achse und Taste auf
der Konsole – in derselben Schreibweise wie die Datei (z. B. `Taste B3
gedrückt`, `Achse ABS 6 -0.84`). Falls Achsen falsch herum laufen, hilft
`invert` hinter der Achse. Liest das Spiel keinen Stick, fehlt meist das
Leserecht auf `/dev/input/event*` (Gruppe `input`).

## Klang

Die Klänge entstehen zur Laufzeit per Synthese im Retro-Stil. Nur die beiden
Ansagen für „PULL UP“ und „OVER G“ sind Samples: `crates/f29-audio/sounds/*.wav`
(PCM, eingebettet). Vorerst CC0-Platzhalter aus Kenneys Voiceover Pack
(„Look out!“, „Hold!“), Herkunft in `crates/f29-audio/sounds/README.md`. Wer sie
ersetzen will, legt dort eine andere PCM-WAV-Datei ab (8/16 Bit, beliebige
Abtastrate; nur frei lizenzierte Aufnahmen, keine aus anderen Spielen).
Hörproben ohne Spiel: `cargo run -p f29-headless -- sound --out sounds`.

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
- [x] Cockpit- und Verfolgeransicht, Tastatur und Gamepad (Gamepad in v0.2.8 durch Joystick ersetzt)
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
      sich am Ziel, Zielrahmen zeigt Reichweite und Sweet Spot (v0.2.5)
- [x] Waffen mit realen Bezeichnungen (Mk 82, Mk 82 Snakeye, GBU-12 Paveway II,
      AGM-65 Maverick, AGM-88 HARM, AGM-84 Harpoon); Aufschaltung bleibt auf dem
      Wrack, zerstörte Ziele verschwinden von der Karte; gewählte Waffe unten
      links im HUD; Maverick nur mit optischer Aufschaltung, ohne Aufschaltung
      geradeaus in Abschusslage (v0.2.5)
- [x] Zielhilfe für Bomben im HUD: Aufschlagpunkt mit Fall-Linie, Warnung
      „TOO LOW“ (v0.2.6)
- [x] Lenkwaffen vereinfacht: Abschusskreis ±8° für Raketen, Startgüte nur aus
      der Entfernung (Maverick 16 km, Sweet Spot 8 km), Flug zeigt den Wurf;
      lasergelenkte GBU-12 mit Gleitkorridor und Beleuchtung bis zum Einschlag
      (v0.2.7)
- [x] Sensoren 50 km (nachts optisch 20 km), Nachtflug, AWACS-Datalink für
      Ziele außerhalb der eigenen Erfassung, Feuersignal „IN RANGE“/„SHOOT“,
      HUD-Warnfarbe blau (v0.2.7)
- [x] HUD überarbeitet: breiteres Glas, drei Modi NAV/AG/AA (Taste H), Kanone als
      erste Waffe in der Wahl GUN, BAY 1–4 (v0.2.8)
- [x] Joystick statt Gamepad: Logitech Extreme 3D Pro unter Linux, Belegung als
      Textdatei, `--joystick-debug` (v0.2.8)
- [ ] Joystick am echten Extreme 3D Pro testen (`--joystick-debug`): Achsen,
      Richtungen und Tastennummern prüfen (vorgemerkt)
- [ ] Auslöseklammer für Bomben, neu entworfen (vorgemerkt)
- [x] Bordkanone als eigenes Waffensystem: 600 Schuss (50/s), Rücktaste, 2 %
      Schaden pro Treffer bei jedem Ziel, Kanonenkreuz am Aufschlagpunkt,
      Leuchtspur (v0.2.8)
- [x] Entdeckungsmodell: SAM-Stellungen (Doppler) und Frühwarnradare (Impuls)
      pingen je Antennenumlauf; Impuls gegen Stealth-Level (wie im Original
      ein Wert: Höhe über Grund, Querlage, Schacht, eigenes Radar – tief und
      waagerecht fliegen); Alarmstufen 1–3 mit
      Abklingen, gemerkter Ping-Ort; SIG-Anzeige, Pieps, Reichweitenkreise
      (v0.2.9)
- [x] Lenkwaffen physikalisch wie im Original: reine Verfolgung, Treffer
      geometrisch (Annäherungszünder), Wirkung gewürfelt; „SHOOT“ aus dem
      vorausberechneten Flug statt Sweet Spot (v0.2.9)
- [x] Karte aufgeräumt: Höhenbild in Graustufen statt Höhenlinien, größere
      und einfachere Zielsymbole (v0.2.9)
- [x] Feuersignal „LOCKED“/„SHOOT“; Waffenkamera der Maverick auf der Seite
      WPN CAM, nach dem Abschuss automatisch aufgeschaltet (v0.2.9)
- [x] Boden-Luft-Raketen ab „RADAR ID“: SA-6 der SAM-Stellungen, SA-5 der
      Frühwarnradare; halbaktiv (weit weg nur, solange die Stellung uns
      erfasst), Treffer beschädigen zwei Bordsysteme; Raketenwarnung mit Ton,
      Rauchspur (v0.2.9)
- [x] Drei SAM-Typen mit Startprofil: SA-8 (Kurzstrecken-Flugabwehr bei den
      Zielen, direkt), SA-6 (Boost und Bogen), SA-5 (fast senkrechter Start,
      ausgeprägter Bogen); Vorhalt für alle Lenkwaffen (v0.2.9)
- [ ] Täuschkörper (Düppel, Fackeln) gegen Langstreckenraketen
- ~~Flak~~ (gestrichen, Spielerentscheidung v0.2.9)

### v0.3: Missionen
- [ ] Einsatzgebiete als Daten (M-031): eigenes fiktives Küstengebiet, rund
      400–500 km, Grobraster 16 × 16 (Land/Wasser, Geländeart, Zugehörigkeit)
      steuert das prozedurale Gelände; feste Objekte (Basen, Träger, geheime
      Pisten, Radarnetz, Städte, Häfen, Brücken, Kraftwerke)
- [ ] Missionsgenerator nach dem Vorbild des Originals: Ziele aus dem Gebiet,
      nächste eigene Startbasis, Strecke in einem Band, Route aus Basis und
      Zielen
- [ ] Planquadrate auf Karte und Missionsseite; Geländeart als Radarfaktor
- [ ] Missionsdaten: Primär- und Sekundärziel, Route, Bedrohungslage
- [ ] Bewaffnung und Betankung vor dem Start (Zusatztanks), Missionsauswertung
      mit Landebewertung, Verbrauchsvorschau auf der Karte
- [ ] Flugzeugträger

### v0.4: Luftkampf
- [ ] Weitwinkelsicht ohne MFDs für den Luftkampf (Taste C)
- [ ] Luft-Luft-Raketen, Bordkanone gegen Luftziele
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
