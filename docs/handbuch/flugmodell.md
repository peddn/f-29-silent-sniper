# Flugmodell

Die F-29 ist ein schweres Angriffsflugzeug mit zwei Triebwerken ohne
Nachbrenner, das aus Gründen der Tarnung untermotorisiert ist. Sie fliegt
gutmütig, solange sie Fahrt hat, und verzeiht wenig, wenn die Fahrt fehlt.
Kurz gesagt: **Fahrt ist Leben.** Harte Kurven kosten viel Fahrt, steiles
Steigen endet im Abriss, und wer langsam zu hart zieht, verliert viel Höhe.
Zum Landen und für den Langsamflug gibt es die Klappen.

Code: `crates/f29-core/src/flight.rs`, Werte in `crates/f29-data/src/aircraft.rs`
(`F29_SPEC`). Entwicklerreferenz: `docs/MECHANICS.md` (M-001 Flugmodell,
M-004 Abriss, M-006 Turbulenz, M-007 Klappen, M-009 Treibstoff, M-041 Boden,
M-042 Landung).

## Das Flugzeug

| Größe | Wert |
|---|---|
| Leermasse | 16 300 kg |
| Treibstoff (innen) | 5200 kg |
| Waffen (übliche Beladung) | etwa 1,8 t |
| Flügelfläche | 70 m² |
| Schub (beide Triebwerke, Meereshöhe) | 110 kN |
| Schub/Gewicht (volle Tanks, ohne Waffen) | 0,52 |

Treibstoff und Waffen sind echte Masse. Mit vollen Tanks ist das Flugzeug
träge. Gegen Ende des Flugs, und nach dem Abwurf der Waffen, ist es leichter:
Es steigt besser und reißt später ab.

## Wie es fliegt

Das Spiel rechnet in jedem Takt die Kräfte am Flugzeug: Auftrieb, Widerstand,
Seitenkraft, Schub und Gewicht. Fahrt, Steigen, Fahrtverlust in Kurven und der
Strömungsabriss ergeben sich daraus.

- **Auftrieb** wächst mit dem Anstellwinkel, also dem Winkel zwischen Nase und
  Flugbahn. Ab 18° reißt die Strömung ab (siehe unten).
- **Widerstand** hat drei Teile:
  - einen Grundwiderstand, der mit dem Quadrat der Fahrt wächst und die
    Höchstfahrt bestimmt;
  - einen auftriebsbedingten Widerstand, der mit dem Quadrat des Auftriebs
    wächst. Deshalb kosten harte Kurven so viel Fahrt; die facettierte Zelle
    hat davon besonders viel;
  - Zuschläge für Fahrwerk, Klappen und Luftbremse.
- **Schub** folgt dem Schubhebel mit etwa einer Sekunde Verzögerung. Er nimmt
  mit der Luftdichte ab: in 3000 m auf 80 %, in 6000 m auf 63 %, in 9000 m auf
  49 %. Im Leerlauf bremst das Triebwerk selbst spürbar. Gas wegnehmen ist
  deshalb eine echte Bremse: In 3000 m bringen 10 s Vollgas ab 720 km/h etwa
  +80 km/h, 10 s Leerlauf etwa −110 km/h.
- **Luft:** Standardatmosphäre. Alle Fahrtangaben in den Anzeigen sind wahre
  Fahrt in km/h. Weil die Luft oben dünner ist, liegt auch die Abrissfahrt
  dort höher.
- **Kein Schallmauer-Widerstand:** Bewusst gibt es keinen Widerstandsanstieg
  nahe Mach 1. In großer Höhe wird die F-29 deshalb knapp überschallschnell.

## Steuerung

Die F-29 hat eine Fly-by-Wire-Steuerung: Der Knüppel gibt eine **Drehrate**
vor, keinen Ruderausschlag.

- **Knüppel neutral:** Die Lage bleibt. Im Horizontalflug steht die Nase um den
  Anstellwinkel über dem Horizont; wohin das Flugzeug wirklich fliegt, zeigt der
  Flugbahnmarker.
- **Höchste Drehraten:** Nicken 26°/s, Rollen 150°/s, Gieren 11°/s. Das Flugzeug
  folgt in etwa 0,2 s.
- **Ruderwirkung:** Die Ruder wirken voll ab 544 km/h auf Meereshöhe, in 6000 m
  ab 740 km/h. Darunter wirken sie schwächer, bis hinab auf 10 %.
- **g-Begrenzer:** höchstens +7 g und −2,5 g. Einen **Anstellwinkelbegrenzer
  gibt es bewusst nicht**: Wer zu hart zieht, reißt ab.
- **Kurven** sind koordiniert, ein Gierdämpfer hält den Schiebewinkel klein.
  Rollen allein zieht keine g-Last. Querlage ohne Ziehen ergibt deshalb eine
  Kurve, in der das Flugzeug sinkt.
- **Unter 144 km/h** sind g-Begrenzer und Kurvenkoordination aus.
- **Tastatur:** Eine Taste lenkt den Knüppel in einer Drittelsekunde voll aus.
  Beim Loslassen steht er in einer Sechstelsekunde wieder in der Mitte. `+` und
  `-` bewegen den Schub um 40 % je Sekunde; die Ziffern setzen ihn direkt.

## Leistung

Volle Tanks, ohne Waffen, ohne Klappen und Fahrwerk:

| Höhe | Abrissfahrt | mit Klappen | Höchstfahrt | bestes Steigen |
|---|---|---|---|---|
| 0 m | 231 km/h | 200 km/h | 1170 km/h (Mach 0,95) | 62 m/s bei 690 km/h |
| 3000 m | 268 km/h | 232 km/h | 1210 km/h (Mach 1,02) | 50 m/s bei 730 km/h |
| 6000 m | 315 km/h | 273 km/h | 1250 km/h (Mach 1,10) | 38 m/s bei 770 km/h |
| 9000 m | 375 km/h | 324 km/h | 1290 km/h (Mach 1,18) | 27 m/s bei 820 km/h |
| 12 000 m | 459 km/h | 397 km/h | 1310 km/h (Mach 1,24) | 16 m/s bei 900 km/h |

Die Gipfelhöhe liegt bei etwa 15 700 m. Mit fast leeren Tanks liegt die
Abrissfahrt rund 12 % tiefer; Waffen machen alles etwas schlechter.

**Kurven:**

- **Höchste Last:** 7 g erreicht das Flugzeug erst ab etwa 610 km/h
  (Meereshöhe, volle Tanks), in der Höhe entsprechend später.
- **Kurvenrate am g-Begrenzer:** 23°/s bei 600 km/h (Radius etwa 410 m),
  17,5°/s bei 800 km/h (etwa 730 m).
- **Fahrtverlust:** So eine Kurve kostet viel Fahrt. 8 s am g-Begrenzer machen
  aus 833 km/h noch 693 km/h, mit Klappen 715 km/h.
- **Ohne Fahrtverlust** hält das Flugzeug bei Vollgas auf Meereshöhe etwa 4 g
  (bei 800 km/h 10,7°/s), in 3000 m noch 3,4 g.

## Strömungsabriss

Die Strömung reißt ab, wenn eine von zwei Bedingungen eintritt:

- **Die Fahrt liegt unter der Abrissfahrt bei der Last, die der Knüppel
  fordert.**
  - Die Abrissfahrt steigt mit der Wurzel der Last, mit der Höhe und mit der
    Masse. Klappen senken nur ihren Grundanteil.
  - Voller Zug fordert bis zu 2,8 g zusätzlich, in steiler Querlage voll.
    Mit waagerechten Flächen zählen davon nur 40 %.
  - Voll gezogen in steiler Kurve reißt die Strömung deshalb unter etwa
    450 km/h ab, beim Hochziehen mit waagerechten Flächen erst unter etwa
    340 km/h (Meereshöhe, volle Tanks).
- **Der Anstellwinkel steigt über 18°.**

Wie schnell das geht, zeigen Messungen in 3000 m mit 60 % Treibstoff: Bei
343 km/h voll gezogen reißt die Strömung in der 70°-Kurve nach 1,6 s ab (mit
Klappen nach 1,9 s), waagerecht hochgezogen nach 8,6 s.

**Was dann passiert:** Die Stärke des Abrisses baut sich in Bruchteilen einer
Sekunde auf und wächst mit dem Fahrtdefizit. Bei vollem Abriss:

- bleiben 18 % des Auftriebs und 10 % der Ruderwirkung;
- kippt die Nase aus jeder Lage Richtung senkrecht nach unten, die Flugbahn
  folgt ihr. Rollen bleibt möglich.

**Er rastet ein:** Ein merklicher Abriss bleibt mindestens halb so stark, bis
die Fahrt wieder 1,2-mal die Abrissfahrt erreicht (auf Meereshöhe etwa
280 km/h). Erst danach klingt er ab. Das kostet viel Höhe, auch wenn man sofort
loslässt:

| Ausgangslage (3000 m, 60 % Treibstoff) | Höhenverlust | mit Klappen |
|---|---|---|
| waagerecht bis zum Abriss verzögert | etwa 1500 m | etwa 1300 m |
| Abriss aus 40°-Steigflug | etwa 1600 m | etwa 1350 m |
| 60°-Kurve mit 463 km/h voll gezogen | keiner | – |

Kurze Abrisse in schnellen Kurven bleiben also kurz.

**Abfangen:** Knüppel loslassen, die Nase fallen lassen, Fahrt aufnehmen, dann
mäßig ziehen. Wer zu früh voll zieht, bleibt im Abriss.

**Warnung:** „STALL“ mit Warnton erscheint, sobald einer dieser Fälle eintritt:
- der Abriss setzt ein;
- der Anstellwinkel liegt über 15,5°;
- die Fahrt liegt unter 110 % der Abrissfahrt bei der geforderten Last.

## Langsamflug und Durchsacken

Unter dem 1,5-Fachen der Abrissfahrt (auf Meereshöhe etwa 350 km/h) sackt das
Flugzeug durch: Bei gleicher Nicklage liegt die Flugbahn tiefer, bei
Abrissfahrt um bis zu 14°.

- **Folge:** Wer die Nase hält, sinkt stetig. Wer die Höhe halten will, nimmt
  die Nase höher und braucht mehr Schub.
- **Beispiel:** 30 s bei 1,2-facher Abrissfahrt mit 50 % Schub und gehaltener
  Nase kosten etwa 100 m. Mit Klappen hält das Flugzeug die Höhe.
- **Bodennähe:** Unter 15 m Radhöhe verschwindet das Durchsacken
  (Bodeneffekt). Abheben und Abfangen bleiben dadurch unberührt.

## Klappen, Fahrwerk, Luftbremse

- **Klappen (F):** 2 s Fahrzeit, bei jeder Fahrt nutzbar.
  - Sie senken die Abrissfahrt im Geradeausflug um 14 %, unter Last nicht.
  - Sie kosten etwas Fahrt, mindern aber den Widerstand in harten Kurven.
  - Sie machen Abriss und Durchsacken um 30 % milder und halbieren die
    Turbulenz.
  - Für Start, Langsamflug und Landung sind sie das wichtigste Hilfsmittel.
- **Fahrwerk (G):** 5 s Fahrzeit, am Boden nicht einfahrbar.
  - Ausgefahren verdreifacht es fast den Grundwiderstand.
  - In der Luft rüttelt es immer leicht.
  - Über 463 km/h rüttelt es stärker, und „GEAR SPEED“ erscheint.
- **Luftbremse (B):** in 0,8 s ausgefahren, vervierfacht etwa den
  Grundwiderstand. Am Boden wirkt dieselbe Taste als Radbremse.

## Treibstoff

- **Verbrauch:** 0,35 kg/s im Leerlauf, 4,3 kg/s bei Vollgas auf Meereshöhe.
  - Dazwischen wächst er mit dem Quadrat der Schubstellung; halber Schub kostet
    also nur knapp ein Drittel des Vollgas-Verbrauchs.
  - In der Höhe sinkt er wie der Schub, bei Vollgas in 3000 m auf 3,5 kg/s und
    in 9000 m auf 2,3 kg/s.
- **Reichweite:** Vollgas auf Meereshöhe leert den Tank in etwa 20 Minuten,
  Leerlauf erst nach über vier Stunden. Für die Planung rechnet das Spiel
  11 kg je km (720 km/h in 3000 m). Das ergibt gut 450 km je Tankfüllung.
- **Leerer Tank:** „FLAMEOUT“. Das Triebwerk geht aus und springt nicht wieder
  an, das Flugzeug segelt.
- **Warnungen:**
  - „FUEL LOW“ unter 15 % (780 kg).
  - „BINGO“, sobald der Rest nicht mehr bis zur Heimatbasis plus 450 kg Reserve
    reicht. Gerechnet wird mit der jetzigen Fahrt, mindestens 540 km/h, und
    mindestens halbem Schub.
- **Zusatztank:** Er sitzt in einem Schacht. Beim Auslösen pumpt er seine
  1000 kg in die Haupttanks, so weit sie passen, und fällt ab.
- **Leck:** Ein Treffer im Tanksystem kann ein Leck von bis zu 3 kg/s
  verursachen.

## Am Boden

- **Rollen:** Am Boden gibt es keine Querlage.
  - Gelenkt wird mit dem Knüppel (Rollen) und dem Seitenruder, auch im Stand:
    im Stand mit 34°/s, bei 54 km/h noch mit der Hälfte.
  - Die Radbremse (B) verzögert mit bis zu 5 m/s².
  - Wer schneller als 43 km/h die befestigte Fläche verlässt, verunglückt
    („OFF RUNWAY“).
- **Start:**
  - Die Nase hebt sich erst ab dem 1,1-Fachen der Abrissfahrt. Dann gibt der
    Knüppel die Nicklage vor, voll gezogen 12°.
  - Das Flugzeug hebt ab, sobald der Auftrieb es trägt.
  - Mit vollen Tanks und üblicher Beladung braucht es mit Klappen etwa 630 m
    Bahn (18,5 s, Abheben bei 252 km/h), ohne Klappen 910 m (22 s, 300 km/h).
- **Nach dem Abheben:** Wer voll gezogen hält, reißt in etwa 90 m Höhe ab. Mit
  Klappen bleibt es ohne Abriss, das Flugzeug kommt aber nur auf etwa 190 m.
  Wer bei 15° Nicklage nachlässt, ist nach einer Minute etwa 690 m hoch.
- **Landung:** Bewertet wird die erste Bodenberührung:

  | Lage beim Aufsetzen | Ergebnis |
  |---|---|
  | abseits einer Bahn oder im Wasser | Absturz |
  | Fahrwerk nicht verriegelt | GEAR UP LANDING (Absturz) |
  | Sinkrate über 7 m/s oder Nase unter −5° | GEAR COLLAPSED (Absturz) |
  | Querlage über 15° | WING STRIKE (Absturz) |
  | Sinkrate über 4 m/s | HARD LANDING |
  | sonst | GOOD LANDING |

  Die Fahrt zählt nicht. Wer auf einer Bahn zum Stehen kommt, beendet den
  Einsatz. Aus 240 km/h steht das Flugzeug mit Bremse nach etwa 380 m.
- **Flugverhalten** (Menü): „Realistisch“ nutzt die Grenzen oben. „Leicht“
  lockert alle auf das Anderthalbfache. Bei „Kein Absturz“ geht auf der Bahn
  nichts zu Bruch, auch ohne Fahrwerk, und die Turbulenz in Bodennähe
  entfällt. Bodenberührung abseits einer Bahn bleibt in jeder Stufe ein Absturz.

## Turbulenz

- **Bodennähe:** Unter 300 m über Grund wackelt das Flugzeug, umso stärker, je
  tiefer und je schneller es fliegt.
  - Um die Längsachse wackelt es voll, um die Querachse halb so stark.
  - Klappen halbieren es, am Boden ruht es.
  - Der Tiefstflug unter den Radaren bleibt so anspruchsvoll. Wer langsamer
    fliegt oder die Klappen setzt, wackelt weniger.
- **Rütteln** kommt dazu:
  - mit ausgefahrenem Fahrwerk, über dessen Höchstfahrt stärker;
  - je Treffer, bis zum sechsten.

## Schäden

Treffer verschlechtern Bordsysteme (eigenes Kapitel folgt). Auf das Fliegen
wirken vier davon:
- **Triebwerk:** Es begrenzt den Höchstschub.
- **Tanksystem:** Es leckt.
- **Hydraulik:** Klappen und Fahrwerk laufen langsamer, ausgefallen gar nicht.
- **Flugsteuerung:** Sie verliert Drehrate, ausgefallen bleiben 30 %.

## Anzeigen

- **HUD:**
  - links die Fahrt (km/h) mit „G“ (Last), „M“ (Machzahl) und „A“ (Anstellwinkel
    in Grad), rechts die Höhe (m); dazu der Flugbahnmarker.
  - Warnungen: STALL, PULL UP (Bodenaufschlag in weniger als 6 s), OVER G (über
    6,65 g), GEAR SPEED, FUEL LOW, BINGO, FLAMEOUT.
  - Schalterstellungen: GEAR, FLAPS, AIRBRAKE, BRAKES.
- **ENG-Seite:** Drehzahl (Leerlauf 62 %, Vollgas 100 %), Schubhebel, Schub,
  Verbrauch und Treibstoffbalken mit BINGO-Marke.
- **ADI-Seite:** Reserve-Horizont mit Fahrt, Höhe, Kurs, Steigrate und g.

## Kennwerte nachmessen

Die Werte der Tabellen oben gelten für die Stellschrauben unten. Wer an ihnen
dreht, misst nach:

- `cargo run --release -p f29-core --example flight_report` gibt Abriss,
  Durchsacken, Kurven und Start als Tabelle aus.
- `cargo test -p f29-core --test envelope` prüft, ob sie in ihren erlaubten
  Bändern liegen.

Die Leistungstabelle (Höchstfahrt, Steigen, Gipfelhöhe, Kurvenraten) ist aus
denselben Werten berechnet, ohne Waffen.

## Stellschrauben

| Datei | Name | Wert | Wirkung |
|---|---|---|---|
| `f29-data/src/aircraft.rs` | `empty_mass_kg`, `fuel_capacity_kg` | 16 300 kg, 5200 kg | Masse: Abrissfahrt, Trägheit, Steigen; Treibstoffvorrat |
| `f29-data/src/aircraft.rs` | `wing_area_m2` | 70 m² | Flügelfläche: Abrissfahrt und Auftrieb je Fahrt |
| `f29-data/src/aircraft.rs` | `thrust_max_n`, `thrust_density_exponent` | 110 kN, 0,75 | Höchstschub und wie schnell er mit der Höhe fällt; bestimmt Höchstfahrt, Steigen, Dauerkurve |
| `f29-data/src/aircraft.rs` | `engine_spool_time_s`, `idle_ram_drag` | 1,2 s, 200 N je m/s | Verzögerung des Triebwerks; wie stark Gas wegnehmen bremst |
| `f29-data/src/aircraft.rs` | `fuel_flow_idle_kgps`, `fuel_flow_max_kgps` | 0,35 kg/s, 4,3 kg/s | Verbrauch im Leerlauf und bei Vollgas (Meereshöhe) |
| `f29-data/src/aircraft.rs` | `cl0`, `cl_alpha`, `alpha_stall_rad` | 0,04, 3,6, 0,32 rad (18°) | Auftrieb über dem Anstellwinkel, Abrisswinkel; zusammen der größte Auftrieb und damit die Abrissfahrt |
| `f29-data/src/aircraft.rs` | `post_stall_lift_drop` | 8 | wie schnell der Auftrieb jenseits des Abrisswinkels fällt |
| `f29-data/src/aircraft.rs` | `cd0` | 0,024 | Grundwiderstand: Höchstfahrt |
| `f29-data/src/aircraft.rs` | `induced_drag_k` | 0,15 | Widerstand durch Auftrieb: Fahrtverlust in Kurven, Steigen |
| `f29-data/src/aircraft.rs` | `airbrake_cd`, `gear_cd` | 0,07, 0,045 | Widerstand von Luftbremse und Fahrwerk |
| `f29-data/src/aircraft.rs` | `cy_beta`, `yaw_damper_gain` | 0,65, 2,5 | Seitenkraft beim Schieben, Gierdämpfer |
| `f29-data/src/aircraft.rs` | `flap_cl`, `flap_cd`, `flap_induced_factor`, `flap_stall_relief`, `flap_rate` | 0,40, 0,016, 0,6, 0,3, 0,5 je s | Klappen: Abrissfahrt, Fahrtverlust geradeaus und in Kurven, Milderung von Abriss und Durchsacken, Fahrzeit |
| `f29-data/src/aircraft.rs` | `sink_angle_max_rad`, `sink_onset_factor` | 0,25 rad (14°), 1,5 | Durchsacken: größter Winkel und ab welcher Fahrt es einsetzt |
| `f29-data/src/aircraft.rs` | `max_roll_rate`, `max_pitch_rate`, `max_yaw_rate` | 2,6, 0,45, 0,20 rad/s | höchste Drehraten |
| `f29-data/src/aircraft.rs` | `rate_time_constant_s` | 0,18 s | wie schnell das Flugzeug dem Knüppel folgt |
| `f29-data/src/aircraft.rs` | `control_q_ref_pa` | 14 000 Pa | Staudruck, ab dem die Ruder voll wirken (544 km/h auf Meereshöhe) |
| `f29-data/src/aircraft.rs` | `g_limit_pos`, `g_limit_neg` | 7, 2,5 | g-Begrenzer |
| `f29-data/src/aircraft.rs` | `alpha_warning_rad` | 0,27 rad (15,5°) | Anstellwinkel der Abrisswarnung |
| `f29-data/src/aircraft.rs` | `stall_stick_g`, `stall_stick_level_factor` | 2,8 g, 0,4 | Last, die voller Zug für die Abrissfahrt fordert; Anteil mit waagerechten Flächen – wie leicht Kurven und Hochziehen abreißen |
| `f29-data/src/aircraft.rs` | `stall_deficit_full`, `stall_time_constant_s` | 0,18, 0,3 s | bei welchem Fahrtdefizit der Abriss voll ist, wie schnell er sich aufbaut |
| `f29-data/src/aircraft.rs` | `stall_recovery_factor`, `stall_latch_strength`, `stall_recovery_time_s` | 1,2, 0,5, 1,5 s | Einrasten: bis zu welcher Fahrt, wie stark; Abklingzeit danach – bestimmt den Höhenverlust |
| `f29-data/src/aircraft.rs` | `stall_pitch_break`, `stall_lift_factor` | 1,9 rad/s, 0,18 | Abkippen der Nase, verbleibender Auftrieb bei vollem Abriss |
| `f29-data/src/aircraft.rs` | `gear_time_s`, `gear_max_speed_mps` | 5 s, 128,6 m/s (463 km/h) | Fahrzeit und Höchstfahrt des Fahrwerks |
| `f29-data/src/aircraft.rs` | `rolling_friction`, `wheel_brake_decel`, `ground_steer_rate` | 0,03, 5 m/s², 0,6 rad/s | Rollreibung, Radbremse, Lenken im Stand |
| `f29-data/src/aircraft.rs` | `hard_landing_sink_mps`, `max_landing_sink_mps`, `max_landing_bank_rad` | 4 m/s, 7 m/s, 0,26 rad (15°) | Grenzen für harte Landung, Fahrwerksbruch, Flächenberührung |
| `f29-core/src/flight.rs` | `STALL_CONTROL_RETAINED`, `STALL_EXTRA_DRAG`, `STALL_PATH_ALIGN` | 0,1, 0,08, 1,6 je s | Ruderwirkung, Zusatzwiderstand und Bahndrehung zur Nase bei vollem Abriss |
| `f29-core/src/flight.rs` | `STALL_ALPHA_FULL`, `STALL_DIVE_CONE` | 0,2 rad, 0,2 rad | Abriss voll bei so viel über dem Abrisswinkel; ab diesem Winkel zur Senkrechten kippt die Nase nicht weiter |
| `f29-core/src/flight.rs` | `AIRBRAKE_RATE` | 1,25 je s | Ausfahrzeit der Luftbremse |
| `f29-core/src/flight.rs` | `ROTATE_FACTOR`, `TAKEOFF_PITCH`, `TAKEOFF_PITCH_RATE` | 1,1, 0,209 rad (12°), 0,12 rad/s | Abhebefahrt, Nicklage und Nickrate beim Rotieren |
| `f29-core/src/flight.rs` | `STEER_HALF_SPEED`, `NOSE_SETTLE_RATE` | 15 m/s, 0,15 rad/s | Lenken bei Fahrt, Absenken der Nase am Boden |
| `f29-core/src/flight.rs` | `SINK_GROUND_EFFECT_M`, `MIN_FBW_SPEED` | 15 m, 40 m/s | Radhöhe, unter der das Durchsacken ausläuft; Fahrt, unter der g-Begrenzer und Koordination aus sind |
| `f29-core/src/sim/mod.rs` | `STALL_WARNING_MARGIN` | 1,1 | Abrisswarnung unter diesem Vielfachen der Abrissfahrt |
| `f29-core/src/sim/mod.rs` | `TURBULENCE_CEILING_M`, `TURBULENCE_ROLL`, `TURBULENCE_TIME_CONSTANT_S` | 304,8 m, 1,05 rad/s, 0,15 s | Turbulenz: ab welcher Höhe, wie stark (bei 720 km/h direkt über Grund), wie geglättet |
| `f29-core/src/sim/mod.rs` | `GEAR_SHAKE`, `GEAR_BUFFET`, `DAMAGE_SHAKE`, `DAMAGE_SHAKE_HITS` | 0,22, 0,5, 0,07 rad/s, 6 | Rütteln durch Fahrwerk, Überfahrt des Fahrwerks und Treffer |
| `f29-core/src/sim/mod.rs` | `OFF_RUNWAY_MAX_SPEED`, `NOSE_FIRST_PITCH` | 12 m/s (43 km/h), −0,087 rad (−5°) | Abkommen von der Bahn; Bugrad zuerst |
| `f29-core/src/sim/mod.rs` | `FUEL_RESERVE_KG`, `CRUISE_FUEL_KG_PER_KM`, `FUEL_LOW_FRACTION` | 450 kg, 11 kg/km, 0,15 | Reserve für BINGO, Planungsverbrauch, FUEL LOW |
| `f29-core/src/rules.rs` | `Landings::tolerance` | kein Absturz –, leicht 1,5, realistisch 1 | Faktor auf die Landegrenzen je Flugverhalten |
| `f29-core/src/systems.rs` | `FUEL_LEAK_MAX_KGPS`, `CONTROL_BACKUP` | 3 kg/s, 0,3 | größtes Leck; Ruderwirkung bei ausgefallener Flugsteuerung |
| `f29-app/src/input.rs` | `KEY_RAMP`, `KEY_RETURN`, `THROTTLE_RATE` | 3 je s, 6 je s, 0,4 je s | Tastatur: Auslenken und Zurückführen des Knüppels, Schubverstellung |
