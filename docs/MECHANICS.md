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
- **Unser Design:** Das Original rechnet mit einer Geschwindigkeitsschwelle. Wir
  bilden das Verhalten physikalisch über den **Anstellwinkel** ab; die
  Abhängigkeit von Höhe (Luftdichte) und g-Last ergibt sich dabei von selbst
  (v_Abriss ∝ √(n/ρ)).
  - kein Anstellwinkelbegrenzer, nur eine Warnung kurz vor dem Abrisswinkel
  - **Hysterese:** Abriss beim Überschreiten von `alpha_stall_rad`, Wiederanlegen
    der Strömung erst unter `alpha_reattach_rad`
  - im Abriss: Auftrieb auf `stall_lift_factor`, zusätzlicher Widerstand,
    Ruderwirkung auf `stall_control_factor`, Nickabfall mit Mindestrate
    `stall_pitch_break` plus Anteil proportional zur Abrisstiefe
  - Der Autopilot schützt sich selbst (Energie- und Anstellwinkelschutz), das
    Flugzeug tut es nicht.
- **Unsere Werte:** `crates/f29-data/src/aircraft.rs`. Abgestimmt über die
  Flugtests `hard_pull_stalls_sinks_and_recovers_when_released`,
  `hard_turn_bleeds_speed`, `steep_zoom_climb_ends_in_stall` und
  `stall_warning_precedes_stall`. Keine Werte aus der Referenz übernommen.
- **Status:** umgesetzt (v0.1.1)

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
