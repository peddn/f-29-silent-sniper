# Spielmechaniken & Clean-Room-Protokoll

F-29 Silent Sniper ist eine **eigenständige Neuentwicklung**, die sich spielerisch an
*F-19 Stealth Fighter* (MicroProse, 1988) orientiert. Dieses Dokument legt fest, wie wir
mit dem Original und seiner Rekonstruktion umgehen, und protokolliert jede Mechanik, die
wir nachbauen.

## 1. Grundsatz

Spielmechanik, also *wie sich etwas verhält*, ist eine Idee und darf nachgebaut werden.
Code, Daten, Grafiken, Texte, Klänge und Namen des Originals sind Ausdruck und werden
**nicht** übernommen.

| Darf übernommen werden | Darf **nicht** übernommen werden |
|---|---|
| Beobachtbares Verhalten („harte Ziele brauchen Spezialmunition“) | Quellcode, auch nicht abgewandelt, transliteriert oder „nur die Struktur“ |
| Allgemeine Regelstruktur („Wirkung hängt von Waffe und Zielklasse ab“) | Konkrete Zahlenwerte, Tabellen, Schwellwerte, Formeln mit Original-Konstanten |
| Genre-Konventionen (HUD-Symbolik, Karriere mit Orden) | Grafiken, Modelle, Schriften, Paletten, Sounds, Musik, Texte, Kartendaten |
| Öffentliche Bezeichnungen realer Systeme (z. B. AGM-65 Maverick, Mk 82) | Namen: Spiel, Flugzeug, Szenarien, Figuren, Orden des Originals |

## 2. Referenz

Einzige Lesereferenz für das Verhalten des Originals ist das Rekonstruktionsprojekt
[`xor2003/f19re`](https://github.com/xor2003/f19re). Es baut die Original-EXE als
byte-identisches Assembler-Skelett nach und portiert Routinen schrittweise zurück nach C.

> **Wichtig:** Die MIT-Lizenz von f19re deckt die Werkzeuge und die Arbeit des
> Rekonstruktionsprojekts ab. Der rekonstruierte Programmcode selbst bleibt inhaltlich
> der Code des Originals. Für uns ist f19re deshalb **nur zum Lesen** da, als Quelle
> für Verhaltensbeschreibungen, nie als Quelle für Code oder Werte.

Original-Spieldateien (EXE, `*.3D3`, `*.PIC`, `*.SPR` usw.) gelangen **niemals** in
dieses Repository, auch nicht in Tests, Werkzeuge oder Issues.

## 3. Arbeitsablauf (Zwei-Phasen-Regel)

1. **Lesephase:** Wer f19re liest, schreibt einen Eintrag in Abschnitt 5. Darin steht
   *in eigenen Worten*, welches Verhalten beobachtet wurde, außerdem die Referenzstelle
   (Datei/Routine), damit die Herkunft der Idee nachvollziehbar bleibt. Es werden weder
   Code noch Zahlen abgeschrieben.
2. **Entwurfsphase:** Das eigene Design und die eigenen Werte entstehen **nur aus dem
   Eintrag**, ohne geöffnete Referenz. Die Begründung der Werte steht im Eintrag
   (Physik, öffentliche Daten realer Systeme, Spielbalance).
3. **Review:** Pull Requests, die eine Mechanik betreffen, verweisen auf ihren Eintrag.
   Reviewer prüfen, dass keine Werte oder Strukturen aus der Referenz durchgesickert sind.

Ziel ist, dass die Implementierung auch dann genauso aussähe, wenn nur die
Verhaltensbeschreibung bekannt gewesen wäre.

## 4. Woher unsere Werte stammen

- **Physik:** ISA-Atmosphäre, Aerodynamik-Grundgleichungen, Ballistik.
- **Öffentliche Größenordnungen** realer Flugzeuge und Systeme (Masse, Schub,
  Flügelfläche, Radarreichweiten aus Fachliteratur), angepasst an unser fiktives
  Flugzeug.
- **Spielbalance:** Werte werden über Headless-Flugtests und Testspiele abgestimmt.
  Die Tests in `crates/f29-core/tests/` dokumentieren das gewünschte Verhalten.
- **Einheiten:** Der Kern rechnet in SI. Alle Anzeigen (HUD, MFDs, Karte,
  Headless-Ausgaben) sind seit v0.2.7 metrisch: Fahrt in km/h, Höhe und Radarhöhe
  in m, Steig-/Sinkrate in m/s, Entfernungen in km, Masse und Verbrauch in kg bzw.
  kg/min (Spielerentscheidung). Ältere Einträge unten nennen teils noch kt/ft;
  das sind historische Angaben.

## 5. Mechanik-Protokoll

Format jedes Eintrags:

- **Verhalten:** eigene Beschreibung
- **Referenzstelle:** Datei/Routine in f19re, oder „keine“
- **Unser Design:** wie wir es umsetzen
- **Unsere Werte:** konkrete Zahlen + Begründung
- **Status:** geplant / umgesetzt (Version)

### M-001 Flugmodell

- **Verhalten:** Das Flugzeug fliegt gutmütig und arcadenah, verliert in engen Kurven
  Energie, reißt bei zu hohem Anstellwinkel ab und kann ins Gelände stürzen.
- **Referenzstelle:** keine. Das Flugmodell ist von Grund auf eigenständig entworfen.
- **Unser Design:** Translation als 3D-Kräftebilanz (Auftrieb über CL(α),
  Widerstand CD0 + k·CL², Seitenkraft, Schub abhängig von der Luftdichte).
  Rotation über eine Fly-by-Wire-Ratenregelung mit g-Begrenzer,
  Turn-Koordination und Gierdämpfer, aber **ohne** Anstellwinkelbegrenzer
  (siehe M-004). Details in `crates/f29-core/src/flight.rs`.
- **Unsere Werte:** `crates/f29-data/src/aircraft.rs` (`F29_SPEC`). Die Werte sind in
  der Größenordnung eines schweren zweistrahligen Angriffsflugzeugs ohne Nachbrenner
  gewählt und so abgestimmt, dass folgende Ziele erreicht werden:
  - Abrissgeschwindigkeit im Horizontalflug ≈ 64 m/s (≈ 125 kt) auf Meereshöhe, volle Tanks
  - Schub/Gewicht ≈ 0,38: begrenztes Steigvermögen, ein steiler Steigflug endet im Abriss
  - hoher induzierter Widerstand: Eine harte Kurve kostet in 10 s mehr als 30 m/s Fahrt
  - Höchstgeschwindigkeit in Bodennähe ≈ Mach 0,8
  - Begrenzung auf 7 g, die in Bodennähe erst oberhalb von ≈ 340 kt erreichbar sind; darunter reißt
    die Strömung vorher ab
  - Rollrate ≈ 150°/s

  *v0.1.1:* Nach dem ersten Spieltest wurden Schub, Flügelfläche und induzierter
  Widerstand angepasst, weil Kurven zu wenig Energie kosteten und der Abriss nicht
  spürbar war.

  *v0.1.4:* Laut Spieltest verlor das Flugzeug im Geradeausflug langsam Höhe, und
  der Schub wirkte kaum. Deshalb gibt es jetzt:
  - **Bahnhaltung:** Bei neutralem Knüppel und kleiner Querlage hält die
    Flugsteuerung den Bahnwinkel, den das Flugzeug beim Loslassen hat. Kurven mit
    Querlage sinken weiterhin, wenn man nicht zieht.

  *v0.1.5:* Die Bahnhaltung wurde vollständig entfernt (Spielerwunsch). Der
  beobachtete Höhenverlust war korrektes Verhalten: Die Nase lag auf dem
  Horizont, für den Horizontalflug muss sie aber um den Anstellwinkel darüber
  stehen. Maßgeblich für die Flugbahn ist der Flight Path Marker im HUD. Ein
  losgelassener Knüppel hält wieder die Lage der Nase.
  - **Schnelleres Triebwerk und Staudruckwiderstand im Leerlauf:** Gas wegnehmen
    bremst deutlich, und der Maximalschub ist etwas höher. In 10 s ergeben sich
    etwa +13 m/s bei Vollgas und −30 m/s im Leerlauf.

  Die Flugtests prüfen genau diese Eigenschaften.

  *v0.1.8:* Laut Spieltest kosteten harte Kurven etwas zu viel Fahrt, und der
  Abriss kam etwas zu früh. Deshalb: induzierter Widerstand −25 %, Abrisswinkel
  17°→18°, Abriss über den Anstellwinkel setzt allmählicher ein, voller Zug
  fordert 4,5 statt 5 g an. Unterhalb der Eckgeschwindigkeit (≈ 370 kt) reißt
  voller Zug ohne Klappen weiterhin ab – das Flugzeug hat bewusst keinen
  Anstellwinkelbegrenzer.

  *v0.1.6:* Die Masse setzt sich jetzt aus Leermasse und Treibstoff zusammen
  (M-009). Die oben genannten Werte gelten für volle Tanks; gegen Ende eines
  Flugs ist das Flugzeug leichter, steigt besser und reißt später ab.

  *v0.1.18 (Spieltest: mehr Beschleunigung):* Schub 92 → 110 kN, Schub/Gewicht
  bei vollen Tanks ≈ 0,52. Von 200 auf 350 kt in 3000 m dauert es etwa 24 statt
  31 s. Harte Kurven kosten etwas weniger Fahrt (nach 8 s am g-Begrenzer ab
  450 kt ≈ 374 statt 361 kt).
- **Status:** umgesetzt (v0.1)

### M-004 Strömungsabriss

- **Verhalten:** Aggressives Fliegen ist gefährlich. Die Abrissgeschwindigkeit
  steigt mit der Flughöhe und mit der g-Last. Fällt die Fahrt darunter, wird die
  Nase fortlaufend nach unten gedrückt, umso stärker, je größer das Defizit ist. Das
  Flugzeug sackt durch und fängt sich erst wieder, wenn es Fahrt aufgenommen hat.
  Ziehen kostet deutlich Fahrt, und auch Steigen kostet Fahrt.
- **Referenzstelle:** f19re `src/egflight.c`, Flugzustands-Aktualisierung des
  Spielerflugzeugs (Berechnung von Eck- und Abrissgeschwindigkeit sowie das
  Nase-runter-Drücken unterhalb der Abrissgeschwindigkeit).
  Spielerbeobachtung (erster Spieltest): „Im Original sackt das Flugzeug aggressiv
  senkrecht nach unten ab, wenn man aggressiv fliegt.“
- **Detailanalyse der Referenz** (Lesephase zu v0.1.3, eigene Worte, keine Werte):
  Die Flugroutine `stepFlightModel` (`src/egflight.c`) ist kein Kräftemodell,
  sondern ein Fahrt-und-Lage-Modell.
  1. *Fahrt* ist eine einzelne Zahl. Sie läuft mit Verzögerung einer Zielfahrt
     hinterher. Die Zielfahrt steigt mit dem Schub und sinkt mit der Nicklage:
     Nase hoch kostet Fahrt, Nase runter bringt Fahrt. Mit der Höhe wird sie etwas
     größer, mit dem Treibstoffgewicht kleiner, ausgefahrenes Fahrwerk und
     Landeklappen bremsen. Vor allem sinkt sie **linear mit der g-Last** und
     erreicht bei maximaler g-Last null. Harte Kurven fressen deshalb rasch Fahrt.
  2. *g-Last* ergibt sich aus der **Querlage** (Tabelle: Lastvielfaches einer
     horizontalen Kurve bei dieser Querlage) plus Ziehen. Wer nur in die Kurve
     rollt, zieht also automatisch die passende g-Last und verliert Fahrt. Ziehen
     wird an einer Obergrenze gekappt, Drücken wirkt nur halb so stark.
  3. *Abrissgeschwindigkeit* ist eine Grundgröße, die mit der Höhe und mit der
     **Wurzel der g-Last** wächst (beschleunigter Abriss). Landeklappen senken sie.
  4. *Durchsacken:* Die Flugbahn liegt um einen „Durchhängewinkel“ unter der
     Nase. Er wächst mit dem Verhältnis Abriss- zu aktueller Fahrt (begrenzt) und
     mit dem Kosinus der Querlage. Steigrate = Fahrt × sin(Nicklage −
     Durchhängewinkel). Je langsamer, desto mehr sinkt das Flugzeug bei gleicher
     Nase; nahe und unter der Abrissgeschwindigkeit sackt es mit erhobener Nase
     durch.
  5. *Abriss:* Liegt die Fahrt unter der Abrissgeschwindigkeit (in der Luft),
     wird die Nicklage **in jedem Frame um einen Betrag proportional zum
     Fahrtdefizit** nach unten gedrückt. Auf dem höchsten Schwierigkeitsgrad und
     bei Beschädigung geschieht das doppelt so stark. Dazu kommt ein Warnton.
     Es gibt **keinen Abrisszustand und keine Hysterese**: Sobald die Fahrt
     wieder über der Schwelle liegt, endet das Drücken sofort und stetig.
  6. *Nicken* ist ein reines Ratenkommando, unabhängig von der Fahrt. Dass es
     sich bei wenig Fahrt schwach anfühlt, liegt am entgegenwirkenden Nickabfall
     (Punkt 5) und am Durchhängen (Punkt 4).
  7. Mit Fahrwerk unten gibt es auf leichten Schwierigkeitsgraden in Bodennähe
     eine automatische Abfanghilfe (Landehilfe). Turbulenz hängt von
     Schwierigkeitsgrad, Höhe, Fahrwerk und Schäden ab.

  Folgerung für uns: Das als „senkrecht absacken“ erlebte Verhalten entsteht aus
  dem Zusammenspiel von Punkt 4 (Bahn fällt unter die Nase) und Punkt 5 (Nase
  wird stetig nach unten gedrückt, solange Fahrt fehlt). Es ist **stetig** und
  ohne Schaltzustand. Unsere Hysterese-Lösung (v0.1.1/v0.1.2) erzeugt dagegen
  Sprünge.
- **Unser Design (v0.1.3):** Nach dem Vorbild des Originals ist der Abriss eine
  **stetige Abrissstärke 0…1** ohne Schaltzustand und ohne Hysterese, eingebettet
  in unsere Kräftephysik:
  - Abrissgeschwindigkeit `v_s = v_s1g(ρ) · √(1 + Zug · stall_stick_g)`. Die g-Last
    stammt aus dem **Knüppelkommando**, nicht aus dem erreichten Auftrieb, sonst
    würde der Auftriebsverlust den Abriss sofort wieder beenden (Pendeln). Höhe
    (Dichte) und Ziehen heben die Abrissgeschwindigkeit an.
  - Ziel-Abrissstärke = relatives Fahrtdefizit unter `v_s`, voll bei
    `stall_deficit_full`. Jenseits des Abrisswinkels wächst sie zusätzlich mit dem
    Anstellwinkel, damit knapp über `v_s` kein absurder Anstellwinkel möglich ist.
    Die Stärke folgt dem Ziel mit der Zeitkonstante `stall_time_constant_s`.
  - Mit der Abrissstärke wachsen proportional: Auftriebsverlust (bis
    `stall_lift_factor`), Zusatzwiderstand, Ruderverlust (bis
    `stall_control_factor`) und das **Abkippen der Nase Richtung senkrecht nach
    unten** aus jeder Fluglage (bis `stall_pitch_break`). Die Flugbahn folgt der
    Nase, statt flach durchzusacken.
  - Im Sturz kommt die Fahrt zurück, und die Abrissstärke sinkt stetig. Mit
    mäßigem Zug lässt sich das Flugzeug abfangen; wer zu früh voll zieht, hebt
    die Abrissgeschwindigkeit an und bleibt teilweise im Abriss.
  - Rollen allein zieht **keine** g-Last (Abweichung vom Original, bewusst so
    gewählt): Querlage ohne Ziehen ergibt eine sinkende Kurve.
  - kein Anstellwinkelbegrenzer; Warnung bei hohem Anstellwinkel, beginnendem
    Abriss oder Fahrt unter 110 % von `v_s`.
  - Der Autopilot schützt sich selbst (Energie- und Anstellwinkelschutz).

  *Verlauf:* v0.1.1 kannte nur Abriss an/aus mit Anstellwinkel-Hysterese, was zu
  einem schnellen Pendeln führte („Zurückspulen“). v0.1.2 beendete den Abriss erst
  bei ausreichender Fahrt, schaltete aber weiterhin hart. v0.1.3 ist stetig.
- **Unsere Werte:** `crates/f29-data/src/aircraft.rs`. Abgestimmt über die
  Flugtests `slow_stall_drops_nose_steeply_then_recovers_with_speed`,
  `stall_strength_grows_with_speed_deficit`, `stall_strength_changes_smoothly`,
  `hard_pull_stalls_sinks_and_recovers_when_released`, `hard_turn_bleeds_energy`,
  `steep_zoom_climb_ends_in_stall` und `stall_warning_precedes_stall`. Keine Werte aus der Referenz übernommen.
  *v0.1.4/v0.1.5:* Laut Spieltest durfte der Abriss noch aggressiver sein. Deshalb
  gibt es jetzt einen tieferen Auftriebseinbruch, schnelleres Abkippen, früher
  volle Stärke und noch schwächere Ruder. Bei 160 kt steht die Nase nach 2 s auf
  etwa −78°.

  *v0.1.6:* Die g-Anforderung aus dem Knüppel ist jetzt nach oben durch die
  Nickrate begrenzt, die die Ruder beim aktuellen Staudruck überhaupt erzeugen
  können (n = 1 + V·q/g). Vorher galt voller Knüppel auch bei Landefahrt als
  5-g-Anforderung, und schon behutsames Abfangen riss die Strömung ab. Bei
  Kampffahrt ändert sich nichts; bei 250 kt fällt die Abrissstärke unter vollem
  Zug um wenige Prozent. Voller Zug bei Anfluggeschwindigkeit reißt weiterhin ab –
  abgefangen wird mit Gefühl.

  *v0.1.9 (Spielerwunsch):* Die g-Anforderung aus dem Knüppel hängt von der
  Querlage ab: Mit waagerechten Flächen zählen nur 40 %, in steiler Kurve die
  volle Anforderung (Gewichtung über sin² der Querlage). Harte Kurven reißen
  damit schnell ab, Hochziehen und Steigflug erst, wenn die Fahrt verbraucht
  ist. Voll gezogen nach dem Start steigt das Flugzeug immer steiler und reißt
  ohne Klappen in einigen hundert Fuß ab, statt direkt über der Bahn. Langsame
  Kurven ohne Klappen reißen weiterhin ab, etwas milder als zuvor.

  *v0.1.15 – Durchsacken im Langsamflug (Spielerwunsch):* Im langsamen
  Horizontalflug hielt das Flugzeug die Höhe zu gut. Grund war die große
  Schubreserve (Horizontalflug kurz über der Abrissfahrt mit etwa 40 % Schub) und
  die lineare Auftriebskurve. Jetzt sackt es unterhalb von
  `sink_onset_factor` × 1-g-Abrissfahrt (1,5) bei gehaltener Nicklage stetig
  durch, wie im Original, wo die Flugbahn bei geringer Fahrt zunehmend unter der
  Nase liegt (gleiche Referenzstelle). Umsetzung als *Durchsackwinkel*
  (`flight::sink_angle`): Um ihn liegt die Bahn tiefer, ohne dass der Flügel mehr
  Anstellwinkel spürt. Er wächst quadratisch auf `sink_angle_max_rad` (0,15 rad)
  bei Abrissfahrt. Ein Zusatzwiderstand von Gewicht × sin(Winkel) verhindert,
  dass das Sinken in Fahrt umgesetzt wird. Wer die Höhe halten will, nimmt die
  Nase höher und gibt mehr Schub. Klappen lindern das Durchsacken um
  `flap_sink_relief` (60 %). Am Boden gibt es kein Durchsacken; bis 15 m Radhöhe
  blendet es linear ein (Bodeneffekt), damit Abheben und Abfangen unverändert
  bleiben. Im Abriss tritt es zugunsten der Abrissmechanik zurück.
  Physikalischere Varianten wurden geprüft und verworfen: Eine Rückseite der
  Leistungskurve über stark steigenden induzierten Widerstand ist bei
  gehaltener Nicklage instabil, die Fahrt bricht dann bis zum Abriss ein.
  Gemessen (1000 m, 70 % Treibstoff, Knüppel neutral, 30 s): 150 kt mit 50 %
  Schub sinkt das Flugzeug jetzt rund 240 ft, statt 150 ft zu steigen; bei 140 kt
  steigt es mit 70 % Schub nur noch leicht; ab etwa 200 kt ändert sich nichts.
  Abgesichert durch `slow_flight_sinks_steadily_and_flaps_relieve_it`.

  *v0.1.16 – gefährlicherer Abriss (Spielertest):* Der Abriss war zu
  harmlos. Die Abrissgeschwindigkeit hängt auch am gezogenen Knüppel; wer
  loslässt, beendet einen Abriss im Geradeausflug sofort. Im Original kostet
  ein Abriss dagegen viel Höhe. Neu ist eine Einrastung: Ein merklicher Abriss
  bleibt mindestens bei `stall_latch_strength` (0,5, mit Klappen milder), bis
  die Fahrt `stall_recovery_factor` × 1-g-Abrissfahrt (1,2) erreicht. Erst dann
  klingt er mit `stall_recovery_time_s` (1,5 s) ab. Die Erholung bezieht sich
  bewusst auf die 1-g-Abrissfahrt: Schnelle, kurze Abrisse in harten Kurven
  bleiben kurz. Das Abkippen ist etwas kräftiger (`stall_pitch_break` 1,9 statt
  1,7).
  Gemessen mit `examples/stall_loss`, 3000 m: Im Horizontalflug bis zum Abriss
  verzögern, dann loslassen und mit halbem Zug abfangen. Ohne Klappen gehen
  jetzt etwa 4900 ft verloren statt 1300 ft, mit Klappen etwa 2200 ft statt
  40 ft. Ein Abriss aus 40° Steigflug kostet ohne Klappen etwa 5200 ft
  (vorher 3900 ft), mit Klappen 3500 ft (vorher 340 ft). Wer nach dem Start
  voll gezogen hält, kann jetzt auch mit Klappen abstürzen.

  *v0.1.17 – Vereinfachung:* Das Abrissmodell war über viele Versionen
  gewachsen; jede Abstimmung brauchte mehrere Runden. Jetzt gilt:
  - Zwei Auslöser: Fahrt unter der Abrissfahrt bei der aus dem Knüppel
    angeforderten g-Last (bleibt, setzt die Spielerwünsche „harte Kurven
    reißen schnell ab, Hochziehen erst bei verbrauchter Fahrt“ um), oder
    Anstellwinkel über dem Abrisswinkel. Einrastung und Erholung wie in v0.1.16.
  - Klappen wirken auf den Abriss nur noch über einen Regler,
    `flap_stall_relief` (0,45). Er mildert Einrastung, Abkippen und
    Durchsacken. Entfallen sind die Klappenwirkung auf Auftriebseinbruch,
    Ruderverlust und Abriss-Einsatz (`flap_stall_softening` an sechs Stellen),
    die eigene Durchsacklinderung `flap_sink_relief` und die Ruderhilfe
    `flap_control_q_factor`. Der Ruderverlust im Abriss ist eine Konstante.
  - Verhaltenstests prüfen Kennwerte gegen Toleranzbänder
    (`f29_core::envelope`, `tests/envelope.rs`) statt einzelner Flugbahnen.
    Die Übersicht liefert `cargo run --release -p f29-core --example
    flight_report`; es ersetzt die Beispiele `stall_loss`, `stall_trace`,
    `turn_trace`, `slow_trace` und `bank_stall`.
  Folgen gegenüber v0.1.16: Mit Klappen kostet ein Abriss etwas mehr Höhe
  (horizontal ≈ 2800 statt 2200 ft, aus dem Steigflug ≈ 4300 statt 3500 ft).
  Ohne Klappen bleibt alles gleich.

  *v0.1.18 (Spieltest):* Ohne Klappen riss das Flugzeug in Kurven zu schnell
  ab, mit Klappen praktisch nie. Grund: Die Klappen senkten die Abrissfahrt
  auch unter Last um 14 %, genau die Spanne, die harte Kurven knapp unter
  die Abrissgrenze schob. Jetzt senken die Klappen nur den 1-g-Anteil:
  v² = Vs(Klappen)² + (n − 1)·Vs(ohne)² (`flight::stall_speed`). Außerdem:
  `stall_stick_g` 3,5 → 2,8 (späterer Kurvenabriss bei Kampffahrt),
  `flap_stall_relief` 0,45 → 0,3, Schub 92 → 110 kN (siehe M-001) und zum
  Ausgleich `sink_angle_max_rad` 0,15 → 0,25.
  Gemessen (`flight_report`, 3000 m): 70°-Kurve bei 250 kt voll gezogen,
  ohne Klappen Abriss erst nach etwa 7 s statt nach 1 s. Bei 185 kt
  unverändert nach 1,6 s, mit Klappen jetzt nach 1,9 s (vorher nie, Nase
  dann auf etwa −30°, gut 600 ft Verlust). Abriss aus dem Horizontalflug mit
  Klappen ≈ 4200 ft, ohne ≈ 4900 ft.
- **Status:** umgesetzt (v0.1.4, Durchsacken v0.1.15, Einrastung v0.1.16,
  vereinfacht v0.1.17, Kurvenabriss v0.1.18)

### M-009 Treibstoff

- **Verhalten:** Der Vorrat sinkt mit der Zeit. Der Verbrauch wächst mit dem
  Quadrat der Schubstellung, Vollgas ist also überproportional teuer. Ein
  Zusatztank lässt sich in die Haupttanks umpumpen. Volle Tanks machen das
  Flugzeug etwas langsamer, leere etwas schneller. Bei leerem Tank fällt der Schub
  weg, und man segelt; die Nachbesprechung nennt leere Tanks als Absturzursache.
  Eine Kontrollanzeige warnt, sobald der Rest nicht mehr bis zur Heimatbasis
  reicht, und die Karte zeigt eine Vorschau des Verbrauchs je Wegabschnitt.
- **Referenzstelle:** f19re `src/egflight.c` (`stepFlightModel`: Verbrauch, Schub
  bei leerem Tank, Fahrteinfluss), `src/egtacmap.c` (Warnanzeige,
  Verbrauchsvorschau), `src/egcombat.c` (Umpumpen des Zusatztanks),
  `src_end/enbrief.c` (Absturzursache).
- **Unser Design:**
  - Treibstoff ist **echte Masse** (Leermasse + Treibstoff). Abrissfahrt,
    Steigleistung und Kurven ändern sich dadurch physikalisch, ohne pauschalen
    Fahrtfaktor.
  - Verbrauch = Grundverbrauch + (Vollgas − Grund) × Triebwerksstellung² ×
    Dichteverhältnis^k (wie der Schub). In der Höhe reicht der Tank also länger.
    Anders als im Original verbraucht auch der Leerlauf etwas.
  - Leerer Tank: Das Triebwerk läuft aus, kein Neustart, HUD „FLAMEOUT“.
  - Anzeige im Instrumentenbrett in Pfund mit Restflugzeit beim aktuellen
    Verbrauch. „FUEL LOW“ unter 15 %. „BINGO“, sobald der Rest bis zur
    Heimatbasis (mit mindestens Reisefahrt und halbem Schub gerechnet) plus
    Reserve nicht mehr reicht – mit der echten Strecke statt einer Faustformel.
  - Zusatztanks und Verbrauchsvorschau auf der Karte kommen mit v0.3.
- **Unsere Werte:** `crates/f29-data/src/aircraft.rs`: 5 200 kg intern (≈ 11 500 lb),
  Leerlauf 0,35 kg/s, Vollgas 4,3 kg/s auf Meereshöhe. Ergibt rund 20 min bei
  Vollgas und knapp eine Stunde im Reiseflug. Reserve für BINGO: 450 kg.
- **Status:** umgesetzt (v0.1.6)

### M-040 Flugplätze

- **Verhalten:** Flugplätze sind im Original keine echten Bahnen, sondern ein
  schmaler, langer Bereich um einen Mittelpunkt, alle in Nord-Süd-Richtung. Auf der
  leichtesten Landestufe ist der Bereich größer. Flugzeugträger haben einen kürzeren
  Bereich auf Deckhöhe; wer über das Deckende hinausrollt, stürzt ab.
- **Referenzstelle:** f19re `src/egframe.c` (Suche des nächsten Flugplatzes,
  Landekorridor, Startposition), `src/egflight.c` (Trägerstart).
- **Unser Design:**
  - Pro Seed eine **Heimatbasis** (3 km × 45 m, voll markiert, PAPI) und zwei
    **geheime Pisten** (1,5 km × 30 m, unbefestigt, kaum markiert, unbefeuert),
    deterministisch auf flachem Land gewählt.
  - Echte Bahnen in 22,5°-Schritten statt nur Nord-Süd. Das Gelände wird unter
    der Bahn eingeebnet und weich übergeblendet; Kollision, Radarhöhe und
    Darstellung gelten damit automatisch.
  - Die Route führt über zwei Wegpunkte zur nächstgelegenen geheimen Piste und
    zurück zur Heimatbasis. HSI zeigt bei Flugplätzen die Bahnrichtung.
  - Standardstart steht am Anfang der Heimatbahn (`--start air` für den Luftstart).
  - Flugzeugträger folgen mit Szenarien auf See.
- **Unsere Werte:** `crates/f29-terrain/src/airfield.rs`.
- **Status:** umgesetzt (v0.1.6)

### M-041 Fahrwerk und Rollen am Boden

- **Verhalten:** Das Fahrwerk ist ein Schalter, der sich am Boden nicht einfahren
  lässt. Ausgefahren kostet es Fahrt, rüttelt bei hoher Fahrt, und eine Leuchte
  blinkt, wenn man zu schnell ist. Am Boden wird die Querlage aufgehoben, der
  Rollknüppel lenkt (auch im Stand), die Nase kann nicht unter den Horizont, und
  hochziehen geht erst ab einer Mindestfahrt. Dieselbe Taste wirkt in der Luft als
  Luftbremse und am Boden als Radbremse.
- **Referenzstelle:** f19re `src/egkeys.c` (Tasten), `src/egflight.c`
  (`stepFlightModel`: Bodenbehandlung, Bremse, Fahrwerkswiderstand, Rütteln).
- **Unser Design:** Bewusst einfach wie im Original, ohne Federbeine:
  - Fahrwerk mit Ausfahrzeit (5 s), Widerstandsbeiwert, Rütteln über die
    Turbulenz oberhalb von ≈ 250 kt, Panel „GEAR DN/UP“ (blinkt in Bewegung).
    *v0.1.8 (Spielerwunsch):* Das ausgefahrene Fahrwerk bremst spürbarer
    (Widerstandsbeiwert fast verdoppelt) und rüttelt in der Luft immer leicht und
    gleichbleibend, unabhängig von Höhe und Fahrt; über der Höchstfahrt kommt das
    stärkere Rütteln dazu.
  - Rollmodus: Höhe auf den Boden geklemmt, keine Querlage, Lenken über Quer- und
    Seitenruder mit einer Drehrate, die mit der Fahrt abnimmt – auf der Stelle
    wenden geht. Rollreibung, Radbremse (Taste B) abhängig von der Radlast.
  - Rotieren erst ab 1,1 × Abrissfahrt (masseabhängig); Abheben, sobald der
    Auftrieb das Gewicht trägt.
  - *v0.1.7 (in v0.1.9 wieder entfernt):* **Startphase.** Beim Rotieren und nach dem Abheben steuert der
    Knüppel die Nicklage (voll gezogen = 12°) statt der Nickrate, und es wird
    keine g-Last angefordert. Vorher hob voller Zug die Nase in einer halben
    Sekunde auf 14°, und direkt nach dem Abheben riss die Strömung ab
    (Spielertest). Die Startphase endet, sobald der Knüppel losgelassen wird;
    danach gilt wieder die normale Ratensteuerung mit der gewohnt gefährlichen
    Abrissgrenze. Seit v0.1.9 ersetzt die querlagenabhängige Abrissanforderung
    (M-004) die Startphase; nur das Rotieren am Boden steuert weiter die
    Nicklage (bis 12°).
  - Wer schneller als ≈ 23 kt von der befestigten Fläche rollt, verunglückt.
- **Unsere Werte:** `crates/f29-data/src/aircraft.rs` (Abschnitt Fahrwerk und Boden).
  Startrollstrecke mit vollen Tanks und Klappen ≈ 0,7–1 km; eine geheime Piste
  reicht nach dem Wenden am Bahnende für einen Start mit halbvollen Tanks.
- **Status:** umgesetzt (v0.1.6)

### M-042 Aufsetzen und Landung

- **Verhalten:** Beim Aufsetzen auf einem Flugplatz ist die Landung ein Absturz,
  wenn die Sinkrate für die Landestufe zu hoch ist, das Fahrwerk eingefahren ist
  oder die Querlage zu groß ist. Außerhalb eines Flugplatzes ist jede schnelle
  Bodenberührung ein Absturz (außer auf der Stufe ohne Bruchlandungen). Wer auf
  einem Platz zum Stehen kommt, hat gelandet. Mit ausgefahrenem Fahrwerk zeigt das
  HUD die Sinkrate mit einer Marke für die höchste zulässige.
- **Referenzstelle:** f19re `src/egflight.c` (Aufsetzprüfung), `src/egframe.c`
  (Stillstand auf dem Platz), `src/egtacmap.c` (Sinkratenanzeige),
  `src_end/enbrief.c` (Bewertung in der Nachbesprechung).
- **Unser Design:**
  - Bewertung beim ersten Bodenkontakt: Wasser, Gelände (nicht auf der Bahn),
    Fahrwerk nicht verriegelt, Sinkrate über der Bruchgrenze oder Bugrad zuerst,
    Querlage über der Grenze → Absturz mit Ursache (im HUD angezeigt).
    Darunter: „GOOD LANDING“ oder „HARD LANDING“ mit Sinkrate in ft/min.
  - Stillstand auf einer Bahn: „STOPPED – <Bahn>“. Die Simulation läuft weiter,
    man kann wenden und wieder starten. Missionsende und Punkte kommen mit v0.3.
  - Landemodus im HUD (Fahrwerk unten, in der Luft): Sinkratenskala neben dem
    Glas mit Marken für „hart“ und „Bruch“, gestrichelte −3°-Linie in der Leiter.
  - PAPI an der Heimatbasis: vier Lichter links der Bahn, auf dem 3°-Gleitpfad
    zwei weiß, zwei rot.
  - „PULL UP“ ist mit ausgefahrenem Fahrwerk nur bei gefährlicher Sinkrate aktiv;
    „GEAR“ warnt im Anflug mit eingefahrenem Fahrwerk.
  - Schwierigkeitsstufen für die Grenzwerte folgen mit M-008.
- **Unsere Werte:** gut bis 4 m/s (≈ 790 ft/min), Bruch ab 7 m/s (≈ 1 380 ft/min),
  Querlage bis 15°, Nicklage nicht unter −5°. *v0.1.9:* gelockert nach Spieltest
  (vorher 3 / 5 m/s, 10°, −2°); eine flache Landung mit 190 kt ohne Abfangen
  gelingt jetzt als harte Landung. Die Fahrt beim Aufsetzen wird wie im
  Original nicht bewertet.
- **Status:** umgesetzt (v0.1.6)

### M-006 Turbulenz in Bodennähe

- **Verhalten:** Unterhalb von etwa 500 ft beginnt das Flugzeug zu wackeln. Die
  Stärke wächst, je tiefer und je schneller man fliegt. Außerdem gibt es
  Zusatz-Schütteln bei ausgefahrenem Fahrwerk und hoher Fahrt sowie bei Schäden.
  Ausgefahrene Landeklappen halbieren das Wackeln. Auf der leichtesten
  Flug-Schwierigkeitsstufe ist das Bodennähe-Wackeln abgeschaltet.
- **Referenzstelle:** f19re `src/egflight.c`, `stepFlightModel`: Zufallsanteil
  auf Roll- und Nickeingabe; um die Querachse halb so stark wie um die Längsachse.
- **Unser Design:** Wir verwenden die **Höhe über Grund** (Radarhöhe), weil unser
  Gelände echte Berge hat; das Original misst über einem praktisch ebenen Boden.
  Störraten auf Rollen und Nicken (Nicken halb), proportional zu
  `(1 − Höhe/1000 ft) × Fahrt`, Klappen halbieren sie. Die Turbulenz beginnt
  bewusst schon bei 1000 ft statt 500 ft (Spielerwunsch, v0.1.5), etwas stärker
  als in v0.1.4. Die Zufallszahlen kommen aus
  einem eigenen deterministischen RNG-Strom, damit Replays exakt bleiben. Sie
  werden geglättet, damit es rüttelt statt zittert. Seit v0.1.6 rüttelt es
  zusätzlich bei ausgefahrenem Fahrwerk über dessen Höchstfahrt; am Boden ruht
  die Turbulenz. Schwierigkeitsstufen und Schäden folgen später.

  *Designentscheidung (v0.1.9):* Die Turbulenz wird zum Boden hin **nicht**
  ausgeblendet, sie ist direkt über dem Boden am stärksten. Langsamer Tiefstflug
  zum Unterfliegen von Radaren (M-020) soll anspruchsvoll bleiben: Wer langsam
  fliegt, wackelt weniger, und Klappen beruhigen weiter, erhöhen dafür aber
  später den Radarquerschnitt.
- **Unsere Werte:** `TURBULENCE_*` in `crates/f29-core/src/sim.rs`.
- **Status:** umgesetzt (v0.1.4)

### M-007 Landeklappen

- **Verhalten:** Ausgefahrene Landeklappen senken die Abrissgeschwindigkeit um
  etwa ein Viertel, kosten etwas Fahrt und halbieren die Turbulenz.
- **Referenzstelle:** f19re `src/egflight.c` (Klappen-Bit der Flugzeugschalter in
  der Fahrt- und Abrissberechnung) und `src/egkeys.c` (Tastenzuordnung).
- **Unser Design:** Physikalisch über einen zusätzlichen Auftriebsbeiwert
  (`flap_cl`) und Widerstandsbeiwert (`flap_cd`). Der höhere maximale
  Auftriebsbeiwert senkt die Abrissgeschwindigkeit von selbst. Die Klappen fahren
  mit `flap_rate` aus. Bedienung über Taste F, Anzeige „FLAPS“ im Instrumentenbrett.

  *v0.1.8 – eigene Erweiterung, Spielerwunsch:* Die Klappen sind ein
  Schlüsselelement für Langsamflug und Landung und als Manöverklappen bei jeder
  Fahrt nutzbar (kein selbsttätiges Einfahren mehr):
  - **Langsam:** Die Ruder wirken mit Klappen deutlich stärker
    (`flap_control_q_factor`: Bezugsstaudruck × 0,35), ohne dass voller Zug
    mehr g anfordert. Der Abriss wird entschärft (`flap_stall_softening`): kein
    Abkippen der Nase, kaum Ruder- und Auftriebsverlust, allmählicher Einsatz.
    Ohne Klappen bleibt der Langsamflug so tückisch wie bisher – für Landungen
    sind Klappen praktisch Pflicht.
  - **Schnell:** Weniger induzierter Widerstand (`flap_induced_factor` 0,6):
    Harte Kurven kosten deutlich weniger Fahrt, Geradeausflug kostet etwas
    Fahrt (`flap_cd`). Später (v0.2) erhöhen ausgefahrene Klappen den
    Radarquerschnitt (M-020).
  - *v0.1.15:* Klappen lindern außerdem das Durchsacken im Langsamflug um
    60 % (M-004).
  - *v0.1.16 (Spielertest: Klappen zu mächtig):* Abschwächung der
    Abrissentschärfung `flap_stall_softening` 0,6 → 0,45, der Durchsacklinderung
    `flap_sink_relief` 0,6 → 0,4 und der Ruderhilfe `flap_control_q_factor`
    0,35 → 0,5. Die Abrissgeschwindigkeit sinkt weiterhin um etwa 15 %. Ein
    Abriss mit Klappen ist milder, aber nicht mehr harmlos (M-004).
  - *v0.1.17 (Vereinfachung):* Die Klappen wirken jetzt über vier Größen:
    Auftrieb (`flap_cl`, Abrissfahrt etwa −14 %), Widerstand (`flap_cd`),
    Manöverklappen (`flap_induced_factor`) und Milderung von Abriss und
    Durchsacken (`flap_stall_relief`). Die Ruderhilfe bei geringer Fahrt ist
    entfallen. Dazu halbieren sie weiterhin die Turbulenz (M-006).
  - *v0.1.18:* Die Klappen senken die Abrissfahrt nur noch im 1-g-Anteil,
    unter Last zählt die Abrissfahrt ohne Klappen (M-004). Ein Kurvenabriss ist
    mit Klappen wieder möglich. `flap_stall_relief` 0,45 → 0,3.
  - Gemessen (bis v0.1.15): Bei 135 kt und halbem Zug kippt das Flugzeug ohne Klappen auf
    −78° ab; mit Klappen bleibt die Nase oben, die Nickrate ist doppelt so hoch,
    die Rollrate bei 140 kt steigt von 44°/s auf 126°/s. Bei 450 kt und 8 s am
    g-Begrenzer bleiben mit Klappen ≈ 25 kt mehr Fahrt.
- **Unsere Werte:** `crates/f29-data/src/aircraft.rs` (Abrissgeschwindigkeit mit
  Klappen ≈ 85 % von ohne).
- **Status:** umgesetzt (v0.1.4, erweitert v0.1.8, vereinfacht v0.1.17)

### M-008 Bodenkollisions-Abfanghilfe (geplant)

- **Verhalten:** Auf leichten Schwierigkeitsstufen zieht das Original das
  Flugzeug in sehr geringer Höhe selbsttätig hoch, sofern der Pilot nicht drückt,
  die Querlage mäßig ist und die Fahrt über der Abrissgeschwindigkeit liegt.
- **Referenzstelle:** f19re `src/egflight.c`, `stepFlightModel`.
- **Unser Design:** Kommt mit den Schwierigkeitsstufen. Dann gegen die
  Radarhöhe und mit Vorausschau auf das Gelände.
- **Status:** geplant

### M-005 Höhenanzeige

- **Verhalten:** Im Original ist der Boden praktisch eben auf Höhe 0; erhöhter
  Boden gibt es nur an wenigen Objekten (z. B. Schiffsdeck). Die angezeigte Höhe ist
  dort faktisch die Höhe über Grund.
- **Referenzstelle:** f19re `src/egflight.c` und `src/egframe.c` (Bodenhöhe des
  Spielerflugzeugs, Aufsetzen).
- **Unser Design:** Unser Gelände hat echte Berge. Das HUD zeigt deshalb beides:
  die barometrische Höhe über dem Meer im Kasten und darunter **immer** die
  Radarhöhe über Grund („R …“), rot unter 500 ft.
- **Status:** umgesetzt (v0.1.1)

### M-050 Instrumentenbrett mit MFDs

- **Verhalten:** Das Instrumentenbrett nimmt einen großen Teil des Bildschirms ein
  (gut ein Drittel). Eine Kartenanzeige zeigt die Umgebung mit Gitter und lässt
  sich in Stufen zoomen. Eine Weitwinkelsicht blendet das Brett für den
  Luftkampf aus.
- **Referenzstelle:** keine Code-Lektüre für dieses Layout; Anordnung und Inhalt
  nach Spielerwunsch.
- **Unser Design:**
  - Brett 40 % der Fensterhöhe; zwei quadratische MFDs, dazwischen ein schmaler
    senkrechter Balken, reserviert für die Stealth-/Radarimpuls-Anzeige (M-020).
    Ohne Brett (`panel_fraction` 0, spätere Weitwinkelsicht) entfallen die MFDs.
  - **Links: Karte**, Flugrichtung oben, Flugzeug in der Mitte. Zwei Modi (F3):
    *Gitter* mit Kartengitter über einem Höhenbild in Graustufen und *Satellit*
    (Farbraster mit Hangschattierung). Beide Raster 2048², ≈ 98 m/px, im
    Hintergrund berechnet, pro Bild kursbezogen und pixelig ausgeschnitten.
    *v0.2.9 (Spielerwunsch, nach dem Original: dunkle Fläche mit Gitter und
    Symbolen):* Die Höhenlinien des Gittermodus sind ersetzt durch das
    Höhenbild – Wasser schwarz, Land gedämpft grau, je höher, desto heller
    (nach der Wurzel der Höhe, damit auch das Flachland Relief zeigt). Die
    Zielsymbole sind größer und einfacher: Quadrat (Fahrzeuge), Quadrat mit
    Kreuz (Bauwerke, Bunker, Brücken), Dreieck (Flugabwehr; Frühwarnradar mit
    Querstrich), Kreuz (abgestellte Flugzeuge), Raute (Schiffe). In beiden Modi: Flugplätze, Route
    mit aktivem Wegpunkt, Nordpfeil, Kurs, Bereich. Zoom in fünf Stufen mit
    Y/X (2–40 NM von der Mitte bis zum Rand).
  - **Rechts: Seiten** (F4 blättert, bisher eine): *Ressourcen* mit vier leeren
    Waffenstationen als Platzhalter und dem Treibstoffbalken unten (Pfund,
    Restflugzeit, BINGO-Marke „B“ für den Bedarf bis nach Hause plus Reserve).
  - *v0.1.11:* **Schubhebel** ganz links im Brett. Inspiration: Im Original
    (f19re `src/egflight.c`, `drawAirspeedTape`) zeichnet die Schubanzeige
    unten links einen Hebel, dessen Schaft von einem Drehpunkt in der Ecke
    schräg nach oben läuft und dessen Griff mit dem Schub diagonal nach oben
    wandert. Unser Hebel schwenkt in einer Bogenkulisse von IDLE (flach) bis
    MIL (steil), mit dreifachem Schaft, T-Griff, Skala alle 10 % und einer
    grünen Marke für die tatsächliche Triebwerksstellung, die dem Hebel
    verzögert folgt.
  - *v0.1.11:* Fahrwerk, Klappen und Bremse stehen wieder im HUD, unten links
    unter den Flugdaten, und nur solange sie ausgefahren sind. Das Fahrwerk
    blinkt, während es fährt, und bei Überfahrt; am Boden heißt die Bremse
    „BRAKES“. Jede Anzeige hat eine feste Zeile (Fahrwerk, Klappen, Bremse),
    damit Blinken oder Ausblenden die anderen nicht verschiebt (v0.1.15).
  - Die bisherigen Rundinstrumente (Drehzahl, HSI, Horizont) entfallen und kommen
    später als eigene MFD-Seiten zurück.
  - Ansichtstasten liegen auf Shift + F-Taste (Cockpit, Verfolger, Retro-Look,
    HUD).
  - *v0.1.19:* Das rechte MFD hat fünf Seiten, F4 schaltet weiter, eine
    Seitenleiste unten zeigt die aktive. Die Ressourcen-Seite entfällt: Die
    Waffen stehen auf WPN, der Treibstoffbalken auf ENG. An ihre Stelle tritt
    MSN: Primär- und Sekundärziel (Platzhalter bis v0.3), die Route mit
    Teilstrecken und markiertem aktivem Punkt, Entfernung, Peilung und
    Flugzeit dorthin, Entfernung nach Hause und BINGO-Menge. WPN: vier Waffenschächte als liegende Rechtecke (2 × 2, Platzhalter bis
    v0.2), der gewählte doppelt gerahmt, darunter Angaben zur gewählten Waffe
    (Typ, Anzahl, Gewicht, Lenkung, Auslösung) und der Waffen-Hauptschalter
    (SAFE). ENG: Drehzahl als Rundinstrument (Leerlauf 62 %, Vollgas 100 %)
    mit Schubhebelmarke, Schub, Treibstofffluss, Restflugzeit, FLAMEOUT.
    ENG zeigt außerdem den Treibstoffbalken mit BINGO-Marke.
    ADI: Reserve-Horizont mit Querlagenskala, Fahrt, Höhe, Kurs, Steigrate und
    g-Last. SYS: Fahrwerk, Klappen, Luftbremse, Gesamtgewicht, Abrissfahrt,
    Mach und ein Warnfeld mit allen Warnungen. Die HSI-Funktion übernimmt die
    Karte. Die Weitwinkelsicht (Taste C) ist auf v0.4 (Luftkampf) verschoben.
- **Status:** umgesetzt (v0.1.10, Seiten v0.1.19)

### M-060 Klang

- **Verhalten:** Das Original nutzt einen eigenen AdLib-Treiber (FM-Synthese).
  Das Triebwerk bekommt laufend Fahrt und Schub als Tonhöhe übergeben;
  Ereignisse wie Aufsetzen und Abrisswarnung sind kurze, nummerierte
  Klangmuster.
- **Referenzstelle:** f19re `src/egflight.c` (`audio_setEnginePitch(Fahrt,
  Schub)`, Aufrufe von `makeSound` bei Aufsetzen und Abriss), `src/asound.c`
  (Treiber, bisher nur in Bruchstücken rekonstruiert).
- **Unser Design:** Eigene prozedurale Synthese im Crate `f29-audio`, keine
  Klangdateien, keine fremden Aufnahmen. Engine-neutral: Ein `Synth` erzeugt aus
  einer Momentaufnahme der Simulation einen Mono-Datenstrom; Bevy spielt ihn als
  eigene Audioquelle ab, WAV-Hörproben entstehen ohne Soundkarte
  (`f29-headless sound`). Charakter: klassisch-retro, aber zurückhaltend.
  - **Triebwerk:** zwei leicht verstimmte, weich gefilterte Rechtecktöne mit
    der Drehzahl als Tonhöhe (das tragende Brummen), dazu dunkles Rauschen
    (zweifacher Tiefpass, 90–470 Hz je nach Schub), das im Takt des Brummens
    pulsiert und so mit ihm verschmilzt, und ein leises Turbinenpfeifen. Bei
    Flameout läuft es langsam aus. **Rollgeräusch** am Boden.
  - *v0.1.13 (Spielertest):* Das Rauschen war zu laut und zu schrill – jetzt
    deutlich leiser und dunkler. Das Windgeräusch ist entfernt; die Simulation
    hat keinen Wind.
  - **Fahrwerk:** Motorbrummen beim Fahren, dumpfer Schlag beim Verriegeln.
    **Klappen:** Surren, Klick am Ende. **Luftbremse:** Zischen beim Aus- und
    Einfahren.
  - **Aufsetzen:** Reifenquietschen und Schlag, bei harter Landung lauter.
    **Absturz:** Krachen; das Triebwerk verstummt.
  - **Abrisswarnung:** *v0.1.14 (Spielerwunsch):* hoher, pulsierender
    Alarmton (1250 Hz, 6 Pulse/s, 40 % an), solange die Warnung anliegt.
  - **Bodenwarnung „PULL UP“:** Ansage alle 1,6 s, solange die PULL-UP-Warnung
    anliegt. Gedacht für eine authentische Aufnahme
    (`assets/sounds/pull_up.wav`, nur frei lizenzierte Aufnahmen, Herkunft in
    `assets/sounds/README.md`). Ohne Aufnahme spricht ein kleiner
    Formant-Synthesizer (Stimmquelle durch drei Formant-Resonatoren, Zisch- und
    Explosivlaute aus gefiltertem Rauschen, grobe Abtastung und Quantisierung
    wie bei frühen Sprachchips). Das Wort „STALL“ derselben Stimme bleibt für
    spätere Ansagen erhalten.
  - Hörproben rendern Einzelklänge ohne Triebwerk (Klanggruppen Triebwerk,
    Systeme, Ereignisse, Ansagen lassen sich getrennt pegeln).
  - Pause schaltet weich stumm. Startparameter `--volume 0..100` und
    `--no-sound` (dann wird kein Audiogerät geöffnet).
  - Musik (z. B. im Startbildschirm) kommt später als eigener Kanal über Bevys
    Dateiwiedergabe dazu und berührt den Synthesizer nicht.
- **Status:** umgesetzt (v0.1.12)

### M-002 Prozedurales Gelände

- **Verhalten:** Das Original verwendet handgebaute Einsatzgebiete.
- **Referenzstelle:** keine. Wir übernehmen weder Karten noch Geländedaten.
- **Unser Design:** Das Gelände ist eine deterministische Funktion des Seeds aus
  Gradientenrauschen, fBm und Grat-Rauschen für Gebirgszüge, mit Meer, Küsten, Wald und
  Fels. Sie wird für Kollision und Darstellung identisch ausgewertet. Eigene, fiktive
  Einsatzgebiete entstehen später aus Seed und Parametern.
- **Unsere Werte:** `TerrainParams::default()` in `crates/f29-terrain/src/lib.rs`.
- **Status:** umgesetzt (v0.1)

### M-003 HUD und Cockpit-Instrumente

- **Verhalten:** Genre-typisches HUD (Pitch-Ladder, Flight Path Marker, Kurs-,
  Geschwindigkeits- und Höhenanzeige) über der 3D-Sicht, darunter ein Instrumentenbrett.
- **Referenzstelle:** keine (allgemeine HUD-Konventionen realer Kampfflugzeuge).
- **Unser Design:** Eigene Anordnung und ein eigener Vektor-Strichfont
  (`crates/f29-hud`). Alle Anzeigen sind Liniengrafik in auflösungsunabhängigen
  Bildschirmeinheiten.
- **Status:** umgesetzt (v0.1)

### M-010 Waffenwirkung nach Waffe und Zielklasse

- **Verhalten:** Die Wirkung eines Treffers hängt von der Kombination aus Waffentyp und
  Zielklasse ab, d. h. eine Tabelle `[Waffe][Zielklasse]`.
- **Referenzstelle:** f19re `src_en/egtacmap.c` (Nachschlagen der Waffenwerte je
  Zielklasse), `src_en/egtarget.c` (Klassenbezug der Ziele, Wasserziele, Hinweis auf
  zivile/befreundete Ziele), `src_en/egcombat.c` (Wirkung beim Einschlag).
- **Lesephase v0.2 (eigene Worte, keine Werte):** Jedes Bodenobjekt gehört über sein
  Modell zu einer von gut einem Dutzend Zielklassen. Ein paar Klassen sind Wasserziele
  (Schiffe), dazu kommen Kennzeichen für „zivil oder befreundet“, die das Cockpit beim
  Aufschalten als Hinweis zeigt (auf leichten Stufen immer, sonst nur für bestimmte
  Ziele). Zu jeder Waffe gibt es eine Zeile mit einem kleinen Wirkungswert je
  Zielklasse; null heißt „wirkungslos“, dann lässt sich das Ziel mit dieser Waffe gar
  nicht aufschalten. Derselbe Wert bestimmt die Größe des Zielrahmens im HUD, bei
  Flugkörpern die Zerstörungswahrscheinlichkeit beim Treffer (sonst Meldung
  „wirkungslos“) und bei Bomben den Wirkradius. Auch die Bordkanone hat eine Zeile.
  Die Namen der Klassen stehen in Datendateien, die f19re nicht enthält – unsere
  Klassen sind ohnehin eigene.
- **Unser Design:** Neun eigene Zielklassen (`f29_core::targets`): weich (Lastwagen,
  Depots), gepanzert, Gebäude, gehärtet (Bunker, Schutzbauten), Flugabwehrstellung,
  Brücke, abgestelltes Flugzeug, kleines und großes Schiff. Radar und Flugabwehr sind
  wie im Original **ein** Objekt: feste Stellungen mit eigenem Radar und eigenen
  Raketenwerfern (Spielerentscheidung). Ein Kennzeichen markiert zivile Ziele
  (Fabrik, Lagerhalle, Fischerboot, Frachter, Tanker), die nicht angegriffen werden
  sollen; Missionsziele sind nie zivil. Jede Klasse hat eine Form aus wenigen Quadern,
  aus der das 3D-Modell und das optische Zielbild entstehen.
  Platzierung pro Seed (eigener RNG-Strom): Primärziel mit Bewachung am zweiten,
  Sekundärziel am ersten Wegpunkt, fünf Flugabwehrstellungen entlang der Route,
  Schiffsgruppen vor der Küste, weitere Stellungen verstreut; Landziele nur auf
  flachem Land, Schiffe nur auf offenem Wasser, Abstand zu Flugplätzen und
  untereinander. Alle Ziele erscheinen auf der Karte mit einem Symbol je Klasse,
  Primär- und Sekundärziel markiert (P/S), zivile gedämpft.
  Wirkungsmatrix Waffe × Zielklasse: `WeaponKind::effectiveness` als **Schaden pro
  Treffer** (Spielerentscheidung v0.2.5): 1 = ein Treffer zerstört, 0,5 = zwei
  Treffer, 0 = wirkungslos. Der Schaden sammelt sich am Ziel (beschädigt →
  zerstört). Brücken, Gebäude und gepanzerte Ziele brauchen zwei Bomben, große
  Schiffe drei; Radarjäger nur gegen Flugabwehr, Seezielrakete nur gegen Schiffe
  (große: zwei), Luft-Boden-Rakete schwach gegen Gebäude (drei), Brücken und große
  Schiffe (vier). Jeder Treffer würfelt unabhängig einen **kritischen Treffer**
  (1 %), der das Ziel sicher zerstört – mit Glück reicht dann eine Bombe für die
  Brücke. Keine Sekundärexplosionen, keine Wirkung auf Nachbarziele.
- **Unsere Werte:** `crates/f29-core/src/targets.rs`; je Seed etwa 30–50 Ziele.
  Wirkung: `crates/f29-core/src/weapons.rs`.
- **Status:** Ziele umgesetzt (v0.2.1), Wirkung umgesetzt (v0.2.4), Schaden pro
  Treffer und kritische Treffer (v0.2.5)

### M-011 Einschlagdistanz und Schwierigkeitsfaktor

- **Verhalten:** Ob ein Ziel zerstört wird, hängt von der Entfernung zwischen
  Einschlagpunkt und Ziel ab. Der zulässige Abstand skaliert mit dem Schwierigkeitsgrad.
- **Referenzstelle:** f19re `src_en/egcombat.c` (Einschlag von Bomben und Flugkörpern),
  `src_en/egtarget.c` (Bordkanone gegen Bodenziele).
- **Lesephase v0.2:** Bomben zerstören ein Ziel, wenn der Einschlag näher am Ziel liegt
  als ein Abstand aus Wirkungswert (M-010) und Schwierigkeitsgrad – auf schweren Stufen
  muss man genauer treffen. Flugkörper, die ihr Ziel erreichen, zerstören es mit einer
  Wahrscheinlichkeit, die mit dem Wirkungswert steigt. Wer zu tief auslöst, wird von
  den eigenen Splittern getroffen.
- **Unser Design:** Bomben sind rein physikalisch (Spielerentscheidung): Wirkradius
  je Waffe; bis 40 % davon (vom Zielrand) voller Schaden – das ist ein Treffer –,
  dann linear weniger bis null. Kein Würfel außer dem kritischen Treffer (M-010).
  Lenkwaffen treffen geometrisch und würfeln dann die Wirkung wie im Original
  (M-014, v0.2.9). Der Schwierigkeitsgrad skaliert später den Wirkradius.
- **Status:** umgesetzt ohne Schwierigkeitsstufen (v0.2.4, v0.2.5)

### M-012 Gehärtete Ziele: Spezialmunition und Toss-Bombing

- **Verhalten:** Gehärtete Ziele lassen sich nur mit spezieller Munition zerstören. Die
  Bombe muss im Toss-Bombing-Verfahren (Hochwurf aus dem Steigflug) und aus einer
  vorgegebenen Anflugrichtung (Norden) ins Ziel gebracht werden.
- **Referenzstelle:** f19re `src_en/egcombat.c` (Flugbahn der abgeworfenen Waffen).
- **Lesephase v0.2:** Es gibt drei Bombenarten mit eigener Flugbahn: frei fallend
  (die Bahn neigt sich stetig nach unten), gelenkt (gleitet flach, sucht ihr Ziel
  selbst) und eine dritte mit eigenem Sinkprofil. Dazu Lastabwurf (Fracht) als
  Missionsart. Eine echte Eintreffwinkel-Prüfung für gehärtete Ziele war im gelesenen
  Code nicht zu finden – das bleibt unser eigener Entwurf.
- **Unser Design (Entwurf):** Eintreffwinkel und Eintreffgeschwindigkeit der Bombe sind
  Teil der Wirkungsberechnung. Ein gehärtetes Ziel verlangt einen steilen Eintreffwinkel,
  der sich praktisch nur durch Toss-Bombing erreichen lässt. Die erlaubte
  Anflugrichtung ist eine **Eigenschaft des Ziels** in den Szenariodaten (z. B.
  Bunkereingang in eine Richtung), nicht global „Norden“. Die Bombenballistik ist echte
  Simulation im Kern, kein Würfelwurf.
  Umgesetzt: Eintreffwinkel ab 60° (Freifall- und Gleitbombe). Die Anflugrichtung
  je Ziel folgt mit den Szenariodaten.
- **Status:** Eintreffwinkel umgesetzt (v0.2.4), Anflugrichtung geplant

### M-013 Zielerfassung und Lenkung

- **Verhalten:** Bodenziele (und Flugzeuge) werden mit einem Zielsystem aufgeschaltet
  und im Cockpit mit Name, Entfernung und Bild gezeigt. Gelenkte Waffen fliegen das
  aufgeschaltete Ziel an.
- **Referenzstelle:** f19re `src_en/egtacmap.c` (Zielauswahl pro Bild),
  `src_en/egtarget.c` (Aufschalten, Zielrahmen, Zielbild), `src_en/egcombat.c`
  (Suchkopf der Flugkörper, Abschuss), `src/egkeys.c` (Tasten).
- **Lesephase v0.2 (eigene Worte, keine Werte):**
  - Aufschalten: Der Spieler wählt eine Blickrichtung (voraus, links, rechts, hinten).
    Das System nimmt das nächstgelegene noch intakte Ziel in einem breiten Kegel um
    diese Richtung und innerhalb einer Höchstreichweite; erneutes Drücken springt zum
    nächstweiteren. Nachts ist die Reichweite deutlich kürzer.
  - **Es gibt keine Sichtlinienprüfung**: Ziele hinter Bergen lassen sich genauso
    aufschalten (Vermutung des Spielers bestätigt). Auch der Suchkopf eines Flugkörpers
    prüft nur Kegel und Entfernung, kein Gelände; der Flugkörper selbst kann aber am
    Gelände zerschellen.
  - Ein eigenes Bordradar, das man ein- und ausschaltet, gibt es im Original nicht;
    das Zielsystem ist ein Kamera-/Sensorsystem. Emittierend ist nur der Störsender.
  - Feuerbereitschaft für eine gelenkte Waffe: Die Waffe muss gegen die Zielklasse
    wirken (M-010), noch vorhanden sein, das Ziel muss nahe der HUD-Mitte liegen und
    innerhalb der Aufschaltreichweite dieser Waffe. Die gelenkte Bombe verlangt statt
    der HUD-Mitte, dass das Ziel steil genug unter dem Flugzeug liegt.
  - Nach dem Abschuss lenkt der **eigene Suchkopf** des Flugkörpers: Er hält das Ziel,
    solange es in seinem Kegel und in Reichweite liegt; ohne Aufschaltung sucht er
    selbst das beste Ziel seiner Art (Radarjäger nur aktive Radare, Seezielflugkörper
    nur Schiffe). Eine Verbindung zum Flugzeug ist nicht nötig. Verliert er das Ziel,
    fliegt er geradeaus weiter.
  - Waffenarten wechselt eine Taste (Luft-Boden, Luft-Luft, Kanone), die Station eine
    andere; der Schacht muss offen sein, sonst Meldung „Schacht zu“.
- **Unser Design (v0.2.1, Spielerentscheidungen):** Die F-29 hat zwei
  Erfassungssysteme (`f29_core::sensors`):
  - **Radar:** schaltet ohne Sichtlinie auf, also auch durch Berge; größere
    Reichweite; abschaltbar (Taste V, beim Start aus). Eingeschaltet erhöht es die
    eigene Sichtbarkeit (kommt mit M-020).
  - **Optisch:** braucht freie Sichtlinie (Prüfung gegen das Gelände), hat eine
    kürzere Reichweite, verrät das Flugzeug aber nicht. Nur ein optisch erfasstes
    Ziel liefert ein Zielbild.
  - Das Aufschalten ist **immer automatisch**: das nächste intakte Ziel in einem
    Kegel um die gewählte Blickrichtung (I voraus, L rechts, K hinten, J links), wie
    im Original. Zivile Ziele werden mit aufgeschaltet, aber gekennzeichnet.
  - **Zielwechsel (v0.2.2):** Dieselbe Richtungstaste noch einmal schaltet zum
    nächstweiteren erfassten Ziel dieser Richtung; nach dem weitesten geht es zurück
    zum nächsten. Eine andere Richtung setzt auf deren nächstes Ziel.
  - **Aufschaltung halten (v0.2.3):** Der Kegel gilt nur für das Aufschalten. Eine
    bestehende Aufschaltung bleibt, solange ein System das Ziel erfasst (Reichweite,
    beim optischen System auch Sichtlinie), und die Blickrichtung dreht mit dem Ziel
    mit – beim Überflug etwa von voraus über die Seite nach hinten. Die Taste der
    Richtung, in der das Ziel gerade liegt, schaltet dort zum nächsten weiter.
  - Anzeige: Zielrahmen im HUD (durchgekreuzt bei zivilen Zielen, Pfeil am Rand
    außerhalb des Sichtfelds) mit „OPT“ oder „RDR“; Zeile mit Radar, Blickrichtung
    und Entfernung; neue MFD-Seite TGT mit optischem Zielbild, Name, Klasse, Rolle,
    Entfernung, Peilung; auf der Karte ist das aufgeschaltete Ziel eingeklammert.
  - **Zielkamera (v0.2.2):** Das Zielbild ist eine zweite 3D-Kamera unter dem Rumpf,
    die auf die Zielmitte blickt und so weit zoomt, dass das Ziel knapp die halbe
    Bildhöhe füllt. Sie rendert in eine grobe Textur (144 Zeilen); ein eigener
    Nachbearbeitungseffekt macht daraus ein körniges, leicht grün getöntes
    Schwarzweißbild mit Zeilenstruktur, gelegentlichem Zeilenversatz und dunklen
    Rändern. Das Rauschen nimmt mit der Entfernung zu. Die Kamera läuft nur bei
    offener TGT-Seite und optischer Erfassung. Ohne Grafik (SVG-Ausgabe) bleibt das
    Drahtgitter als Ersatzbild.
  - Für Etappe 2 festgelegt: Gelenkte Waffen sind **Fire and Forget** wie im
    Original – sie tragen ihren eigenen optischen oder Radar-Suchkopf und brauchen
    nach dem Abschuss keine Verbindung zum Flugzeug. Erste Waffen: Freifallbomben,
    gelenkte Bomben, Fallschirmbomben (verzögerter Fall) und gelenkte Raketen.
- **Tag und Nacht (v0.2.7, Spielerentscheidung):** Wie im Original gibt es
  Nachtflüge (`--night`, im Replay als Tageszeit). Nachts schrumpft die optische
  Reichweite stark, das Radar verliert nichts; die Außensicht ist abgedunkelt und
  bläulich.
- **AWACS-Datalink (v0.2.7, Spielerentscheidung):** pseudosimuliert als
  Startbedingung „freundliches AWACS in der Luft“ (Standard an, `--no-awacs`,
  im Replay). Mit AWACS bleiben alle Ziele auf der Karte; was die eigenen
  Sensoren nicht erfassen (Radar an und in Reichweite, oder optisch in
  Reichweite mit freier Sicht), kommt per Datalink mit verminderter
  Information: Lage und Zieltyp (Symbol: Schiff, Bunker, Radar …), aber ohne
  Kennung „zivil“ und einheitlich gedämpft. Ohne AWACS fehlen diese Ziele;
  Primär- und Sekundärziel bleiben markiert (Briefing). Aufschalten geht nur
  mit den eigenen Sensoren. Die eigene Erfassung wird reihum geprüft (vier
  Ziele je Erfassungstakt).
- **Unsere Werte:** Radar 50 km Tag und Nacht, optisch 50 km bei Tag und 20 km
  nachts (Spielerentscheidung v0.2.7). Kegel ±45° um die Blickrichtung, Erfassung 4-mal pro Sekunde (ein
  Wechsel von Radar oder Blickrichtung wirkt sofort), Sichtlinie in 60-m-Schritten
  (höchstens 1000 Prüfpunkte). Bis v0.2.6 waren es optisch 14,8 km und Radar
  37 km.
- **Status:** Erfassung umgesetzt (v0.2.1), Zielwechsel und Zielkamera (v0.2.2), Aufschaltung beim Überflug halten (v0.2.3), Suchkopf und Lenkung der Waffen (v0.2.5, Details M-014), große Reichweiten und Nacht (v0.2.7)

### M-014 Waffenschacht, Abwurf und Zielhilfe

- **Verhalten:** Die Waffen hängen in vier internen Schächten. Der Spieler wählt
  Waffenart und Station, öffnet den Schacht und löst aus; Bomben fallen, gelenkte
  Waffen fliegen mit eigenem Suchkopf. Das HUD hilft beim Zielen.
- **Referenzstelle:** f19re `src_en/egcombat.c` (Abschuss, Flugbahn und Lenkung,
  Einschlag), `src_en/egframe.c` (Grundbeladung, Stationen), `src_en/egtarget.c`
  (Zielhilfe im HUD).
- **Lesephase Etappe 2 (eigene Worte, keine Werte):**
  - Beladung: vier Stationen, jede mit einer Waffenart und einer Stückzahl; dazu
    Kanonenmunition. Eine Station kann statt Waffen auch Zusatztreibstoff tragen,
    der beim „Auslösen“ in den Tank wandert.
  - Auslösen nur bei nicht zu großer Querlage, mit Munition an der Station und
    offenem Schacht – sonst Meldung „Schacht zu“. Auslösen beendet die
    Zeitraffung. Nur wenige eigene Waffen gleichzeitig in der Luft.
  - Start: Die Waffe übernimmt Position (knapp unter dem Flugzeug), Kurs, Lage und
    Fahrt des Flugzeugs; die Nase zeigt etwas nach unten. Flugkörper beschleunigen
    danach auf ihre Höchstfahrt. Eine Lebensdauer (aus Reichweite und Fahrt)
    begrenzt den Flug; Ungelenktes fliegt bis zum Boden.
  - Lenkung: Bei Abschuss mit Aufschaltung übernimmt die Waffe das Ziel, wenn ihre
    Art dazu passt (Radarjäger nur auf Stellungen mit aktivem Radar). Ohne
    Aufschaltung sucht der Suchkopf selbst das nächste passende Ziel in seinem
    Kegel (See- oder Landziel je nach Waffe). Kurs und Nicklage drehen mit einer
    begrenzten Drehrate je Waffe auf das Ziel, die Nicklage zielt dabei etwas über
    den direkten Weg (leichter Bogen). Ohne Ziel sinkt die Nase langsam ab.
  - Bombenbahnen: die Freifallbombe neigt sich stetig nach unten bis zu einem beim
    Abwurf festgelegten Grenzwinkel (Wurf aus dem Steigflug möglich); die gelenkte
    Bombe hält mindestens einen flachen Gleitwinkel und lenkt; eine dritte Art fällt
    langsam senkrecht (Lastabwurf).
  - Einschlag einer Bombe: Das nächste Ziel am Einschlagpunkt wird zerstört, wenn
    der Abstand kleiner ist als ein Wert aus Wirkung (M-010) und Schwierigkeit;
    sonst Meldung „verfehlt“. Liegt der Einschlag zu nah am eigenen Flugzeug
    (waagerecht plus Höhe), trifft der Splitterschaden das Flugzeug. Flugkörper,
    die ihr Ziel erreichen, zerstören es mit einer Wahrscheinlichkeit aus dem
    Wirkungswert, sonst „wirkungslos“. Ein Einschlag macht den Gegner auf diesen
    Ort aufmerksam. Gelände beendet jeden Flug.
  - Zielhilfe für die Freifallbombe: Bei geringer Querlage zeigt das HUD den
    vorausberechneten Aufschlagpunkt auf dem Boden voraus, mit einer Linie vom
    Flugbahnsymbol. Mit aufgeschaltetem Ziel kommt eine Raute über dem Ziel und
    oben im HUD eine Klammer, die mit dem Abstand zwischen Aufschlagpunkt und Ziel
    schrumpft – geschlossen heißt „jetzt auslösen“. Die Klammer blinkt, wenn das
    Flugzeug zu tief für einen sicheren Abwurf ist. Für die gelenkte Bombe zeigt
    die Klammer die Entfernung zum Ziel.
- **Unser Design (Etappe 2, Spielerentscheidungen):** `f29_core::weapons`.
  - Sechs Waffenarten mit den Bezeichnungen der realen Vorbilder
    (Spielerentscheidung; öffentliche Typbezeichnungen, nicht aus dem Original
    übernommen): Mk 82 (Freifallbombe), Mk 82 Snakeye (Fallschirmbombe,
    Bremsschirm öffnet kurz nach dem Abwurf), GBU-12 Paveway II (lasergelenkte
    Bombe, bis v0.2.6 AGM-62 Walleye), AGM-65 Maverick (optisch gelenkte Luft-Boden-Rakete),
    AGM-88 HARM (Radarjäger, nur Flugabwehrstellungen), AGM-84 Harpoon
    (Seezielrakete, nur Schiffe). Alle gelenkten Waffen sind Fire and Forget
    (M-013). Masse und Größenordnungen folgen den Vorbildern, Lenk- und
    Wirkungswerte sind Spielwerte.
  - Beladung: vier Schächte mit je einer Waffenart, **feste Standardbeladung**
    (Mk 82, Snakeye, GBU-12, Maverick), änderbar mit
    `--loadout`; ein Beladungsbildschirm kommt mit dem Missionsbriefing. Die
    Waffen zählen zur Flugzeugmasse und werden beim Abwurf leichter.
  - Bedienung: Schachtklappen mit **eigener Taste** (öffnen dauert gut eine
    Sekunde, in Etappe 3 erhöht ein offener Schacht die Sichtbarkeit; solange die
    Klappen nicht ganz zu sind, zeigt die Waffenanzeige unten links „OPEN“ bzw.
    „DOORS“ in Warnfarbe; die zentrale Warnung „BAY OPEN“ ist seit v0.2.7
    entfallen, Spielerwunsch), Schacht
    wählen, auslösen. Auslösen nur bei offenem Schacht, nicht am Boden, bis 60°
    Querlage und mit höchstens acht eigenen Waffen in der Luft; sonst Meldung.
  - Bombenflugbahn: echte Ballistik im Kern statt Neigungsregel – Massenpunkt mit
    Schwerkraft und Luftwiderstand aus der Normatmosphäre, 120 Hz. Start knapp
    unter dem Rumpf mit kleiner Ausstoßgeschwindigkeit. Wurf aus dem Steigflug
    (Toss) ergibt sich daraus von selbst. Eine Vorhersage mit derselben Rechnung
    liefert den Aufschlagpunkt (für die Zielhilfe).
  - Bombeneinschlag: Alle Ziele im Wirkradius erhalten Schaden nach Abstand
    (M-011), gehärtete Ziele nur bei steilem Eintreffen (M-012). Zerstörte Ziele
    werden zum flachen, dunklen Wrack, die Aufschaltung fällt ab.
  - **Lenkwaffen – physikalisch seit v0.2.9 (wie im Original, Spielerentscheidung):
    Der Flug entscheidet über den Treffer, gewürfelt wird nur die Wirkung.**
    Dasselbe System ist für die feindlichen Raketen vorgesehen (Etappe 3).
    - Raketen (Maverick, HARM, Harpoon) fallen kurz frei aus dem Schacht, gehen
      auf ihre **Marschfahrt und halten sie** (kein Brennschluss, wie im
      Original) und fliegen **reine Verfolgung** mit begrenzter Drehrate auf
      die Zielmitte. Der Suchkopf prüft gestaffelt zehnmal pro Sekunde, ob er
      das Ziel noch im Suchkegel sieht (optisch mit Sichtlinie); sonst fliegt
      die Rakete geradeaus weiter. Liegt das Ziel mehr als 135° hinter der
      Flugrichtung, gibt sie es auf (wie im Original).
    - **Treffer geometrisch** (wie im Original): Kommt die Rakete der Zielmitte
      näher als ihr Wirkradius, zündet sie. Gelände im Weg oder Treibstoffende
      (Lebensdauer) beenden den Flug ohne Treffer am Ziel.
    - **Wirkung gewürfelt** (wie im Original): Die Wirkungsmatrix ist die
      Chance, das Ziel zu zerstören (Maverick gegen Gebäude etwa jedes dritte
      Mal), sonst „INEFFECTIVE“. Bomben behalten den Schaden nach Abstand.
    - **Abschusskreis** (Spielerentscheidung): ein fester Kreis von ±8° um die
      Flugzeugnase im HUD, gleich für alle Raketen. Übergabe beim Abschuss nur,
      wenn das aufgeschaltete Ziel im Kreis liegt, in Reichweite ist und der
      Suchkopf es sieht (Klasse passt, optisch mit Sichtlinie). Optische
      Suchköpfe übernehmen nur eine **optische** Aufschaltung.
    - Ohne Übergabe fliegt die Rakete nach dem Herausfallen **in ihrer
      Abschusslage geradeaus**; erkennt ihr Suchkopf (Kegel um die Flugrichtung)
      dabei ein Ziel, schaltet sie es auf und verfolgt es – **auch zivile
      Ziele**; HARM nur Flugabwehr, Harpoon nur Schiffe.
    - **Laserbombe GBU-12 Paveway II** (ersetzt die Walleye, Spielerentscheidung):
      übernimmt ein optisch aufgeschaltetes Ziel, fällt physikalisch und dreht
      ihre Flugbahn mit begrenzter Rate zum Laserfleck. Kein Abschusskreis,
      sonst ginge nur der Sturzflug. Das Flugzeug muss das Ziel **bis zum
      Einschlag optisch aufgeschaltet halten** (Laserbeleuchtung,
      Spielerentscheidung); reißt die Sicht ab oder wechselt der Pilot das
      Ziel, meldet das HUD „LASER LOST“ und die Bombe fällt ungelenkt weiter.
      Ob ihre Energie reicht, zeigt allein die Vorausberechnung.
    - **Vorausberechnung („SHOOT“):** Zwanzigmal pro Sekunde (und sofort nach
      einer neuen Aufschaltung) fliegt der Kern die gewählte Lenkwaffe vom
      momentanen Abschusspunkt aus in groben Schritten bis zum Ende voraus –
      mit derselben Übergabe und demselben Flug wie beim echten Auslösen,
      Sichtlinie und Laser als gehalten angenommen. Daraus die Zone: übernähme
      nicht (außer Reichweite, nicht im Abschusskreis), übernähme, aber
      verfehlte („LOCKED“), träfe („SHOOT“). Der frühere Sweet Spot (feste
      Wunschentfernung) und die Startgüte entfallen.
  - **Zielrahmen** (wie im Original: erfasst, näher dran Farbwechsel; keine
    Mindestreichweite): außer Reichweite einfacher Rahmen, in Reichweite der
    gewählten Lenkwaffe kräftige Ecken, blau hervorgehoben, wenn die
    Vorausberechnung trifft,
    gestrichelt „NO EFF“, wenn die Waffe nicht wirkt oder ihr Suchkopf die Klasse
    nicht sieht. Beschädigte Ziele tragen „DMG“.
  - **Wrack:** Nach der Zerstörung bleibt die Aufschaltung auf dem Wrack (Rahmen
    gedämpft, „DESTROYED“, Zielbild zeigt das Wrack), bis der Spieler mit einer
    Richtungstaste ein neues Ziel wählt; Wracks sind dann nicht mehr anwählbar.
    Auf der Karte verschwinden zerstörte Ziele.
  - **Zielhilfe für Bomben (v0.2.6)** nach der Lesephase oben, eigene Gestaltung:
    Bei gewählter Mk 82 / Snakeye und höchstens 60° Querlage zeigt das HUD den
    vorausberechneten Aufschlagpunkt (Kreis mit Punkt) und eine Fall-Linie vom
    Flugbahnsymbol dorthin – liegt der Kreis auf dem Ziel, auslösen. Läge der
    Einschlag im Splitterbereich des weiterfliegenden Flugzeugs, wird alles rot
    und „TOO LOW“ blinkt. Die Vorhersage nutzt dieselbe Flugbahnrechnung wie
    die echte Bombe (20-mal pro Sekunde, grobe Schritte mit gemittelter
    Geschwindigkeit).
  - **Für später vorgemerkt: Auslöseklammer** (Spielerwunsch, im Original
    vorhanden). Eine erste Fassung (v0.2.6: Klammerbreite = Längsablage bis 2 km,
    „RELEASE“-Fenster ±15 m) war im Spiel nicht nutzbar – im groben Maßstab
    wirkte sie geschlossen, lange bevor die Bombe getroffen hätte, und das
    Auslösefenster war kürzer als eine Rechenperiode. Neu entwerfen mit
    zeitbasierter Anzeige (Sekunden bis zum Auslösen) und nichtlinearem
    Maßstab; die Längs- und Querablage liefert der Kern schon
    (`BombAim::target_offset`).
  - **Feuersignal** der Lenkwaffen unten mittig im HUD-Glas: „LOCKED“, wenn
    die Waffe das Ziel übernähme (bis v0.2.9 „IN RANGE“, umbenannt, weil es
    nicht mehr die Entfernung meint – Spielerwunsch), darüber zusätzlich „SHOOT“ (umrahmt), wenn
    die Vorausberechnung trifft (Spielerwunsch v0.2.7, seit v0.2.9 aus dem
    vorausberechneten Flug).
  - **Waffenkamera** (Spielerwunsch v0.2.9): Waffen mit Fernsehsuchkopf
    (Maverick) senden ihr Bild auf die eigene Seite „WPN CAM“ (Kurzname CAM)
    des rechten MFDs; nach dem Abschuss schaltet das MFD selbst dorthin. Das
    Bild kommt von einer zweiten 3D-Kamera an der Waffe (derselbe körnige
    Schwarzweiß-Effekt wie das Zielbild) mit festem, schmalem Sichtfeld von 3°
    wie ein echter Suchkopf – das Ziel wächst im Anflug stetig (v0.2.9, statt
    mitlaufendem Zoom; ein Abbremsen im Endanflug wurde erwogen und verworfen,
    Spielerentscheidung). Blick zur Zielmitte, solange der
    Suchkopf verfolgt („TRACK“, Zielkästchen), sonst in Flugrichtung
    („SEARCH“). Mit dem Einschlag reißt das Bild ab („NO SIGNAL“). Die GBU-12
    hat wie das reale Vorbild nur einen Lasersensor und keine Kamera; ihren
    Einschlag zeigt das Zielbild des Flugzeugs (TGT).
  - **HUD-Warnfarbe blau** (Spielerwunsch v0.2.7): Warnungen und
    Hervorhebungen im HUD (z. B. „TOO LOW“, Zielrahmen bei erwartetem
    Treffer, „SHOOT“) sind blau statt rot; die MFDs bleiben bei Rot.
  - **Karte:** Waffen in der Luft erscheinen auf der Karte, Raketen als kurzer
    Strich in Flugrichtung, Bomben als Punkt; eigene in der Farbe des
    Flugzeugsymbols, feindliche rot (`FlightData::munitions`; feindliche
    Raketen kommen mit Etappe 3, Spielerwunsch v0.2.7).
  - **HUD unten links:** gewählte Waffe mit Anzahl (z. B. „MK-82 X2“) und
    Schacht mit Klappenzustand („BAY 1 SAFE/DOORS/OPEN“, offen rot).
  - Splitter: Schlägt eine Waffe im Splitterbereich des eigenen Flugzeugs ein,
    gibt es vorerst nur die Warnung „FRAG WARNING“ (Schaden kommt mit Etappe 4).
    Die Fallschirmbombe ist genau dafür da: Sie bleibt im Tiefflug weit genug
    zurück.
  - Meldungen unter dem HUD-Glas (z. B. „MK-82 AWAY“, „BAY CLOSED“, „AGM-65 LOCK …“,
    „GBU-12 LASER LOST“, „… DAMAGED“, „… DESTROYED“, „CRITICAL HIT … DESTROYED“,
    „INEFFECTIVE ON …“, „CIVILIAN … DESTROYED“); die WPN-Seite zeigt Bestand je
    Schacht, Klappenzustand (SAFE/DOORS/ARMED) und Bereitschaft.
- **Unsere Werte:** Mk 82 241 kg, Cw·A 0,020 m²,
  Fallschirm 0,9 m² nach 0,3 s, zwei je Schacht, Wirkradius 30 m, Splitterbereich
  180 m. GBU-12 227 kg, Cw·A 0,022 m², Steuerung 6°/s. Abschusskreis ±8°.
  Maverick 210 kg, 700 m/s nach gemächlicher Beschleunigung (70 m/s², bleibt
  länger sichtbar), 25°/s, 16 km, Suchkegel ±25°, Wirkradius 8 m;
  HARM 360 kg, 650 m/s, 20°/s, 30 km, ±35°; Harpoon 520 kg, 270 m/s,
  10°/s, 30 km, ±40°. Aufgabe bei mehr als 135° Ablage. Gemessen (Maverick mit
  Übergabe aus 8 km, flach, über bis zu 120 Karten): gegen weiche Ziele 100 %
  zerstört, gegen Gebäude rund ein Drittel. Bomben: volle
  Wirkung bis 40 % des Wirkradius (vom Zielrand gemessen), dann linear auf null.
  Gehärtet ab 60° Eintreffwinkel. Größenordnungen aus öffentlichen Daten realer
  250-kg-Bomben und leichter Luft-Boden-Flugkörper, Wirkung nach Spielbalance.
  Volle Standardbeladung wiegt knapp 2 t; Landeanflüge in Tests fliegen leer.
- **Status:** Abwurf, Ballistik und Wirkung umgesetzt (v0.2.4); Lenkwaffen,
  gewürfelter Endanflug und Zielrahmen nach Reichweite (v0.2.5); HUD-Zielhilfe für
  Bomben (v0.2.6); vereinfachte Lenkwaffen mit Abschusskreis, Laserbombe,
  Auslöseklammer vorgemerkt (v0.2.7); physische Lenkwaffen mit gewürfelter
  Wirkung und vorausberechnetem „SHOOT“ (v0.2.9)

### M-015 Bordsysteme und Schadensmodell (Gerüst)

- **Verhalten:** Treffer und Splitter beschädigen einzelne Bordsysteme, die danach
  eingeschränkt oder gar nicht mehr arbeiten.
- **Referenzstelle:** f19re `src_en/egcombat.c` (Splitterschaden nach eigenem
  Bombeneinschlag und nach Flugkörpertreffern).
- **Lesephase (eigene Worte, keine Werte):** Ein Treffer setzt zufällig eine oder
  mehrere Schadensmarken aus einer festen, kleinen Liste von Systemen; auf schweren
  Stufen mehr. Das Cockpit zeigt die Schäden auf einer eigenen Anzeige.
- **Lesephase v0.2.9** (`bombTarget`, `egflight.c`): Ein Raketentreffer setzt
  so viele zufällige Schadensmarken aus acht Systemen, wie die Konfliktstufe
  plus eins vorgibt, und zählt einen Trefferzähler hoch. Wirkung je System,
  jeweils mit dem Trefferzähler wachsend: Höchstschub sinkt, die Steuerung
  zittert zufällig (in der Luft), Treibstoff leckt, Raketenwarnung, Karte und
  Störsender fallen aus. Viele Treffer verstärken außerdem das Durchsacken
  beim Strömungsabriss. Einen festen „Lebensbalken“ gibt es nicht – das
  Flugzeug geht über die Folgen verloren (Absturz, kein Treibstoff).
- **Unser Design (Gerüst, Spielerentscheidung):** `f29_core::systems`. Acht Systeme
  (Triebwerk, Treibstoff, Hydraulik, Flugsteuerung, Radar, Optik, Waffenschacht,
  Anzeigen), jedes intakt, eingeschränkt (Restleistung 0..1) oder ausgefallen.
  Vorerst ist alles intakt; das Schadensmodell (Etappe 4) setzt Zustände über
  `Sim::set_condition` mit eigenem Zufallsstrom. Die Wirkung hängt schon jetzt an
  genau einer Stelle je System: Höchstschub, Treibstoffleck, Stellgeschwindigkeit
  von Klappen, Fahrwerk und Schachtklappen (ausgefallen = blockiert), Drehraten
  (ausgefallen = Notsteuerung), Radar- und Optikreichweite (ausgefallenes Radar
  lässt sich nicht einschalten), Waffenschacht (ausgefallen = Klappen klemmen,
  kein Auslösen, Meldung „BAY FAULT“), Anzeigen (flackern, ausgefallen dunkel; der
  mechanische Schubhebel bleibt). Die SYS-Seite zeigt alle acht: intakt gedämpft,
  eingeschränkt gelb mit Restleistung, ausgefallen rot umrahmt. Der Zustand geht in
  den Zustandshash ein.
- **Unsere Werte:** Leck bis 3 kg/s (volle Tanks in knapp 30 min leer),
  Notsteuerung 30 % Drehrate. Welche Treffer welche Systeme wie stark beschädigen,
  folgt mit Etappe 4.
- **Status:** Gerüst umgesetzt (v0.2.4); erste Schadensquelle: Treffer
  feindlicher Raketen beschädigen zwei zufällige Systeme, intakt → halbe
  Leistung → ausgefallen, beschädigtes Triebwerk 4 % Höchstschub je Treffer
  wie im Original (v0.2.9, M-021); Splitter eigener Bomben folgen

### M-016 Bordkanone

- **Verhalten:** Eine fest eingebaute Kanone für Bodenziele im Tiefflug
  (Strafing), unabhängig von Waffenschacht und Klappen.
- **Referenzstelle:** keine (eigener Entwurf nach Spielerwunsch).
- **Unser Design:** eigenes Waffensystem mit gehaltener Feuertaste (Rücktaste,
  Joystick-Taste 1 vorgesehen). Jede Patrone fliegt als Massenpunkt
  (Mündungsgeschwindigkeit plus Flugzeuggeschwindigkeit, Schwerkraft, ohne
  Luftwiderstand) mit kleiner, deterministisch gewürfelter Streuung aus einem
  eigenen Zufallsstrom. Trifft ihre Flugbahn innerhalb eines Ticks die Form
  eines intakten Ziels (senkrechter Zylinder mit Klassenradius und -höhe),
  richtet sie bei **jedem** Ziel denselben Schaden an, ohne kritische Treffer
  und ohne Wirksamkeitstabelle (Spielerentscheidung v0.2.8). Am Boden und nach
  einem Absturz schießt sie nicht. Zerstört die Kanone ein Ziel, meldet das HUD
  „GUN … DESTROYED“; ein neuer Druck bei leerer Kanone meldet „GUN EMPTY“.
  HUD: **Kanonenkreuz** (offenes Kreuz) dort, wo eine jetzt abgefeuerte Patrone
  auf dem Gelände einschlüge – nur innerhalb ihrer Flugzeit; unten links „GUN“
  mit Munitionsstand. 3D-Ansicht: Leuchtspur je Patrone, kleine Wölkchen bei
  Einschlägen (am Ziel rot). Zustand (Munition, Patronen, Zufallsstrom) geht in
  den Zustandshash ein; das Replay-Format bleibt (Bit 3 im Waffenbyte).
- **Unsere Werte:** 600 Schuss, 50 Schuss/s (12 s Dauerfeuer), 1000 m/s
  Mündungsgeschwindigkeit, 3 s Flugzeit (≈ 3 km), Streuung 3 mrad, 2 % Schaden
  je Treffer (50 Treffer zerstören jedes Ziel).
- **Status:** umgesetzt (v0.2.8); Kanone als erste Waffe in der Wahl und
  Feuern mit der Auslösetaste (v0.2.8, siehe M-017)

### M-017 HUD-Modi und Waffenwahl

- **Verhalten:** Das HUD zeigt je nach Aufgabe nur die passende Symbolik.
- **Referenzstelle:** keine (eigener Entwurf nach Spielerwunsch).
- **Unser Design:** drei Modi, mit einer Taste (H) durchgeblättert; der Modus
  ist reine Anzeige und gehört nicht zum Simulationszustand.
  - **NAV** (Start): Flugdaten, Navigationspunkt, Konfiguration; mit
    ausgefahrenem Fahrwerk Gleitlinie und Sinkratenskala. Keine Waffen- und
    Zielsymbolik.
  - **AG** (Luft-Boden): Zielrahmen, Zielerfassung, gewählte Waffe und die
    Zielhilfe je nach Waffe – Kanone: Kanonenkreuz am vorausberechneten
    Bodeneinschlag; ungelenkte Bombe: Bombenkreis; Rakete: Abschusskreis (jetzt
    in der vollen HUD-Farbe statt gedämpft); Lenkwaffen: Feuersignal.
  - **AA** (Luft-Luft): ein großer Boresight-Kreis (±10°), einheitlich für alle
    Luft-Luft-Waffen und immer sichtbar.
  Das HUD-Glas ist breiter (Daten und Symbole weiter außen). Die Waffenwahl
  (Tab) läuft GUN → BAY 1 → … → BAY 4 → GUN; die Kanone ist die Startwahl. Die
  Auslösetaste feuert bei gewählter Kanone, solange sie gehalten wird (eigenes
  Bit im Waffenbyte), sonst löst sie die Schachtwaffe aus. Die Rücktaste feuert
  die Kanone unabhängig von der Wahl. Die Wahl geht in den Zustandshash ein.
- **Status:** umgesetzt (v0.2.8)

### M-020 Radar-/Detektionsmodell (Stealth)

- **Verhalten:** Wie leicht der Gegner das Flugzeug entdeckt, hängt von der eigenen
  elektromagnetischen Sichtbarkeit (Lage, Waffenschacht, Radar an/aus), der Entfernung
  und der Höhe bzw. Geländemaskierung ab. Das Cockpit zeigt die eigene Sichtbarkeit und
  die Reichweite gegnerischer Radare an.
- **Referenzstelle:** f19re `src_en/egcombat.c` (Bewertung je Bedrohung, Alarmstufen),
  `src_en/egframe.c` (eigene Sichtbarkeit pro Bild).
- **Lesephase v0.2:** Die eigene Sichtbarkeit ist eine Zahl, die pro Bild aus Flughöhe
  (höher = sichtbarer), Querlage, offenem Waffenschacht und eingeschaltetem
  Störsender wächst. Jede Radarstellung bewertet das Flugzeug mit einem Wert, der mit
  der Entfernung linear fällt, von der Geländeart unter dem Flugzeug abhängt (grobe
  Maskierung über Kartenzellen, keine echte Sichtlinie) und bei Doppler-Radaren vom
  Aspekt. Übersteigt Bewertung plus Sichtbarkeit eine Schwelle, steigt die Alarmstufe
  der Stellung; ab einer zweiten Schwelle meldet sie das Flugzeug, später schießt sie.
  Der Störsender erhöht die eigene Sichtbarkeit, halbiert aber das Anwachsen der
  Alarmstufe und lenkt radargelenkte Flugkörper ab; ein zweiter Störer wirkt gegen
  infrarotgelenkte. Die Bedrohungsanzeige im Cockpit zeigt Bewertung gegen
  Sichtbarkeit als zwei Balken.
- **Unser Design:** `crates/f29-core/src/radar.rs`. Spielerentscheidungen v0.2.9:
  Radare sehen **durch das Gelände** (keine Sichtlinie, wie unsere eigenen
  Sensoren); feste **Stufenleiter** statt fein dosierter Alarmzuwächse.
  - **Stellungen:** SAM-Stellungen (Dopplerradar, mittlere Reichweite) und
    Frühwarnradare (Impulsradar, große Reichweite, neue Zielklasse abseits der
    Route). Eine zerstörte Stellung strahlt nicht mehr und vergisst.
  - **Pings:** Jede Stellung pingt einmal je Antennenumlauf (versetzt je
    Stellung). Nur dann wird gerechnet.
  - **Impulsstärke** am Flugzeug: fällt linear mit der Entfernung bis zur
    Reichweitengrenze. Doppler: schwach, wenn das Flugzeug quer zur Stellung
    fliegt (nur die Bewegung auf sie zu oder von ihr weg zählt). Impuls:
    richtungsunabhängig. Der Boden ist kein Sonderfall (Spielerentscheidung
    v0.2.9: auch gelandet kommen Pings an); feindliche Radare stehen
    mindestens 35 km von der Heimatbasis entfernt, damit das Anrollen nicht
    schon entdeckt wird.
  - **Stealth-Level** = 1 − eigene Sichtbarkeit, **ein Wert für alle
    Radare** wie im Original (Umbau v0.2.9 nach dem zweiten Spieltest; die
    erste Fassung rechnete je Radar aus der Blickrichtung auf Bug, Flanke oder
    Bauch – die Querlage wirkte kaum, die Höhe gar nicht). Die Sichtbarkeit
    setzt sich zusammen aus einem Grundwert, der **Höhe über Grund** (bis
    100 m am unauffälligsten, darüber zunächst schnell, dann immer langsamer
    steigend wie im Original), der **Querlage** (linear bis 90°, Rückenflug wie
    waagerecht), dem offenen Schacht und dem eigenen Radar (ein kleiner Verlust
    wie der offene Schacht). Nicklage, Fahrwerk und Klappen zählen nicht – das
    Original kennt sie nicht. Ganz unten, auch am Boden, bleibt der
    Grundwert: höchstens 90 %, ganz unsichtbar ist das Flugzeug nie. Kurz:
    tief und waagerecht fliegen.
  - **Alarmstufe:** Ein Ping, dessen Impuls stärker ist als das Stealth-Level,
    hebt die Stufe der Stellung um eins: 1 „RADAR DETECTED“ (Stellung mit
    Reichweitenkreis auf der Karte), 2 „TRACKED“ (Ping-Ort gemerkt – Ziel
    späterer Patrouillen), 3 „RADAR ID“ (Auslöser für den Raketenstart, auch
    beim Frühwarnradar mit Langstreckenraketen; kommt mit dem
    Schadensmodell). Ohne Treffer sinkt die Stufe nach 30 s um eins; bei 0
    „LOST CONTACT“.
  - **Anzeigen:** SIG-Säule zwischen den MFDs mit **einem** Balken für das
    Stealth-Level (Spielerwunsch v0.2.9). Jeder Ping erscheint für eine
    Sekunde als Querstrich in seiner Stärke – rot und über dem Balken ist ein
    Treffer (Alarmstufe dieses Radars +1), gelb darunter keiner; der Balken
    folgt dabei weiter dem aktuellen Stealth-Level. Drei Alarmlampen, Radartyp PLS/DOP, Pieps des
    Radarwarnempfängers (Treffer doppelt und höher), Reichweitenkreise und
    gemerkter Ping-Ort auf der Karte. Der Zustand geht in den Zustandshash ein.
- **Unsere Werte:** Frühwarnradar 60 km, Umlauf 10 s; SAM 30 km, Umlauf 5 s.
  Sichtbarkeit: Grundwert 0,10; Höhe bis 0,40 (Arkustangens über
  (Höhe − 100 m) / 1500 m: 1000 m ≈ 0,14, 3000 m ≈ 0,28); Querlage bis 0,30
  bei 90°; Schacht 0,15; eigenes Radar 0,15. Doppler im reinen Querflug 25 %.
  Beispiele: Tief, waagerecht und clean (Stealth 90 %) entdeckt eine
  SAM-Stellung das Flugzeug erst unter 3 km, ein Frühwarnradar unter 6 km; in
  3000 m (Stealth ≈ 62 %) unter etwa 11 bzw. 23 km; dort mit 90° Querlage
  (≈ 32 %) unter etwa 20 bzw. 41 km.
- **Testoption** `--no-stealth` (Spielerwunsch v0.2.9): Stealth-Level immer 0,
  jeder Ping in Reichweite trifft – zum Testen von Alarmstufen, Raketen und
  Schäden. Steht im Replay (`stealth on|off`, Format 6).
- **Status:** Entdeckung umgesetzt (v0.2.9); Raketen, Patrouillen und
  Störsender folgen.

### M-021 Boden-Luft-Bedrohungen

- **Verhalten:** Radargeführte Flugabwehrraketen bedrohen das Flugzeug abhängig
  von Detektion und Reichweite. Flak gibt es nicht (gestrichen, Spielerentscheidung
  v0.2.9).
- **Referenzstelle:** f19re `src_en/egcombat.c` (Flugkörper der Bedrohungen),
  `src_en/egframe.c` (Täuschkörper).
- **Lesephase v0.2:** Täuschkörper (Fackeln, Düppel) und ein Störsender sind begrenzt
  vorrätig; ein Täuschkörper lockt Flugkörper der passenden Lenkart an, solange er in
  ihrem Suchkegel liegt.
- **Lesephase v0.2.9** (`egcombat.c`: `fireGroundThreat`,
  `updateThreatTargeting`, `samCanAcquireTarget`, `bombTarget`; `egframe.c`:
  `countermeasures`; `egflight.c`: Wirkung der Schäden):
  - **Start:** Erst wenn die Alarmstufe einer Stellung fast voll ist, meldet
    sie „Radar ID“; bei den folgenden Bewertungen schießt sie, sofern das
    Flugzeug innerhalb der Aufschaltreichweite ihres Raketentyps ist, ihr
    Startplatz frei ist (eine Rakete je Stellung in der Luft) und insgesamt
    nicht schon zu viele Raketen fliegen (Grenze steigt mit der
    Konfliktstufe). Reine Radarstellungen ohne Werfer schießen nicht.
  - **Raketentypen:** je Typ Aufschaltreichweite, Höchstgeschwindigkeit,
    Drehrate und Lenkart (radar- oder infrarotgelenkt, verschiedene Klassen).
    Die Flugzeit ist begrenzt, etwa so lang, wie die Rakete für ihre
    Aufschaltreichweite braucht.
  - **Flug:** Die Rakete dreht in Kurs und Steigwinkel auf das Ziel, begrenzt
    durch ihre Drehrate und zusätzlich durch die Konfliktstufe (auf niedrigen
    Stufen träger). Sie lenkt nur auf Ziele in einem Suchkegel von etwa ±22°
    um ihre Flugrichtung; liegt das Ziel weit hinter ihr und ist es weiter weg
    als ein Flugschritt, gibt sie auf.
  - **Halbaktive Lenkung:** Ist die Rakete noch weit vom Ziel entfernt
    (mehrere km), lenkt sie nur, solange ihre Stellung das Flugzeug weiter
    verfolgt (Alarmmarke der Stellung). Verliert die Stellung den Kontakt,
    fliegt die Rakete ungelenkt weiter – Stealth wirkt also auch während des
    Anflugs.
  - **Treffer:** rein geometrisch, nicht gewürfelt – kommt die Rakete näher als
    ihren Flugschritt (Höhe gewichtet), trifft sie. Dazu Raketenwarnung:
    Leuchte und Warnton, wenn eine gelenkte Rakete in der Nähe ist.
  - **Täuschkörper:** zwei Arten (gegen Radar- bzw. Infrarotlenkung) in
    begrenztem Vorrat, je bis zu drei gleichzeitig. Ein Täuschkörper bleibt
    am Abwurfort stehen und brennt einige Sekunden (auf höheren Stufen
    kürzer). Liegt er im Suchkegel einer Rakete der passenden Lenkart, lenkt
    sie auf ihn statt auf das Flugzeug – kein Würfeln. Leerer Vorrat meldet
    „erschöpft“.
  - **Störsender:** begrenzt, läuft deutlich länger als ein Täuschkörper;
    radargelenkte Raketen verlieren außerhalb kurzer Entfernung die
    Aufschaltung, und die Alarmstufen der Stellungen steigen nur halb so
    schnell (M-020). Ein Störer für Infrarot wirkt entsprechend.

- **Unser Design (v0.2.9, nach dem Original):** Die feindlichen Raketen sind
  dieselben physikalischen Flugkörper wie die eigenen (M-014) – gleiche Liste,
  gleicher Flug, gleicher Annäherungszünder; Ziel ist das Flugzeug.
  - **Start:** Jeder weitere Treffer einer Stellung, die schon auf Stufe 3
    („RADAR ID“) steht, ist ein Startwunsch. Gestartet wird, wenn das Flugzeug
    in der Reichweite des Raketentyps ist, die Stellung keine Rakete mehr in
    der Luft hat und nicht schon vier feindliche fliegen. Die Rakete hebt 10°
    über der Linie zum Flugzeug ab (im Suchkegel) und lenkt nach 0,4 s ein.
    Meldung „MISSILE LAUNCH SA-6 - <Stellung>“.
  - **Drei Typen** (Spielerentscheidung v0.2.9), je mit eigenem Startprofil:

    | | Kurzstrecke SA-8 Gecko | Mittelstrecke SA-6 Gainful | Langstrecke SA-5 Gammon |
    |---|---|---|---|
    | Stellung | Kurzstrecken-Flugabwehr (neu, Fahrzeug, bewacht Zielgruppen) | SAM-Stellung | Frühwarnradar |
    | Radar | Doppler 12 km, Umlauf 2,5 s | Doppler 30 km, 5 s | Impuls 60 km, 10 s |
    | Reichweite | 10 km | 25 km | 60 km |
    | Geschwindigkeit | 900 m/s | 650 m/s | 450 m/s |
    | Drehrate | 30°/s | 22°/s | 10°/s |
    | Start | direkt aufs Ziel | 45°, 1,5 s Boost | 85° (fast senkrecht wie im Original), 4 s Boost |
    | Bogen | 0 | 0,1 | 0,25 |
    | Wirkradius / Flugzeit | 12 m / 20 s | 20 m / 50 s | 30 m / 150 s |

    Die Kurzstrecke ist kaum auszukurven (nur Reichweite und Stealth
    schützen), die Mittelstrecke der Allrounder, die Langstrecke kommt über
    jeden Berg und aus großer Entfernung, ist aber lange unterwegs – Zeit für
    Täuschkörper (Spielerwunsch). Suchkegel aller ±22° wie im Original.
  - **Startprofil:** Die Rakete startet im Startwinkel ihres Typs (mindestens
    10° über der Linie zum Flugzeug) und steigt im **Boost** ungelenkt. Danach
    lenkt sie mit **Bogen** wie im Original: Sie zielt um Bogenfaktor ×
    Entfernung über das Ziel, der Zuschlag schrumpft beim Annähern auf null.
    Suchkegel und Aufgabe gelten wie im Original nur in der Peilung
    (waagerecht) – so findet auch eine steil gestartete Rakete ihr Ziel.
  - **Munition:** unbegrenzt wie im Original, eine Rakete je Stellung in der
    Luft (Spielerentscheidung).
  - **Lenkung halbaktiv**, mit **Vorhalt** auf den Treffpunkt (Flugzeugort
    plus Geschwindigkeit × Restflugzeit, höchstens 10 s): Abweichung vom
    Original, das auch hier rein verfolgt – im Spieltest rettete sonst das
    Hineindrehen in die Rakete, im Luftkampf das Falscheste (Spielerwunsch
    v0.2.9). Der Vorhalt gilt für **alle Lenkwaffen** (gemeinsamer Flug): Bei
    stehenden Zielen ergibt er die reine Verfolgung des Originals, bei
    bewegten (feindliche Flugzeuge, Schiffe, Kolonnen – später) den
    Kollisionskurs. Ein **Vorhaltfaktor** je Waffe (0 = reine Verfolgung,
    1 = voller Vorhalt) stellt die Gefährlichkeit ein – vorgesehen auch für
    Schwierigkeitsgrade; vorerst haben alle Waffen 1 (Spielerwunsch). Eine „Pitbull“-Phase mit mehr Leistung im Endanflug gibt es im
    Original nicht; nur die Unabhängigkeit von der Stellung auf kurze
    Entfernung (unten). Weiter als
    5 km lenkt die Rakete nur, solange ihre Stellung das Flugzeug beim
    **letzten Antennenumlauf erfasst** hat (Ping-Treffer, „painted“) – Stealth
    wirkt also auch während des Anflugs: tief, waagerecht, Schacht zu, quer zum
    Dopplerradar. Näher lenkt ihr eigener Suchkopf. In jedem Fall nur, wenn das
    Flugzeug im Suchkegel liegt; sonst fliegt sie geradeaus weiter. Liegt es
    mehr als 135° hinter ihr, gibt sie auf.
  - **Treffer** geometrisch (Annäherungszünder) – Folgen **nur über die
    Bordsysteme** (M-015): Jeder Treffer trifft zwei zufällige, verschiedene
    Systeme; ein intaktes fällt auf halbe Leistung, ein eingeschränktes fällt
    aus. Ausnahme Triebwerk, genau wie im Original: Es wird nur als
    „beschädigt“ markiert und fällt nie aus; ab dann kostet jeder
    Raketentreffer 4 % Höchstschub (Gesamtzahl der Treffer, frühere zählen
    mit – nach fünf Treffern also 80 %). Wiederholte Treffer verschärfen so
    die Folgen wie im Original mit der Trefferzahl. Kein Lebensbalken – das Flugzeug geht über die Folgen
    verloren. Meldung „SA-6 HIT“.
  - **Schubhebel:** Ist das Triebwerk beschädigt, sperrt eine rote Marke den
    nicht mehr verfügbaren Bereich der Kulisse, daneben „MAX 92“ in Rot
    (Spielerwunsch v0.2.9).
  - **Raketenwarnung:** Solange eine feindliche Rakete das Flugzeug verfolgt,
    blinkt „MISSILE“ im HUD und ein schneller Wechselton warnt. Auf der Karte
    erscheinen feindliche Raketen als roter Strich, in der 3D-Ansicht mit
    Rauchspur.
  - Täuschkörper folgen mit Etappe 4 (Düppel auf „.“, Fackeln später mit den
    Infrarotraketen).
- **Status:** Boden-Luft-Raketen umgesetzt (v0.2.9), Täuschkörper geplant

### M-031 Einsatzgebiete und Missionsgenerator

- **Verhalten:** Das Original hat vier feste Einsatzgebiete (Libyen, Persischer
  Golf, Nordkap, Mitteleuropa) mit fester Geografie. Jede Mission wird in einem
  Gebiet aus dessen festen Objekten erzeugt.
- **Referenzstelle:** f19re `src_start/stgen.c` (`missionGenerate`,
  `parseWorld`, `runGenerator`, `findOrPlaceItem`, `formatGridRef`),
  `src_start/stparse.c` (Geländekacheln), `src_en/egcombat.c` (Rasterzellen in
  der Radarbewertung, `spawnSamThreat`). Die eigentlichen Gebietsdateien
  (`*.wld`, `*.3dT`, `*.xxx`) liegen dem Rekonstruktionsprojekt nicht bei;
  ausgewertet wurden nur Aufbau, Maßstab und Regeln, keine Positionen.
- **Lesephase v0.2.9:**
  - Je Gebiet eine Weltdatei, eine Geländekacheldatei und ein Kartenbild.
  - Die Karte ist ein Quadrat von 32 768 internen Einheiten; die
    Entfernungsanzeige rechnet 64 Einheiten je km (in der untersuchten,
    übersetzten Fassung), das Gebiet wäre also rund 500 km breit – gut doppelt
    so groß wie unsere Karte.
  - Für Briefings ist sie in 20 × 20 Planquadrate (je rund 25 km) mit
    Buchstaben-Ziffern-Kennung geteilt, je Gebiet mit eigenem Versatz.
  - Ein Grobraster von 16 × 16 Zellen (je rund 32 km) trägt je Zelle wenige
    Merkmale: Land oder Wasser (Missionsziele nur auf Land), eine Geländeart,
    die die Radarbewertung direkt verstärkt oder schwächt (die grobe
    „Maskierung“ des Originals, ohne Sichtlinie), und ein Merkmal, in dem
    tagsüber spontan eine tragbare Flugabwehrrakete in der Nähe starten kann.
  - Die Weltdatei enthält eine Objektliste mit Koordinaten, Typ und Merkmalen:
    zielfähige Anlagen, Startbasen (eigene Flugplätze und Flugzeugträger, die
    in Deckshöhe starten), Bodenstellungen mit Radar und SAM-Typ sowie
    Flugzeugeinheiten (Patrouillen mit Basis, Typ und Treibstoff).
  - Geländekacheln: bis zu 32 Kacheltypen in fünf Detailstufen – fertige
    Ausschnitte (Orte, Anlagen, Hügel), die in Rasterzellen gesetzt werden;
    Zufallsziele „rasten“ am nächsten solchen Geländemerkmal ein.
  - Missionsgenerator: Primärziel aus der Objektliste oder ein Zufallspunkt
    auf Land, der zum nächsten Geländemerkmal rutscht; ebenso das
    Sekundärziel; beide höchstens rund 200 km auseinander. Startbasis: eine
    eigene Basis nahe den Zielen, mit etwas Zufall und Vorrang für Träger. Die
    Gesamtstrecke muss in einem Band liegen, sonst wird neu gewürfelt (bis zu
    1000 Versuche). Welcher Missionstyp zum Ziel passt, regelt eine Tabelle
    nach Zielklasse, Gebiet und Kampagnenstufe; für ein Gebiet gibt es
    zusätzlich fest vorgegebene Missionen.
- **Unser Design (Entwurf, v0.3):**
  - **Eigene, fiktive Einsatzgebiete als Daten** (z. B.
    `data/theaters/*.ron`): Seed und Größe (rund 400–500 km), ein Grobraster
    16 × 16 (Land/Wasser, Geländeart flach/hügelig/Gebirge, Zugehörigkeit
    eigen/neutral/feindlich), das unser prozedurales Gelände steuert (Küsten,
    Gebirge), und eine Objektliste: Heimatbasen, ein Träger, geheime Pisten,
    Radarnetz (Frühwarnradare an Grenze und Küste, SAM-Gürtel um wichtige
    Anlagen), Städte und Dörfer als Gruppen ziviler Gebäude, Häfen, Brücken,
    Kraftwerke, feindliche Flugplätze, später Patrouillenbasen.
  - **Missionsgenerator nach dem Vorbild des Originals:** Ziele aus der
    Objektliste (oder Zufallspunkt, der zum nächsten Ort rutscht), nächste
    eigene Startbasis, Strecke in einem Band; die Route folgt aus Basis und
    Zielen – nicht mehr wie bisher Ziele um eine Zufallsroute.
  - **Geländeart als Radarfaktor** (optional): billiger Ersatz für die
    gestrichene Sichtlinie, nahe am Original – im Gebirge schwächerer, über
    flachem Land und Wasser stärkerer Impuls.
  - **Planquadrate** auf Karte und Missionsseite.
  - Technik: Floating Origin und Kachelstreaming tragen; die
    Kartenraster brauchen größere Ausschnitte; Schleifen über viele
    feste Objekte bekommen eine Vorauswahl nach Rasterzellen.
  - Keine Positionen aus dem Original (sie liegen auch nicht vor): nur die
    Struktur (Raster, Objektliste, Generatorregeln) wird nachgebaut.
- **Status:** Lesephase abgeschlossen (v0.2.9), Umsetzung geplant (v0.3)

### M-030 Karriere, Szenarien, Orden

- **Verhalten:** Ein Pilot fliegt eine Folge von Missionen in mehreren Einsatzgebieten
  und bei unterschiedlichen Konfliktstufen. Er sammelt Punkte, steigt im Rang auf, erhält
  Orden und kann gefangen genommen werden oder fallen.
- **Referenzstelle:** *offen (v0.5).*
- **Unser Design (Entwurf):** Eigene, fiktive Einsatzgebiete, eigene Ränge und eigene
  Auszeichnungen mit eigenen Namen. Die Wertung ist datengetrieben
  (`data/career.ron`), der Spielstand versioniert serialisiert.
- **Status:** geplant (v0.5)
