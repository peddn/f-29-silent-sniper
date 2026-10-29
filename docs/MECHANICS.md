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
- **Status:** umgesetzt (v0.1.4)

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
  - Gemessen: Bei 135 kt und halbem Zug kippt das Flugzeug ohne Klappen auf
    −78° ab; mit Klappen bleibt die Nase oben, die Nickrate ist doppelt so hoch,
    die Rollrate bei 140 kt steigt von 44°/s auf 126°/s. Bei 450 kt und 8 s am
    g-Begrenzer bleiben mit Klappen ≈ 25 kt mehr Fahrt.
- **Unsere Werte:** `crates/f29-data/src/aircraft.rs` (Abrissgeschwindigkeit mit
  Klappen ≈ 85 % von ohne).
- **Status:** umgesetzt (v0.1.4, erweitert v0.1.8)

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
    *Gitter* mit Kartengitter, Küstenlinie und Höhenlinien (400/900/1500 m,
    Marching Squares über ein 500-m-Raster) und *Satellit* (Farbraster
    2048², ≈ 98 m/px, mit Hangschattierung, im Hintergrund berechnet, pro Bild
    kursbezogen und pixelig ausgeschnitten). In beiden Modi: Flugplätze, Route
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
    „BRAKES“.
  - Die bisherigen Rundinstrumente (Drehzahl, HSI, Horizont) entfallen und kommen
    später als eigene MFD-Seiten zurück.
  - Ansichtstasten liegen auf Shift + F-Taste (Cockpit, Verfolger, Retro-Look,
    HUD).
- **Status:** umgesetzt (v0.1.10)

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
  - **Abrisswarnung:** gesprochenes „STALL“ alle 1,7 s, solange die Warnung
    anliegt. Die Stimme kommt aus einem kleinen Formant-Synthesizer (Laute S, T,
    AO, L; Stimmquelle durch drei Formant-Resonatoren, Zischlaute aus gefiltertem
    Rauschen) mit grober Abtastung und Quantisierung wie bei frühen
    Sprachchips. Sie lässt sich durch eine eigene Aufnahme
    (`assets/sounds/stall.wav`) ersetzen.
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
  einem eigenen RNG-Strom. Ausgefahrene Klappen (M-007) und Fahrwerk erhöhen den
  Radarquerschnitt – der Preis für die bessere Kurven- und Langsamflugleistung.
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
