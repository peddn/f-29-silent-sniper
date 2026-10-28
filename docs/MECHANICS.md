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
| | Namen: Spiel, Flugzeug, Szenarien, Figuren, Orden des Originals |

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
  - Abrissgeschwindigkeit im Horizontalflug ≈ 66 m/s (≈ 130 kt) auf Meereshöhe
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
- **Status:** umgesetzt (v0.1.4)

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
  werden geglättet, damit es rüttelt statt zittert. Schwierigkeitsstufen,
  Fahrwerk und Schäden folgen später.
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
  mit `flap_rate` aus; oberhalb von `flap_max_speed_mps` (≈ 250 kt) fahren sie
  selbsttätig ein. Bedienung über Taste F, Anzeige „FLAPS“ im Instrumentenbrett.
- **Unsere Werte:** `crates/f29-data/src/aircraft.rs` (Abrissgeschwindigkeit mit
  Klappen ≈ 80 % von ohne).
- **Status:** umgesetzt (v0.1.4)

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
- **Referenzstelle:** *offen, wird in der Lesephase zu v0.2 ermittelt.*
- **Unser Design (Entwurf):** Eigene Zielklassen (z. B. weich, gepanzert, gehärtet,
  Schiff, Radar/Elektronik, Flugzeug am Boden) und eine eigene Wirkungsmatrix als
  Datendatei (`data/weapons.ron`). Ein Validierungstest stellt sicher, dass die Matrix
  vollständig ist.
- **Unsere Werte:** werden in v0.2 festgelegt und hier begründet.
- **Status:** geplant (v0.2)

### M-011 Einschlagdistanz und Schwierigkeitsfaktor

- **Verhalten:** Ob ein Ziel zerstört wird, hängt von der Entfernung zwischen
  Einschlagpunkt und Ziel ab. Der zulässige Abstand skaliert mit dem Schwierigkeitsgrad.
- **Referenzstelle:** *offen (v0.2).*
- **Unser Design (Entwurf):** Wirkradius je Waffe aus der Sprengladung; die
  Trefferwahrscheinlichkeit fällt mit dem Abstand stetig ab, statt an einer harten
  Schwelle umzuschlagen. Der Schwierigkeitsgrad skaliert den Wirkradius.
- **Status:** geplant (v0.2)

### M-012 Gehärtete Ziele: Spezialmunition und Toss-Bombing

- **Verhalten:** Gehärtete Ziele lassen sich nur mit spezieller Munition zerstören. Die
  Bombe muss im Toss-Bombing-Verfahren (Hochwurf aus dem Steigflug) und aus einer
  vorgegebenen Anflugrichtung (Norden) ins Ziel gebracht werden.
- **Referenzstelle:** *offen (v0.2).*
- **Unser Design (Entwurf):** Eintreffwinkel und Eintreffgeschwindigkeit der Bombe sind
  Teil der Wirkungsberechnung. Ein gehärtetes Ziel verlangt einen steilen Eintreffwinkel,
  der sich praktisch nur durch Toss-Bombing erreichen lässt. Die erlaubte
  Anflugrichtung ist eine **Eigenschaft des Ziels** in den Szenariodaten (z. B.
  Bunkereingang in eine Richtung), nicht global „Norden“. Die Bombenballistik ist echte
  Simulation im Kern, kein Würfelwurf.
- **Status:** geplant (v0.2)

### M-020 Radar-/Detektionsmodell (Stealth)

- **Verhalten:** Wie leicht der Gegner das Flugzeug entdeckt, hängt von der eigenen
  elektromagnetischen Sichtbarkeit (Lage, Waffenschacht, Radar an/aus), der Entfernung
  und der Höhe bzw. Geländemaskierung ab. Das Cockpit zeigt die eigene Sichtbarkeit und
  die Reichweite gegnerischer Radare an.
- **Referenzstelle:** *offen (v0.2).*
- **Unser Design (Entwurf):** Radarquerschnitt abhängig vom Aspektwinkel (Bug, Flanke,
  Unterseite) plus Zuschläge für offenen Schacht und eigene Emissionen. Die Detektion
  folgt der Radargleichung (Reichweite ∝ RCS^¼) mit Sichtlinienprüfung gegen das Gelände.
  Erfassung als Zustandsmaschine (Suche → Verfolgung → Abschuss) mit Zufallsanteilen aus
  einem eigenen RNG-Strom.
- **Status:** geplant (v0.2)

### M-021 Boden-Luft-Bedrohungen

- **Verhalten:** Radargeführte Flugabwehrraketen und Flak bedrohen das Flugzeug abhängig
  von Detektion und Reichweite.
- **Referenzstelle:** *offen (v0.2).*
- **Unser Design (Entwurf):** Eigene, fiktive Systemklassen mit Reichweiten aus
  öffentlichen Größenordnungen. Lenkflugkörper werden mit Proportionalnavigation
  simuliert; Täuschkörper wirken über eigene Wahrscheinlichkeitsmodelle.
- **Status:** geplant (v0.2)

### M-030 Karriere, Szenarien, Orden

- **Verhalten:** Ein Pilot fliegt eine Folge von Missionen in mehreren Einsatzgebieten
  und bei unterschiedlichen Konfliktstufen. Er sammelt Punkte, steigt im Rang auf, erhält
  Orden und kann gefangen genommen werden oder fallen.
- **Referenzstelle:** *offen (v0.5).*
- **Unser Design (Entwurf):** Eigene, fiktive Einsatzgebiete, eigene Ränge und eigene
  Auszeichnungen mit eigenen Namen. Die Wertung ist datengetrieben
  (`data/career.ron`), der Spielstand versioniert serialisiert.
- **Status:** geplant (v0.5)
