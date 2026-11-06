# Radar und Entdeckung

Die F-29 ist ein Tarnkappenflugzeug: Feindliche Radare sehen sie schlecht,
aber nie gar nicht. Ob eine Radarstellung das Flugzeug entdeckt, hängt davon
ab, wie stark ihr Impuls beim Flugzeug ankommt und wie sichtbar sich das
Flugzeug gerade macht. Kurz gesagt: **tief, waagerecht und leise fliegen,
Hügel und Gebirge nutzen – und Querflug zu Dopplerradaren.**

Code: `crates/f29-core/src/radar.rs`, Entwicklerreferenz: `docs/MECHANICS.md`
(M-020).

## Radarstellungen

| Stellung (Karte) | Radar | Reichweite | Antennenumlauf | Rakete (M-021) |
|---|---|---|---|---|
| EW RADAR (Frühwarnung) | Impuls | 60 km | 10 s | SA-5 |
| SAM SITE (Flugabwehr) | Doppler | 30 km | 5 s | SA-6 |
| SHORAD (Kurzstrecke) | Doppler | 12 km | 2,5 s | SA-13 |

Jede Stellung „pingt“ einmal je Antennenumlauf, die Stellungen gegeneinander
versetzt. Nur in diesem Moment wird gerechnet. Eine zerstörte Stellung
strahlt nicht mehr und vergisst alles, was sie wusste.

## Der Impuls

Die Stärke des Impulses am Flugzeug liegt zwischen 0 und 1:

- **Entfernung:** Direkt an der Stellung ist sie 1, bis zur Reichweite fällt
  sie gleichmäßig auf 0.
- **Impulsradar:** sieht in jeder Flugrichtung gleich gut.
- **Dopplerradar:** sieht nur die Bewegung auf die Stellung zu oder von ihr
  weg. Im reinen Querflug kommt nur ein Viertel des Impulses an.
- **Gelände:** Über Hügeln und Gebirge reicht jedes Radar nicht so weit (siehe
  unten).

## Die eigene Sichtbarkeit

Die Sichtbarkeit ist ein Wert für alle Radare zugleich. Sie setzt sich
zusammen aus:

| Anteil | Wert |
|---|---|
| Grundwert (immer) | 10 % |
| Höhe über Grund: ab 100 m steigend, erst schnell, dann immer langsamer | bis 40 % (1000 m ≈ 14 %, 3000 m ≈ 28 %) |
| Querlage: gleichmäßig bis 90° (Rückenflug zählt wie waagerecht) | bis 30 % |
| Schachtklappen offen | 15 % |
| Eigenes Radar an (Taste V) | 15 % |
| Störsender an (Taste M) | 15 % |

Das **Stealth-Level** ist 1 − Sichtbarkeit, höchstens 90 %. Nicklage,
Fahrwerk und Klappen zählen nicht.

## Treffer und Alarmstufen

Ist der Impuls eines Pings stärker als das Stealth-Level, ist das ein
**Treffer**, und die Alarmstufe der Stellung steigt um eins:

1. **RADAR DETECTED:** Die Stellung kennt das Flugzeug, ihr Reichweitenkreis
   erscheint auf der Karte.
2. **TRACKED:** Sie merkt sich den Ort des Pings. Dorthin schicken die
   feindlichen Flugplätze ihre Jäger.
3. **RADAR ID:** Die Stellung startet eine Rakete, ebenso bei jedem weiteren
   Treffer – sofern das Regelset es erlaubt (siehe unten).

Ohne Treffer sinkt die Stufe nach 30 s um eins. Bei 0 meldet das Cockpit
„LOST CONTACT“.

## Das Gelände

Radare sehen durch Berge; eine Sichtlinie braucht nur das eigene optische
System. Die Geländeart unter dem Flugzeug verkürzt aber im Tiefflug die
Reichweite jedes feindlichen Radars, auch des Bordradars der Jäger:

| Gelände | Reichweite |
|---|---|
| Wasser, Flachland, Wüste | 100 % |
| Dorf | 90 % |
| Wald | 85 % |
| Stadt | 80 % |
| Hügel | 70 % |
| Gebirge | 45 % |

Wald, Stadt und Dorf stehen für Bodenechos: Bebauung und Bäume verdecken
ein tief fliegendes Flugzeug etwas, aber weniger als Berge.

- **Höhe:** Voll gilt das bis 100 m über Grund. Bis 1000 m läuft es
  gleichmäßig aus, darüber zählt das Gelände nicht mehr.
- **Übergänge:** Die Geländeart kommt aus dem Grobraster des Gebiets (64 × 64
  Zellen zu je knapp 8 km). Sie geht so weich und unregelmäßig über wie die
  Landschaft selbst, ohne Sprung an den Zellgrenzen. Die Gebirgsketten sind
  schmal; den vollen Faktor gibt es nur über ihrem Kamm.
- **Nie ganz verdeckt:** Weil der Impuls gleichmäßig mit der Entfernung
  fällt, entdeckt jedes Radar über rauem Gelände im selben Verhältnis erst
  näher, aber immer noch.
- **Erkennen:** Die linke Karte zeigt die Geländearten im Satellitenbild
  (F3).

## Wie nah darf man heran?

Entfernung, unter der eine Stellung das Flugzeug entdeckt (Impulsradar bzw.
Dopplerradar im Anflug auf die Stellung):

| Lage | SAM SITE | EW RADAR |
|---|---|---|
| tief, waagerecht, Schacht zu, über Flachland | 3 km | 6 km |
| dasselbe über Gebirge | 1,4 km | 2,7 km |
| tief, Schacht offen, über Flachland | 7,5 km | 15 km |
| tief, Schacht offen, über Gebirge | 3,4 km | 6,8 km |
| 3000 m, waagerecht | 11 km | 23 km |
| 3000 m, 90° Querlage | 20 km | 41 km |

Faustregel: Die Entdeckungsentfernung ist Sichtbarkeit × Reichweite der
Stellung × Geländeanteil. Im Querflug zu einem Dopplerradar entdeckt es erst
ab 75 % Sichtbarkeit – tief und sauber also nie.

## Jäger

Feindliche Jäger tragen ein eigenes Radar. Für sie gelten derselbe Impuls,
dieselbe Sichtbarkeit und dasselbe Gelände, nur mit eigenen Werten:

| Jäger | Radar | Reichweite | Umlauf | Blickkegel |
|---|---|---|---|---|
| MiG-21 | Impuls | 15 km | 2 s | ±30° |
| MiG-29 | Doppler | 35 km | 2 s | ±60° |

Ein Treffer gibt dem Jäger Kontakt; was er dann tut, beschreibt das Kapitel
zu den feindlichen Flugzeugen (folgt).

## Anzeigen im Cockpit

- **SIG-Säule** zwischen den Bildschirmen: Der Balken zeigt das
  Stealth-Level. Jeder Ping erscheint eine Sekunde lang als Querstrich in
  seiner Stärke – rot über dem Balken ein Treffer, gelb darunter keiner.
- **Alarmlampen:** drei Stück, eine je Stufe, dazu der Radartyp (PLS/DOP).
- **Radarwarnempfänger:** piept bei jedem Ping, bei Treffern doppelt und
  höher.
- **Karte:** Reichweitenkreise entdeckender Stellungen und der zuletzt
  gemerkte Ping-Ort. Die Kreise zeigen die volle Reichweite, ohne Gelände.

## Regelsets

| Regelset | Feuer | Raketen zugleich | Abzug je Entdeckung | Angriff erlaubt |
|---|---|---|---|---|
| COLD WAR | nur über feindlichem Gebiet | 2 | 30 Punkte | nur, was entdeckt hat (Stellung mit Alarm, Jäger mit Radarkontakt), sonst ROE VIOLATION |
| LIMITED | überall | 2 | 10 Punkte | alles |
| OPEN WAR | überall | 4 | 0 | alles |

Wer die entdeckende Stellung später zerstört, bekommt den Abzug zurück.

## Testoption

`--no-stealth` setzt das Stealth-Level dauerhaft auf 0: Jeder Ping in
Reichweite trifft. Die Option steht im Replay.

## Stellschrauben

| Datei | Name | Wert | Wirkung |
|---|---|---|---|
| `f29-core/src/radar.rs` | `EARLY_WARNING`, `SAM`, `SHORAD` | Impuls 60 km / 10 s, Doppler 30 km / 5 s, Doppler 12 km / 2,5 s | Art, Reichweite und Umlauf der Stellungen; die Reichweite skaliert alle Entdeckungsentfernungen, der Umlauf das Tempo der Alarmstufen |
| `f29-core/src/radar.rs` | `BEAM_FLOOR` | 0,25 | Anteil des Impulses, den ein Dopplerradar im reinen Querflug sieht |
| `f29-core/src/radar.rs` | `SIG_BASE` | 0,10 | Sichtbarkeit, die immer bleibt; bestimmt die Entdeckung im sauberen Tiefflug |
| `f29-core/src/radar.rs` | `LOW_LEVEL_M` | 100 m | bis hier zählt die Höhe nicht, und das Gelände wirkt voll |
| `f29-core/src/radar.rs` | `ALTITUDE_SCALE_M`, `SIG_ALTITUDE` | 1500 m, 0,40 | wie schnell und wie weit die Sichtbarkeit mit der Höhe steigt |
| `f29-core/src/radar.rs` | `SIG_BANK` | 0,30 | Zuschlag bei 90° Querlage |
| `f29-core/src/radar.rs` | `SIG_BAY`, `SIG_RADAR`, `SIG_ECM` | je 0,15 | Zuschlag für offenen Schacht, eigenes Radar, Störsender |
| `f29-core/src/radar.rs` | `ground_factor` | Hügel 0,7, Gebirge 0,45, Stadt 0,8, Wald 0,85, Dorf 0,9, sonst 1 | Reichweitenanteil über rauem Gelände |
| `f29-core/src/radar.rs` | `GROUND_TOP_M` | 1000 m | bis hier läuft die Wirkung des Geländes aus |
| `f29-core/src/radar.rs` | `MAX_ALERT`, `DECAY_S` | 3, 30 s | Zahl der Alarmstufen (die letzte startet Raketen), Abklingzeit je Stufe ohne Treffer |
| `f29-core/src/targets.rs` | `air_defense` | EW → SA-5, SAM → SA-6, SHORAD → SA-13 | welches Radar und welche Rakete zu welcher Stellung gehört |
| `f29-core/src/fighters.rs` | `radar`, `radar_cone_rad` der Jägertypen | siehe Tabelle Jäger | Bordradar der Jäger |
| `f29-core/src/rules.rs` | `fire_only_over_hostile`, `max_hostile_missiles`, `detection_penalty`, `attack_needs_detection` | siehe Tabelle Regelsets | Folgen einer Entdeckung je Regelset |
| `data/theaters/*.ron` | `objects` der Radarklassen | Lage und Zahl je Gebiet | wie viele Stellungen wo stehen – die größte Stellschraube; empfohlen mindestens 35 km Abstand zur eigenen Basis (`docs/GEBIETSFORMAT.md`) |
| `f29-world/src/lib.rs` | `GRID_WARP` | 0,4 Zellen | wie unregelmäßig die Geländegrenzen verlaufen – für Landschaft und Radar zugleich |
| `data/theaters/*.ron` | `grid.ground` | `~ . h M d f C v` je Zelle | wo Wasser, Flachland, Hügel, Gebirge, Wüste, Wald, Stadt und Dorf liegen |
| `f29-hud/src/mfd.rs` | `PING_SHOW_S` | 1 s | wie lange ein Ping in der SIG-Säule steht |
