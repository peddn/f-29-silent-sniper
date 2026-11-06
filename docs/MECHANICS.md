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
- **Einheiten:** Der Kern rechnet in SI, alle Anzeigen sind metrisch (km/h, m,
  m/s, km, kg bzw. kg/min).

## 5. Mechanik-Protokoll

Jeder Eintrag beschreibt den heutigen Stand; den Verlauf zeigt git.

- **Original:** das Verhalten des Originals in eigenen Worten, ohne Werte
- **Referenzstelle:** Datei/Routine in f19re, oder „keine“
- **Unser Design:** wie wir es umsetzen, mit Ort im Code, Werten und Gründen
- **Abweichungen:** was wir anders machen als das Original und warum
- **Status:** umgesetzt / teilweise (was fehlt) / geplant

### M-001 Flugmodell

- **Original:** Das Flugzeug fliegt gutmütig und arcadenah, verliert in engen Kurven
  Energie, reißt bei zu hohem Anstellwinkel ab und kann ins Gelände stürzen.
- **Referenzstelle:** keine (eigenständiger Entwurf).
- **Unser Design:** `crates/f29-core/src/flight.rs`, Werte in
  `crates/f29-data/src/aircraft.rs` (`F29_SPEC`, SI).
  - **Kräfte:** 3D-Bilanz aus Auftrieb über CL(α), Widerstand CD0 + k·CL²,
    Seitenkraft, Schub ∝ (ρ/ρ0)^0,75 und Gewicht; Masse = Leermasse + Treibstoff
    (M-009) + Waffen; gegen Ende des Flugs ist es leichter, steigt besser und reißt
    später ab. Ein Staudruckwiderstand des Triebwerks im Leerlauf lässt Gas
    wegnehmen deutlich bremsen.
  - **Steuerung:** Fly-by-Wire mit Ratenkommando, g-Begrenzer, Turn-Koordination und
    Gierdämpfer, bewusst **ohne** Anstellwinkelbegrenzer (M-004). Ein losgelassener
    Knüppel hält die Lage der Nase; für den Horizontalflug muss sie um den
    Anstellwinkel über dem Horizont stehen, maßgeblich für die Bahn ist der Flight
    Path Marker.
  - **Werte:** schweres zweistrahliges Angriffsflugzeug ohne Nachbrenner: 16 300 kg
    leer, 70 m², 110 kN, hoher induzierter Widerstand der facettierten Zelle
    (k = 0,15), +7/−2,5 g, Rollrate 2,6 rad/s (≈ 150°/s). Damit: Abrissfahrt auf
    Meereshöhe mit vollen Tanks ≈ 64 m/s (125 kt); Schub/Gewicht ≈ 0,52, begrenztes
    Steigvermögen, ein steiler Steigflug endet im Abriss; 7 g erst ab ≈ 170 m/s,
    darunter reißt voller Zug vorher ab; nach 8 s am g-Begrenzer ab 450 kt bleiben
    ≈ 374 kt; in 3000 m bringen 10 s Vollgas ab 200 m/s +22 m/s, Leerlauf −31 m/s;
    Höchstfahrt in Bodennähe ≈ Mach 0,96, in 3000 m knapp Überschall (≈ Mach 1,02):
    bewusst kein Widerstandsanstieg nahe Mach 1 (Entscheidung des Spielers).
- **Abweichungen:** Kräftemodell statt des Fahrt-und-Lage-Modells des Originals
  (M-004).
- **Status:** umgesetzt.

### M-004 Strömungsabriss

- **Original:** Aggressives Fliegen ist gefährlich (der Spieler erlebte es als
  senkrechtes Absacken). Die Flugroutine ist ein Fahrt-und-Lage-Modell. Die *Fahrt*
  folgt verzögert einer Zielfahrt, die mit dem Schub steigt, mit der Nicklage sinkt
  (Nase hoch kostet, Nase runter bringt Fahrt), mit der Höhe etwas wächst, mit
  Treibstoffgewicht, Fahrwerk und Klappen sinkt und vor allem linear mit der g-Last
  fällt, bis auf null bei maximaler g-Last: Ziehen, harte Kurven und Steigen kosten
  rasch Fahrt. Die *g-Last* ist das Lastvielfache einer horizontalen Kurve bei der
  Querlage (Tabelle) plus Ziehen – wer nur rollt, zieht automatisch die passende
  g-Last; Ziehen ist gekappt, Drücken wirkt halb. Die *Abrissgeschwindigkeit* wächst
  mit der Höhe und mit der Wurzel der g-Last, Klappen senken sie. *Durchsacken:* Die
  Bahn liegt um einen Winkel unter der Nase, der mit dem Verhältnis Abriss- zu
  aktueller Fahrt (begrenzt) und dem Kosinus der Querlage wächst; nahe und unter der
  Abrissfahrt sackt das Flugzeug mit erhobener Nase durch. *Abriss:* Unter der
  Abrissfahrt wird die Nicklage in jedem Bild proportional zum Fahrtdefizit nach
  unten gedrückt, auf dem höchsten Schwierigkeitsgrad und bei Beschädigung doppelt
  so stark, dazu ein Warnton. Es gibt keinen Abrisszustand und keine Hysterese:
  Reicht die Fahrt wieder, endet das Drücken sofort und stetig. Das „senkrechte
  Absacken“ entsteht aus Durchsacken und stetigem Nickabfall. Nicken ist ein reines
  Ratenkommando, langsam nur durch Nickabfall und Durchsacken schwach. Mit Fahrwerk
  unten gibt es auf leichten Stufen in Bodennähe eine Abfanghilfe; Turbulenz hängt
  von Stufe, Höhe, Fahrwerk und Schäden ab.
- **Referenzstelle:** f19re `src/egflight.c`, `stepFlightModel` (Flugzustand des
  Spielerflugzeugs: Eck- und Abrissgeschwindigkeit, Nase-runter-Drücken unterhalb
  der Abrissgeschwindigkeit).
- **Unser Design:** `flight.rs`, Werte in `F29_SPEC`: eine stetige Abrissstärke 0…1
  in der Kräftephysik.
  - **Abrissfahrt** (`flight::stall_speed`): v² = Vs(Klappen)² + (n − 1)·Vs(ohne)²
    aus Masse, Luftdichte und CLmax. Klappen senken nur den 1-g-Anteil, sonst wäre
    mit ihnen kein Kurvenabriss möglich. n ist die vom Knüppel *angeforderte* g-Last
    (`demanded_load_factor`), nicht die erreichte, sonst beendete der
    Auftriebsverlust den Abriss sofort (Pendeln): voller Zug bis 2,8 g zusätzlich,
    höchstens was die Ruder beim aktuellen Staudruck an Nickrate erzeugen
    (n = 1 + V·q/g), mit waagerechten Flächen 40 % davon (Gewichtung über sin² der
    Querlage). So reißen harte Kurven schnell ab, Hochziehen und Steigflug erst bei
    verbrauchter Fahrt, behutsames Abfangen bei Landefahrt gar nicht.
  - **Stärke:** Ziel ist das größere aus Fahrtdefizit (voll bei 18 % unter der
    Abrissfahrt) und Anstellwinkel über 0,32 rad (voll 0,2 rad darüber, gegen
    absurde Anstellwinkel knapp über der Abrissfahrt); sie folgt mit 0,3 s und
    klingt mit 1,5 s ab. Ein merklicher Abriss (ab 0,1) rastet bei mindestens 0,5
    ein, bis die Fahrt 1,2 × 1-g-Abrissfahrt erreicht: Er kostet viel Höhe, auch
    wenn man sofort loslässt, pendelt nicht, und kurze Abrisse in schnellen Kurven
    bleiben kurz.
  - **Wirkung** proportional zur Stärke: Auftrieb bis auf 18 %, Zusatzwiderstand,
    Ruder bis auf 10 %; die Nase kippt aus jeder Lage Richtung senkrecht nach unten
    (bis 1,9 rad/s, Rollen bleibt frei), die Bahn folgt der Nase. Im Sturz kommt die
    Fahrt zurück; mäßiger Zug fängt ab, wer zu früh voll zieht, bleibt im Abriss.
  - **Durchsacken** (`flight::sink_angle`): Unter 1,5 × 1-g-Abrissfahrt liegt die
    Bahn bei gehaltener Nase tiefer, quadratisch wachsend bis 0,25 rad bei
    Abrissfahrt, ohne dass der Flügel mehr Anstellwinkel spürt; ein Zusatzwiderstand
    Gewicht × sin(Winkel) verhindert, dass das Sinken in Fahrt umgesetzt wird. Am
    Boden keins, bis 15 m Radhöhe eingeblendet (Abheben und Abfangen unverändert),
    im Abriss tritt es zurück. Grund: Mit großer Schubreserve hielt das Flugzeug
    langsam zu gut die Höhe; eine Rückseite der Leistungskurve über den induzierten
    Widerstand wäre bei gehaltener Nicklage instabil.
  - **Klappen** mildern Einrastung, Abkippen, Bahndrehung und Durchsacken um 30 %
    (M-007). **Warnung** „STALL“ mit Alarmton (M-060) ab Stärke 0,02, Anstellwinkel
    0,27 rad oder Fahrt unter 110 % der Abrissfahrt; die Autopiloten
    (`autopilot.rs`) schützen sich selbst.
  - **Kennwerte** (`tests/support/envelope.rs`, Bänder in `tests/envelope.rs`,
    Übersicht `cargo run --release -p f29-core --example flight_report`; 3000 m,
    60 % Treibstoff): Abrissfahrt 125 kt, mit Klappen 108 kt (volle Tanks,
    Meereshöhe). Höhenverlust (loslassen, dann halber Zug): horizontal verzögert
    ≈ 4900 ft (Klappen ≈ 4200), aus 40°-Steigflug ≈ 5200 ft (≈ 4400), 60°-Kurve bei
    250 kt voll gezogen keiner. Voller Zug ab 185 kt reißt in der 70°-Kurve nach
    1,6 s ab (Klappen 1,9 s), waagerecht nach 8,6 s. 30 s bei 1,2 × Vs mit
    gehaltener Nase: −330 ft, mit Klappen ±0. Start voll gezogen: bei 15°
    losgelassen kein Abriss; gehalten Abriss in ≈ 300 ft, mit Klappen höchstens
    ≈ 640 ft.
- **Abweichungen:** Kräftephysik; Rollen allein zieht keine g-Last (Querlage ohne
  Ziehen ergibt eine sinkende Kurve); der Abriss rastet ein, weil unsere Abrissfahrt
  am Knüppel hängt und Loslassen ihn sonst sofort beendete – im Original kostet er
  viel Höhe; die Nase kippt Richtung senkrecht, auch der Anstellwinkel löst aus;
  Klappen mildern; kein stärkerer Abriss auf hoher Stufe oder bei Schaden.
- **Status:** umgesetzt.

### M-009 Treibstoff

- **Original:** Der Vorrat sinkt mit der Zeit; der Verbrauch wächst mit dem Quadrat
  der Schubstellung, Vollgas ist also überproportional teuer. Ein Zusatztank lässt
  sich in die Haupttanks umpumpen. Volle Tanks machen das Flugzeug etwas langsamer,
  leere etwas schneller. Bei leerem Tank fällt der Schub weg, man segelt; die
  Nachbesprechung nennt leere Tanks als Absturzursache. Eine Kontrollanzeige warnt,
  sobald der Rest nicht mehr bis zur Heimatbasis reicht. Die Wegpunkttafel zeigt
  je noch offenem Wegpunkt die Strecke (zum aktiven ab der eigenen Position, dann
  von Punkt zu Punkt), die Flugzeit bei der jetzigen Fahrt und die Uhrzeit bei
  Ankunft; der Treibstoff steht dort als Balken mit einem Stück je Abschnitt
  (Verbrauch beim jetzigen Schub, nach Schaden mit Aufschlag) und einem Stück für
  den Rest, daneben der Vorrat als Zahl.
- **Referenzstelle:** f19re `src/egflight.c` (`stepFlightModel`: Verbrauch, Schub
  bei leerem Tank, Fahrteinfluss), `src/egtacmap.c` (Warnanzeige,
  `drawWaypointPanel`: Wegpunkttafel mit Verbrauchsvorschau), `src/egcombat.c` (Umpumpen des Zusatztanks),
  `src_end/enbrief.c` (Absturzursache).
- **Unser Design:** `flight::fuel_flow`, Warnungen in
  `crates/f29-core/src/sim/info.rs`. Treibstoff ist echte Masse (M-001) statt eines
  Fahrtfaktors. Verbrauch = Leerlauf + (Vollgas − Leerlauf) × Stellung²
  × (ρ/ρ0)^0,75 wie beim Schub, in der Höhe reicht der Tank also länger: 5200 kg,
  0,35 bzw. 4,3 kg/s – rund 20 min Vollgas auf Meereshöhe, in 3000 m mit 200 m/s
  etwa eine Stunde. Leer läuft das Triebwerk ohne Neustart aus („FLAMEOUT“). Anzeige
  in kg auf ENG und MSN (M-050); „FUEL LOW“ unter 15 %, „BINGO“, sobald der Rest bis
  zur Heimatbasis plus 450 kg Reserve nicht mehr reicht (echte Entfernung,
  mindestens 150 m/s und halber Schub). Zusatztank (1000 kg) und Bedarfsschätzung
  aus der Routenlänge: M-036.
  - **Leck:** Ein beschädigtes Tanksystem verliert zusätzlich Treibstoff
    (Schadensmodell, M-015); der angezeigte Verbrauch (FUEL FLOW), die
    Restflugzeit, BINGO und die Vorschau rechnen es mit.
  - **Verbrauchsvorschau:** Die MSN-Seite (M-050) nennt je Wegpunkt Strecke
    und Verbrauch des Abschnitts – zum aktiven Punkt ab der eigenen Position,
    dann von Wegpunkt zu Wegpunkt – beim jetzigen Verbrauch je Meter
    (Verbrauch durch wahre Fahrt), unter 50 m/s nach der Planung mit
    11 kg/km; darunter LEFT, der Rest nach der Route, unter der Reserve in
    Warnfarbe. Die Lagebesprechung (FLIGHT PLAN, M-036) zeigt dasselbe nach
    der Planung: 11 kg/km je Abschnitt und der Rest vom mitgeführten
    Treibstoff.
- **Abweichungen:** Auch der Leerlauf verbraucht; die Nachbesprechung nennt keinen
  leeren Tank. Die Vorschau steht als Zahl je Abschnitt und Rest statt als
  Balken, ohne Flug- und Ankunftszeit je Abschnitt (die Flugzeit zum aktiven
  Punkt steht darunter); erledigte Punkte bleiben gedämpft sichtbar; im Flug
  zählen nur die Haupttanks, ein Zusatztank erst nach dem Umpumpen.
- **Status:** umgesetzt.

### M-040 Flugplätze

- **Original:** Flugplätze sind keine echten Bahnen, sondern ein schmaler, langer
  Bereich um einen Mittelpunkt, alle in Nord-Süd-Richtung; auf der leichtesten
  Landestufe ist der Bereich größer. Flugzeugträger haben einen kürzeren Bereich auf
  Deckhöhe; wer über das Deckende hinausrollt, stürzt ab.
- **Referenzstelle:** f19re `src/egframe.c` (Suche des nächsten Flugplatzes,
  Landekorridor, Startposition), `src/egflight.c` (Trägerstart).
- **Unser Design:** `crates/f29-world/src/airfield.rs`. Bahnen aus der Gebietsdatei
  (M-031): die **Heimatbasis** (3 km × 45 m, voll markiert, PAPI) und **feindliche
  Fliegerhorste** (2,5 km × 40 m, Heimat der Jäger, M-022); gelandet werden kann auf
  jeder. Echte Bahnen in 22,5°-Schritten (Richtungen als exakte Konstanten, bitgenau
  deterministisch); das Gelände wird darunter eingeebnet und weich übergeblendet,
  damit gelten Kollision, Radarhöhe und Darstellung automatisch. Start stehend am
  Anfang der Heimatbahn (`--start air` für den Luftstart); die Route kommt vom
  Missionsgenerator (M-031). Im NAV-Modus sind Bahnen wählbar (M-017).
- **Abweichungen:** echte Bahnen in vielen Richtungen statt Nord-Süd-Korridor; das
  Flugverhalten lockert nur die Landegrenzen, nicht die Platzgröße (M-042); keine
  geheimen Pisten (kämen ggf. mit Absetzaufträgen).
- **Status:** teilweise (Flugzeugträger folgen mit Szenarien auf See).

### M-041 Fahrwerk und Rollen am Boden

- **Original:** Das Fahrwerk ist ein Schalter, der sich am Boden nicht einfahren
  lässt. Ausgefahren kostet es Fahrt, rüttelt bei hoher Fahrt, und eine Leuchte
  blinkt, wenn man zu schnell ist. Am Boden wird die Querlage aufgehoben, der
  Rollknüppel lenkt (auch im Stand), die Nase kann nicht unter den Horizont, und
  hochziehen geht erst ab einer Mindestfahrt. Dieselbe Taste wirkt in der Luft als
  Luftbremse und am Boden als Radbremse.
- **Referenzstelle:** f19re `src/egkeys.c` (Tasten), `src/egflight.c`
  (`stepFlightModel`: Bodenbehandlung, Bremse, Fahrwerkswiderstand, Rütteln).
- **Unser Design:** `flight::ground_step`, Werte in `F29_SPEC` (Abschnitt Fahrwerk
  und Boden); bewusst einfach, ohne Federbeine.
  - **Fahrwerk** (Taste G): 5 s Fahrzeit (hydraulisch, M-015), am Boden nicht
    einfahrbar, spürbarer Widerstand. Es rüttelt in der Luft immer leicht, über
    128,6 m/s (250 kt) stärker, mit „GEAR SPEED“ (M-006); HUD „GEAR“ blinkt beim
    Fahren und bei Überfahrt.
  - **Rollmodus:** Höhe auf den Boden geklemmt, keine Querlage, Nase 0…12°; Lenken
    über Quer- und Seitenruder mit einer Drehrate, die mit der Fahrt abnimmt – auf
    der Stelle wenden geht. Rollreibung und Radbremse (Taste B wie die Luftbremse,
    HUD „BRAKES“) nach Radlast. Wer schneller als 12 m/s (≈ 23 kt) die befestigte
    Fläche verlässt, verunglückt („OFF RUNWAY“).
  - **Start:** Rotieren erst ab 1,1 × Abrissfahrt (masse- und klappenabhängig), dann
    wählt der Knüppel die Nicklage (voll gezogen 12°); Abheben, sobald der Auftrieb
    das Gewicht trägt, danach normale Ratensteuerung (M-004). Startrollstrecke mit
    vollen Tanks und Standardbeladung ≈ 630 m mit Klappen, ≈ 900 m ohne.
- **Abweichungen:** Fahrwerk mit Fahrzeit; gelenkt wird auch mit dem Seitenruder.
- **Status:** umgesetzt.

### M-042 Aufsetzen und Landung

- **Original:** Beim Aufsetzen im Landekorridor ist die Landung ein Absturz, wenn
  die Sinkrate für die Landestufe zu hoch, das Fahrwerk eingefahren oder die
  Querlage zu groß ist. Ein Faktor je Stufe: „Leicht“ verträgt die doppelte Sinkrate
  und Querlage von „realistisch“, bei „kein Absturz“ zerstört Aufsetzen im Korridor
  nie, auch ohne Fahrwerk. Außerhalb eines Flugplatzes ist jede schnelle
  Bodenberührung ein Absturz. Wer auf einem Platz zum Stehen kommt, hat gelandet;
  die Nachbesprechung bewertet das. Mit ausgefahrenem Fahrwerk zeigt das HUD die
  Sinkrate mit einer Marke für die höchste zulässige. Auf der leichtesten Stufe
  entfällt das Wackeln in Bodennähe (M-006).
- **Referenzstelle:** f19re `src/egflight.c` (Aufsetzprüfung), `src/egframe.c`
  (Stillstand auf dem Platz), `src/egtacmap.c` (Sinkratenanzeige),
  `src_end/enbrief.c` (Bewertung in der Nachbesprechung).
- **Unser Design:** `evaluate_touchdown` in `crates/f29-core/src/sim/mod.rs`, Stufen
  als `Landings` in `rules.rs`.
  - **Erster Bodenkontakt:** Wasser oder Gelände abseits einer Bahn → Absturz; auf
    der Bahn Fahrwerk nicht verriegelt („GEAR UP LANDING“), Sinkrate über 7 m/s oder
    Nase unter −5° („GEAR COLLAPSED“), Querlage über 15° („WING STRIKE“) → Absturz.
    Sonst „GOOD LANDING“, über 4 m/s „HARD LANDING“, mit Sinkrate in m/s; so gelingt
    eine flache Landung mit 190 kt ohne Abfangen noch als harte
    (`tests/landing.rs`). Die Fahrt zählt wie im Original nicht. Stillstand auf
    einer Bahn nach dem Flug („STOPPED – <Bahn>“) beendet die Mission (M-032).
  - **Flugverhalten** (M-036): realistisch ×1; leicht ×1,5 auf alle Grenzen, ohne
    Fahrwerk weiter Bruch; kein Absturz: auf der Bahn nie Bruch, auch ohne Fahrwerk,
    und kein Wackeln in Bodennähe; abseits einer Bahn bleibt Bodenberührung ein
    Absturz.
  - **HUD-Landemodus** (NAV, Fahrwerk unten, in der Luft): Sinkratenskala neben dem
    Glas mit Marken für „hart“ und „Bruch“ der Stufe, gestrichelte −3°-Linie in der
    Leiter. „PULL UP“ warnt mit ausgefahrenem Fahrwerk nur bei gefährlicher Sinkrate
    (über der doppelten Bruchgrenze), „GEAR“ im Anflug mit eingefahrenem Fahrwerk.
  - **PAPI** der Heimatbasis: je Richtung vier Lichter links am Aufsetzpunkt, auf
    dem 3°-Gleitpfad zwei weiß, zwei rot. Testpilot `autopilot::Approach`.
- **Abweichungen:** „Leicht“ lockert auf das Anderthalbfache statt das Doppelte –
  bei unseren schon großzügigen Werten wäre das Doppelte ein Aufprall. Bruch auch,
  wenn das Bugrad zuerst aufsetzt.
- **Status:** umgesetzt.

### M-006 Turbulenz in Bodennähe

- **Original:** In geringer Höhe beginnt das Flugzeug zu wackeln, umso stärker, je
  tiefer und je schneller es fliegt: ein Zufallsanteil auf Roll- und Nickeingabe, um
  die Querachse halb so stark wie um die Längsachse. Dazu Schütteln bei
  ausgefahrenem Fahrwerk und hoher Fahrt sowie bei Schäden. Ausgefahrene
  Landeklappen halbieren das Wackeln; auf der leichtesten Flugstufe ist das
  Bodennähe-Wackeln abgeschaltet. Der Boden ist praktisch eben (M-005).
- **Referenzstelle:** f19re `src/egflight.c`, `stepFlightModel` (Zufallsanteil auf
  Roll- und Nickeingabe).
- **Unser Design:** `update_turbulence` in `crates/f29-core/src/sim/mod.rs` (Werte
  `TURBULENCE_*`, `GEAR_*`, `DAMAGE_SHAKE`). Störraten auf Rollen und Nicken (Nicken
  halb) ∝ (1 − Höhe über Grund / 1000 ft) × Fahrt, Klappen halbieren sie; über
  Grund, weil unser Gelände Berge hat. Zum Boden hin nicht ausgeblendet: Langsamer
  Tiefstflug unter Radaren (M-020) soll anspruchsvoll bleiben, wer langsam fliegt
  oder Klappen setzt, wackelt weniger. Dazu Rütteln mit ausgefahrenem Fahrwerk in
  der Luft (immer leicht, über dessen Höchstfahrt stärker) und je Treffer bis zum
  sechsten (M-015). Zufall aus eigenem deterministischen RNG-Strom (jeden Tick
  gezogen, Replays bleiben exakt), geglättet, damit es rüttelt statt zittert. Am
  Boden ruht die Turbulenz; „kein Absturz“ (M-042) schaltet den Bodennähe-Anteil ab.
- **Abweichungen:** Höhe über Grund; die Turbulenz beginnt bewusst höher als im
  Original (Wunsch des Spielers); leichtes Fahrwerksrütteln bei jeder Fahrt.
- **Status:** umgesetzt.

### M-007 Landeklappen

- **Original:** Ausgefahrene Landeklappen senken die Abrissgeschwindigkeit um etwa
  ein Viertel, kosten etwas Fahrt und halbieren die Turbulenz.
- **Referenzstelle:** f19re `src/egflight.c` (Klappen-Bit der Flugzeugschalter in
  der Fahrt- und Abrissberechnung) und `src/egkeys.c` (Tastenzuordnung).
- **Unser Design:** Taste F, 2 s Fahrzeit (hydraulisch), bei jeder Fahrt nutzbar
  (Manöverklappen, kein selbsttätiges Einfahren), HUD „FLAPS“. Vier Größen in
  `F29_SPEC`: `flap_cl` – der höhere CLmax senkt die 1-g-Abrissfahrt von selbst auf
  ≈ 86 %, unter Last zählt die ohne Klappen (M-004); `flap_cd` – Geradeausflug
  kostet etwas Fahrt; `flap_induced_factor` 0,6 auf den induzierten Widerstand –
  harte Kurven kosten weniger Fahrt (8 s am g-Begrenzer ab 450 kt: 386 statt
  374 kt); `flap_stall_relief` 0,3 – Abriss und Durchsacken milder, aber nicht
  harmlos (M-004). Für Langsamflug und Landung sind Klappen das Schlüsselelement.
  Sie halbieren die Turbulenz (M-006); auf die Sichtbarkeit für Radare wirken sie
  nicht (M-020).
- **Abweichungen:** Die Abrissfahrt sinkt nur um ≈ 14 % und nur im 1-g-Anteil;
  Manöverklappen und Abrissmilderung sind eigene Erweiterungen.
- **Status:** umgesetzt.

### M-008 Bodenkollisions-Abfanghilfe (geplant)

- **Original:** Auf leichten Schwierigkeitsstufen zieht das Original das Flugzeug in
  sehr geringer Höhe selbsttätig hoch, sofern der Pilot nicht drückt, die Querlage
  mäßig ist und die Fahrt über der Abrissgeschwindigkeit liegt.
- **Referenzstelle:** f19re `src/egflight.c`, `stepFlightModel`.
- **Unser Design:** noch nicht gebaut; vorgesehen als Teil des Flugverhaltens
  (M-036), gegen die Radarhöhe und mit Vorausschau auf das Gelände.
- **Abweichungen:** keine.
- **Status:** geplant.

### M-005 Höhenanzeige

- **Original:** Der Boden ist praktisch eben auf Höhe 0; erhöhten Boden gibt es nur
  an wenigen Objekten (z. B. Schiffsdeck). Die angezeigte Höhe ist dort faktisch die
  Höhe über Grund.
- **Referenzstelle:** f19re `src/egflight.c` und `src/egframe.c` (Bodenhöhe des
  Spielerflugzeugs, Aufsetzen).
- **Unser Design:** Weil unser Gelände Berge hat, zeigt das HUD die Höhe über dem
  Meer (m) im Kasten und darunter **immer** die Radarhöhe über Grund bzw. Wasser
  („R …“), rot unter 150 m.
- **Abweichungen:** zusätzliche Radarhöhe – über dem Meer allein reicht nicht für
  den Tiefflug.
- **Status:** umgesetzt.

### M-050 Instrumentenbrett mit MFDs

- **Original:** Das Instrumentenbrett nimmt gut ein Drittel des Bildschirms ein. Die
  Karte zeigt die Umgebung als dunkle Fläche mit Gitter und Symbolen, in Stufen
  zoombar. Die Schubanzeige unten links ist ein Hebel, dessen Schaft schräg von
  einem Drehpunkt in der Ecke ausgeht und dessen Griff mit dem Schub diagonal nach
  oben wandert. Eine Weitwinkelsicht blendet das Brett für den Luftkampf aus.
- **Referenzstelle:** für das Layout keine Code-Lektüre (Anordnung und Inhalt nach
  Wunsch des Spielers); Schubhebel nach f19re `src/egflight.c` (`drawAirspeedTape`).
- **Unser Design:** `crates/f29-hud/src/mfd.rs`, `pages.rs`, `throttle.rs`. Brett
  40 % der Fensterhöhe: zwei quadratische MFDs, dazwischen die SIG-Säule (M-020),
  ganz links der Schubhebel.
  - **Karte (links):** Flugrichtung oben, Flugzeug in der Mitte. F3 wechselt *GRID*
    (Höhenbild mit Gitter, grau nach der Wurzel der Höhe, damit auch Flachland
    Relief zeigt; Wasser dunkelblau, außerhalb schwarz) und *SAT* (Farbraster mit
    Hangschattierung); beide 4096² über das ganze Gebiet, im Hintergrund berechnet,
    je Bild kursbezogen und pixelig ausgeschnitten. Zoom mit Y/X in elf Stufen,
    0,625–640 km von der Mitte bis zum Rand. Darauf Flugplätze, Route mit aktivem
    Wegpunkt, Ziele als Quadrat (Fahrzeuge), Quadrat mit Kreuz (Bauwerke, Bunker,
    Brücken), Dreieck (Flugabwehr; Frühwarnradar mit Querstrich), Kreuz (abgestellte
    Flugzeuge) oder Raute (Schiffe) nach Erfassung (M-013), Radarstellungen (M-020),
    Jäger, Waffen, Täuschkörper, Nordpfeil, Kurs und Bereich.
  - **Seiten (rechts),** F4 blättert, eine Leiste unten zeigt die aktive: MSN
    (Ziele, Route mit Strecke und Verbrauch je Abschnitt und Rest, M-009,
    Richtung und Flugzeit zum aktiven Punkt, Heimweg, BINGO), TGT
    (gewähltes Ziel mit Bild, M-017), CAM (Fernsehsuchkopf), WPN (Schächte 2 × 2,
    gewählte Waffe, Hauptschalter; M-014), ENG (Drehzahl mit Leerlauf 62 %, Schub,
    Verbrauch, Treibstoffbalken mit BINGO-Marke), ADI (Reserve-Horizont mit Fahrt,
    Höhe, Kurs, Steigrate, g), SYS (Konfiguration, Gewicht, Abrissfahrt, Bordsysteme
    M-015, Warnfeld). Die HSI-Funktion übernimmt die Karte.
  - **Schubhebel** in einer Bogenkulisse von IDLE (flach) bis MIL (steil),
    dreifacher Schaft, T-Griff, Skala alle 10 %; eine Marke zeigt die tatsächliche,
    verzögert folgende Triebwerksstellung, eine rote sperrt bei Triebwerksschaden
    den verlorenen Bereich.
  - **Ansichten** auf Shift + F1–F5: Cockpit, Verfolger, Retro-Look, HUD und
    **Weitwinkelsicht** (C ist bei uns Düppel): Das Brett verschwindet, die
    3D-Ansicht füllt das Fenster im selben Maßstab (Sichtwinkel wächst nach unten,
    HUD-Symbole behalten ihre Größe); ohne MFDs keine Zielkamera. Ohne Brett auch
    beim Blick zur Seite oder nach hinten (M-035) und nach dem Ausstieg.
- **Abweichungen:** zwei MFDs mit Seiten; die Karte auch als Satellitenbild;
  Weitwinkelsicht auf Shift+F5.
- **Status:** umgesetzt.

### M-060 Klang

- **Original:** eigener AdLib-Treiber (FM-Synthese); das Triebwerk bekommt laufend
  Fahrt und Schub als Tonhöhe übergeben; Ereignisse wie Aufsetzen und Abrisswarnung
  sind kurze, nummerierte Klangmuster.
- **Referenzstelle:** f19re `src/egflight.c` (`audio_setEnginePitch(Fahrt, Schub)`,
  Aufrufe von `makeSound` bei Aufsetzen und Abriss), `src/asound.c` (Treiber, nur in
  Bruchstücken rekonstruiert).
- **Unser Design:** eigene prozedurale Synthese im Crate `f29-audio`.
  Engine-neutral: `Synth` macht aus einer Momentaufnahme der Simulation einen
  Mono-Datenstrom, den Bevy abspielt; WAV-Hörproben ohne Soundkarte mit
  `f29-headless sound` (Klanggruppen getrennt pegelbar). Klassisch-retro, aber
  zurückhaltend. Pause schaltet weich stumm; `--volume 0..100`, `--no-sound` (kein
  Audiogerät).
  - **Triebwerk:** zwei leicht verstimmte, weich gefilterte Rechtecktöne mit der
    Drehzahl als Tonhöhe, dunkles, im Takt des Brummens pulsierendes Rauschen
    (90–470 Hz je nach Schub) und ein leises Turbinenpfeifen; bei Flameout läuft es
    aus. Rollgeräusch am Boden; kein Windgeräusch, die Simulation hat keinen Wind.
  - **Systeme und Ereignisse:** Fahrwerk brummt und verriegelt mit dumpfem Schlag,
    Klappen surren und klicken, die Luftbremse zischt; Aufsetzen mit
    Reifenquietschen und Schlag (hart lauter), Absturz mit Krachen, das Triebwerk
    verstummt; Detonationen, Auslösen, Ausstieg.
  - **Warnungen:** Abriss als hoher, pulsierender Alarmton (1250 Hz, 6 Pulse/s),
    solange die Warnung anliegt. „PULL UP“ und „OVER G“ alle 1,6 s (Bodenwarnung
    zuerst) als eingebettete Samples `crates/f29-audio/sounds/pull_up.wav` und
    `over_g.wav`: CC0-Platzhalter aus Kenneys Voiceover Pack („Look out!“, „Hold!“),
    weil keine freie Aufnahme mit dem richtigen Wortlaut zu finden war (Herkunft:
    `sounds/README.md`). Radarwarnempfänger: M-020, Raketenwarnung: M-021.
  - **Musik** (`f29-app/src/music.rs`): `assets/music/theme.ogg` (Ogg Vorbis)
    läuft in Schleife auf allen Bildschirmen außerhalb des Cockpits (Titelbild,
    Menüs, Nachbesprechung) und pausiert im Flug, wie im Original. Ohne Datei
    bleibt es still; die Datei liefert der Spieler.
- **Abweichungen:** eigene Synthese statt FM-Treiber; die Tonhöhe folgt der Drehzahl
  statt Fahrt und Schub.
- **Status:** umgesetzt; die Musikdatei fehlt noch.

### M-002 Prozedurales Gelände

- **Original:** handgebaute Einsatzgebiete.
- **Referenzstelle:** keine; wir übernehmen weder Karten noch Geländedaten.
- **Unser Design:** `crates/f29-world/src/lib.rs`. Die Höhe ist eine
  deterministische Funktion von Gebiet und Ort, für Kollision, Sichtlinien und
  Darstellung identisch ausgewertet. Das Grobraster der Gebietsdatei (M-031) gibt je
  Zelle Meer, Ebene, Hügel oder Gebirge vor, weich gemischt und verzerrt abgefragt
  (natürliche Küsten); Gradientenrauschen (fBm) und Grat-Rauschen für Gebirgszüge
  formen die Feinheiten, dazu Strand, Wald, Fels und Schnee. Ohne Gebiet reines
  Rauschen nach `TerrainParams::default()`.
- **Abweichungen:** prozedurales, bergiges Gelände statt handgebauter, praktisch
  ebener Gebiete (M-005).
- **Status:** umgesetzt.

### M-003 HUD und Cockpit-Instrumente

- **Original:** genre-typisches HUD (Pitch-Ladder, Flight Path Marker, Kurs-, Fahrt-
  und Höhenanzeige) über der 3D-Sicht, darunter ein Instrumentenbrett.
- **Referenzstelle:** keine (allgemeine HUD-Konventionen realer Kampfflugzeuge).
- **Unser Design:** `crates/f29-hud`, ohne Engine: Aus `FlightData` und Layout
  entstehen Linien in auflösungsunabhängigen Bildschirmeinheiten mit eigenem
  Vektor-Strichfont, ohne GPU testbar (`f29-headless hud-svg`); auch den Inhalt der
  Menüs und der Nachbesprechung liefert es (`Page`, gezeichnet mit bevy_ui). Im
  Glas: Nicklage-Leiter alle 5°, Boresight, Flight Path Marker, Kursband mit
  Peilmarke, Querneigungsskala; links g, Fahrt (km/h), Mach, Anstellwinkel und
  darunter Fahrwerk, Klappen und Bremse in festen Zeilen, nur solange ausgefahren;
  rechts Steigrate (m/s), Höhe (m), Radarhöhe (M-005) und Navigationspunkt;
  blinkende Warnungen (MISSILE, PULL UP, STALL, OVER G, GEAR, GEAR SPEED, TOO LOW,
  FLAMEOUT, BINGO, FUEL LOW). Modi M-017, Landemodus M-042.
- **Abweichungen:** keine; Anordnung und Schrift sind eigen.
- **Status:** umgesetzt.

### M-010 Waffenwirkung nach Waffe und Zielklasse

- **Original:** Jedes Bodenobjekt gehört über sein Modell zu einer von gut einem
  Dutzend Zielklassen, einige sind Wasserziele (Schiffe). Ein Kennzeichen „zivil
  oder befreundet“ zeigt das Cockpit beim Aufschalten als Hinweis (auf leichten
  Stufen immer, sonst nur für bestimmte Ziele). Eine Tabelle [Waffe][Zielklasse],
  auch mit einer Zeile für die Bordkanone, gibt kleine Wirkungswerte; null heißt
  wirkungslos, dann lässt sich das Ziel mit dieser Waffe nicht aufschalten. Der
  Wert bestimmt die Größe des Zielrahmens, bei Flugkörpern die Zerstörungschance
  (sonst „wirkungslos“), bei Bomben den Wirkradius. Die Klassennamen stehen in
  Datendateien, die f19re nicht enthält.
- **Referenzstelle:** f19re `src_en/egtacmap.c` (Nachschlagen der Waffenwerte je
  Zielklasse), `src_en/egtarget.c` (Klassenbezug der Ziele, Wasserziele, Hinweis
  auf zivile/befreundete Ziele), `src_en/egcombat.c` (Wirkung beim Einschlag).
- **Unser Design:** elf eigene Zielklassen (`f29_world::class`, Ziele mit Zustand
  und Rolle in `f29_core::targets`): weich, gepanzert, Gebäude, gehärtet,
  SAM-Stellung, Frühwarnradar, Kurzstrecken-Flugabwehr, Brücke, abgestelltes
  Flugzeug, kleines und großes Schiff; Radar und Flugabwehr sind wie im Original
  **ein** Objekt (M-020). Jede Klasse hat Namen und eine Form aus wenigen Quadern
  (3D-Modell, Zielbild, Trefferform). Die Ziele sind die Objekte des
  Einsatzgebiets (M-031); zivile (Fabrik, Lagerhalle, Fischerboot, Frachter,
  Tanker) nicht angreifen, Missionsziele sind nie zivil. Karte: Symbol je Klasse,
  P/S für Primär- und Sekundärziel, zivile gedämpft, zerstörte verschwinden.
  - **Wirkungsmatrix** `WeaponKind::effectiveness` (`weapons/spec.rs`): bei Bomben
    **Schaden pro Treffer** (1 = ein Treffer, 0,5 = zwei), der sich am Ziel
    sammelt (beschädigt → zerstört); bei Raketen wie im Original die **Chance**,
    mit dem Treffer zu zerstören, sonst „INEFFECTIVE“ (M-014). Weiche Ziele,
    Flugzeuge am Boden, Radarstellungen und kleine Schiffe fallen mit einem
    Treffer; Gebäude, Brücken: zwei Bomben, Maverick 34 bzw. 25 %; Panzer: zwei
    Mk 82/Snakeye, eine GBU-12 oder Maverick; große Schiffe: drei Mk 82/Snakeye,
    zwei GBU-12, Maverick 25 %, Harpoon 50 %. HARM nur gegen Radarstellungen,
    Harpoon nur gegen Schiffe; gehärtete Ziele M-012, Kanone M-016.
  - **Kritischer Treffer:** Ein voller Bombentreffer (M-011) zerstört zu 1 %
    sicher – mit Glück reicht eine Bombe für die Brücke. Schaden unter 0,05 zählt
    nicht; jedes Ziel im Wirkradius zählt für sich, keine Sekundärexplosionen.
- **Abweichungen:** eigene Klassen; Bomben mit Schaden pro Treffer (Wunsch des
  Spielers) und kritischen Treffern; wirkungslose Ziele bleiben wählbar
  („NO EFF“), der Rahmen hat feste Größe (M-017); die Kanone hat keine
  Tabellenzeile.
- **Status:** umgesetzt.

### M-011 Einschlagdistanz und Schwierigkeitsfaktor

- **Original:** Bomben zerstören ein Ziel, wenn der Einschlag näher liegt als ein
  Abstand aus Wirkungswert (M-010) und Schwierigkeitsgrad – auf schweren Stufen
  genauer. Flugkörper, die ihr Ziel erreichen, zerstören es mit einer
  Wahrscheinlichkeit, die mit dem Wirkungswert steigt. Wer zu tief auslöst, wird
  von den eigenen Splittern getroffen.
- **Referenzstelle:** f19re `src_en/egcombat.c` (Einschlag von Bomben und
  Flugkörpern), `src_en/egtarget.c` (Bordkanone gegen Bodenziele).
- **Unser Design:** `Munitions::blast` (`weapons/munitions.rs`). Bomben rein
  physikalisch: Abstand vom Zielrand (waagerecht bzw. über der Zielhöhe); bis
  40 % des Wirkradius voller Schaden nach M-010 – das ist ein Treffer –, dann
  linear weniger bis null; gewürfelt wird nur der kritische Treffer. Raketen
  würfeln für jedes Ziel in ihrem Wirkradius (M-014). Splitter: „FRAG WARNING“.
- **Abweichungen:** Schaden nach Abstand statt fester Grenze (Wunsch des Spielers).
- **Status:** teilweise (es fehlen der Schwierigkeitsgrad als Faktor auf den
  Wirkradius und der Splitterschaden am eigenen Flugzeug).

### M-012 Gehärtete Ziele: Spezialmunition und Toss-Bombing

- **Original:** Gehärtete Ziele lassen sich nur mit spezieller Munition zerstören,
  im Toss-Bombing (Hochwurf aus dem Steigflug) und aus vorgegebener
  Anflugrichtung (Norden). Es gibt drei Bombenarten mit eigener Flugbahn (M-014)
  und Lastabwurf als Missionsart; eine Prüfung des Eintreffwinkels war im
  gelesenen Code nicht zu finden.
- **Referenzstelle:** f19re `src_en/egcombat.c` (Flugbahn der abgeworfenen Waffen).
- **Unser Design:** Echte Ballistik (M-014), Toss ergibt sich von selbst. Gegen
  gehärtete Ziele wirken nur Mk 82 und GBU-12 ab 60° Eintreffwinkel unter der
  Waagerechten (`HARDENED_MIN_IMPACT_RAD`), auch dann erst mit zwei Treffern – so
  steil kommt eine Bombe praktisch nur im Toss oder Sturzflug. Die Snakeye ist
  dafür zu leicht gebaut, Raketen wirken nicht, die Kanone schon (M-016).
  Geplant: die erlaubte Anflugrichtung als **Eigenschaft des Ziels** in den
  Gebietsdaten (z. B. Bunkereingang) statt global „Norden“.
- **Abweichungen:** Die Prüfung des Eintreffwinkels ist eigener Entwurf.
- **Status:** teilweise (die Anflugrichtung je Ziel fehlt).

### M-013 Zielerfassung und Lenkung

- **Original:** Ein Zielsystem wählt Boden- und Luftziele und zeigt sie mit Name,
  Entfernung und Bild; gelenkte Waffen fliegen das gewählte an. Der Spieler wählt
  eine Blickrichtung (voraus, links, rechts, hinten): das nächste intakte Ziel in
  einem breiten Kegel bis zu einer Höchstreichweite, erneutes Drücken das
  nächstweitere; nachts (es gibt Nachtflüge) deutlich kürzer. Es gibt **keine
  Sichtlinienprüfung**, auch nicht im Suchkopf der Flugkörper; am Gelände
  zerschellen können sie aber. Ein schaltbares Bordradar gibt es nicht
  (Kamera-/Sensorsystem, emittierend ist nur der Störsender). Feuerbereit ist
  eine gelenkte Waffe, wenn sie wirkt (M-010), vorhanden ist und das Ziel nahe der
  HUD-Mitte und in ihrer Aufschaltreichweite liegt; die gelenkte Bombe verlangt
  stattdessen ein Ziel steil genug unter dem Flugzeug. Danach lenkt der **eigene
  Suchkopf** ohne Verbindung zum Flugzeug: Er hält das Ziel in Kegel und
  Reichweite, sucht ohne Aufschaltung selbst das beste seiner Art (Radarjäger nur
  aktive Radare, Seezielflugkörper nur Schiffe) und fliegt ohne Ziel geradeaus.
  Je eine Taste wechselt Waffenart (Luft-Boden, Luft-Luft, Kanone) und Station;
  bei geschlossenem Schacht Meldung „Schacht zu“.
- **Referenzstelle:** f19re `src_en/egtacmap.c` (Zielauswahl pro Bild),
  `src_en/egtarget.c` (Aufschalten, Zielrahmen, Zielbild), `src_en/egcombat.c`
  (Suchkopf der Flugkörper, Abschuss), `src/egkeys.c` (Tasten).
- **Unser Design:** `f29_core::sensors`.
  - **Vier Stufen** eines Ziels (Boden, Luft, Landebahn; Begriffe so in Code und
    Doku): **gemeldet** – nur vom AWACS, sichtbar, nicht wählbar; **entdeckt** –
    vom eigenen Radar oder optischen System, wählbar; **gewählt** – je Zielart
    eines; **erfasst** – im Erfassungskreis und von der gewählten Waffe
    übernehmbar (M-017); nur so geht es an eine Rakete.
  - **Sensoren,** rundum und für alle Zielarten gleich (das Auge ist nur die
    Darstellung ferner Jäger als Punkt): **Radar** ohne Sichtlinie (durch Berge),
    50 km Tag und Nacht, Taste V, beim Start aus, macht das Flugzeug sichtbarer
    (M-020). **Optisch** (als Infrarot erzählt) mit freier Sichtlinie, 50 km bei
    Tag, 20 km nachts, verrät nichts; nur es liefert ein Zielbild, und optische
    Suchköpfe (Maverick, AIM-9, feindlich SA-13, AA-8) brauchen wie es freie
    Sicht. Schäden kürzen beide Reichweiten (M-015).
  - **Wählen** automatisch wie im Original: das nächste entdeckte Ziel in ±45° um
    die Blickrichtung (I voraus, L rechts, K hinten, J links); der Kegel gilt nur
    fürs Wählen, die Zielart gibt der HUD-Modus vor (M-017); zivile werden mit
    gewählt, aber gekennzeichnet. Dieselbe Taste wieder: das nächstweitere, nach
    dem weitesten das nächste. Die Wahl bleibt, solange ein System das Ziel
    entdeckt, und die Blickrichtung dreht mit (Überflug von voraus nach hinten);
    ohne Wahl nimmt das System das nächste; ein Wrack bleibt gewählt (M-014).
  - **Takt** 4-mal pro Sekunde (Radarschalter und Tasten sofort); Sichtlinie in
    60-m-Schritten, höchstens 1000, die letzten 60 m vor dem Ziel frei; eigene
    Erfassung reihum, vier Bodenziele je Takt.
  - **AWACS-Datalink** (pseudosimuliert als Startbedingung, Standard an,
    `--no-awacs`, im Replay): Was die eigenen Sensoren nicht erfassen, bleibt mit
    Lage und Zieltyp auf der Karte, ohne Kennung „zivil“ und gedämpft; ohne AWACS
    fehlt es (Primär- und Sekundärziel bleiben markiert). Gewählt wird nur mit
    eigenen Sensoren. **Nacht** (`--night`, im Replay): optisch 20 km, das Radar
    verliert nichts, die Außensicht ist dunkel und bläulich.
  - **Anzeige:** HUD-Zielrahmen (M-014, M-017), zivile durchgekreuzt, außerhalb
    des Glases ein Randpfeil; Zeile mit Radar, Blickrichtung, Entfernung;
    TGT-Seite mit Zielbild, Name, Klasse (zivil „DO NOT ATTACK“), Rolle,
    Entfernung, Peilung, Sensor.
  - **Zielkamera** (`crates/f29-app/src/tgt_camera.rs`): eine 3D-Kamera unter dem
    Rumpf zoomt auf die Zielmitte, bis das Ziel knapp die halbe Bildhöhe füllt;
    grobe Textur (144 Zeilen), eigener Effekt: körniges, leicht grünes
    Schwarzweißbild mit Zeilen, gelegentlichem Zeilenversatz, dunklen Rändern,
    verrauschter mit der Entfernung. Nur bei offener TGT-Seite, optisch gesehenem
    Ziel (auch Jäger, Landebahn) und intakten Anzeigen; ohne Grafik (SVG) ein
    Drahtgitter.
  - Lenkwaffen sind Fire and Forget wie im Original, außer der Laserbombe (M-014).
- **Abweichungen:** schaltbares Radar und optisches System mit Sichtlinie statt
  eines Sensorsystems ohne Geländeprüfung; optische Suchköpfe prüfen die
  Sichtlinie; entdeckt wird rundum, nur gewählt im Kegel; Stufen und AWACS sind
  eigene Zutaten; Feuerbereitschaft über den Erfassungskreis (M-017).
- **Status:** umgesetzt.

### M-014 Waffenschacht, Abwurf und Zielhilfe

- **Original:** Vier interne Stationen tragen je eine Waffenart mit Stückzahl,
  dazu kommt Kanonenmunition; eine Station kann Zusatztreibstoff tragen, der beim
  „Auslösen“ in den Tank wandert. Ausgelöst wird bei offenem Schacht (sonst
  „Schacht zu“), mit Munition und nicht zu großer Querlage; das beendet die
  Zeitraffung, nur wenige eigene Waffen fliegen gleichzeitig. Die Waffe startet
  knapp unter dem Flugzeug mit dessen Kurs, Lage und Fahrt, die Nase etwas nach
  unten; Flugkörper beschleunigen auf Höchstfahrt, eine Lebensdauer aus
  Reichweite und Fahrt begrenzt den Flug, Ungelenktes fliegt bis zum Boden. Mit
  Aufschaltung übernimmt die Waffe das Ziel, wenn ihre Art passt (Radarjäger nur
  Stellungen mit aktivem Radar), sonst sucht ihr Suchkopf selbst (M-013). Kurs
  und Nicklage drehen mit begrenzter Drehrate je Waffe zum Ziel, die Nicklage
  etwas darüber (leichter Bogen); ohne Ziel sinkt die Nase langsam. Die
  Freifallbombe neigt sich stetig bis zu einem beim Abwurf festgelegten
  Grenzwinkel (Wurf aus dem Steigflug möglich), die gelenkte Bombe hält
  mindestens einen flachen Gleitwinkel und lenkt, eine dritte Art fällt langsam
  senkrecht (Lastabwurf). Einschlag wie in M-011 („verfehlt“ bzw.
  „wirkungslos“); zu nah am eigenen Flugzeug (waagerecht plus Höhe) trifft es der
  Splitterschaden; ein Einschlag macht den Gegner auf den Ort aufmerksam; Gelände
  beendet jeden Flug. Zielhilfe der Freifallbombe bei geringer Querlage:
  Aufschlagpunkt mit Linie vom Flugbahnsymbol, mit Ziel eine Raute darüber und
  oben eine Klammer, die mit dem Abstand Aufschlag–Ziel schrumpft (geschlossen =
  auslösen) und blinkt, wenn das Flugzeug zu tief ist; für die gelenkte Bombe
  zeigt sie die Entfernung.
- **Referenzstelle:** f19re `src_en/egcombat.c` (Abschuss, Flugbahn und Lenkung,
  Einschlag), `src_en/egframe.c` (Grundbeladung, Stationen), `src_en/egtarget.c`
  (Zielhilfe im HUD).
- **Unser Design:** `f29_core::weapons`: `Stores` (Schächte, Wahl, Klappen,
  Auslösen), `Munitions` (alles in der Luft), Kennwerte in `spec.rs`, Flug und
  Lenkung in `guidance.rs`.
  - **Ausrüstung** mit realen Bezeichnungen: Mk 82 (Freifallbombe), Mk 82 Snakeye
    (Bremsschirm), GBU-12 Paveway II (Laserbombe), AGM-65 Maverick
    (Fernsehsuchkopf), AGM-88 HARM (Radarjäger), AGM-84 Harpoon (Seezielrakete);
    dazu AIM-9, AIM-120 (M-022), Kamera (M-032), Zusatztank (M-036). Beladung
    nach Vorschlag des Auftrags bzw. Bewaffnungsbildschirm (M-032, M-036) oder
    `--loadout fall,chute,lgb,agm` (auch `arm`, `ship`, `aim9`, `aim120`, `cam`,
    `tank`, `-` leer); Tests und Demo fliegen sonst Mk 82, Snakeye, GBU-12,
    Maverick (knapp 2 t), Landeanflüge leer. Die Waffen zählen zur Masse.
  - **Auslösen:** Klappen mit eigener Taste (O, 1,2 s; offen sichtbarer, M-020),
    Waffe mit Tab (M-017). Nur in der Luft, bei offenem Schacht (Zusatztank
    ohne), intaktem Waffenschacht (M-015), bis 60° Querlage und mit höchstens acht
    eigenen Waffen in der Luft; sonst „BAY CLOSED“, „BAY FAULT“, „BAY EMPTY“,
    „BANK LIMIT“ bzw. „WEAPONS BUSY“.
  - **Bomben:** Massenpunkt mit Schwerkraft und Luftwiderstand aus der
    Normatmosphäre (120 Hz), Start 1,5 m unter dem Rumpf mit 3 m/s Ausstoß; Toss
    ergibt sich von selbst. Die Snakeye bremst ab 0,3 s am Schirm und bleibt so im
    Tiefflug weit genug zurück. Einschlag nach M-011 und M-012; zerstörte Ziele
    werden flache, dunkle Wracks.
  - **Raketen** (dieselbe Rechnung für feindliche, M-021, M-022): 0,4 s freier
    Fall aus dem Schacht, dann Marschfahrt bis zum Ende (kein Brennschluss, wie im
    Original) und begrenzte Drehrate auf die Zielmitte – reine Verfolgung, gegen
    bewegte Ziele mit Vorhalt. Der Suchkopf prüft gestaffelt zehnmal pro Sekunde
    Kegel, Reichweite und (optisch) Sichtlinie, sonst fliegt die Rakete geradeaus;
    über 135° Ablage gibt sie auf (wie im Original). Sie zündet, wenn ihre Bahn der
    Zielmitte näher kommt als der Wirkradius; Gelände oder Ende der Flugzeit
    beenden den Flug. Die Wirkung wird gewürfelt (M-010).
  - **Übergabe** nur, wenn das gewählte Ziel im Erfassungskreis liegt (Boden ±6°,
    Luft ±10°, M-017), in Reichweite ist und der Suchkopf es sieht (Klasse passt,
    optisch nur ein optisch entdecktes). Sonst fliegt die Rakete in Abschusslage
    geradeaus und schaltet das nächste passende Ziel ihres Kegels auf – **auch
    zivile**; HARM nur Radarstellungen, Harpoon nur Schiffe.
  - **Laserbombe GBU-12:** übernimmt ein gewähltes, optisch entdecktes Ziel ohne
    Erfassungskreis (sonst ginge nur der Sturzflug), fällt physikalisch und dreht
    die Bahn mit 6°/s zum Laserfleck. Das Flugzeug muss das Ziel bis zum Einschlag
    gewählt und optisch entdeckt halten (Wunsch des Spielers), sonst „LASER LOST“
    und sie fällt ungelenkt. Ob die Energie reicht, zeigt nur die Vorausberechnung.
  - **Vorausberechnung** (`launch_zone`, 20-mal pro Sekunde und sofort nach neuer
    Wahl): Der Kern fliegt die gewählte Lenkwaffe vom momentanen Abschusspunkt in
    groben Schritten voraus, mit derselben Übergabe und demselben Flug, Sicht und
    Laser als gehalten angenommen. Zonen: wirkt nicht, übernähme nicht,
    übernähme und verfehlte („LOCKED“), träfe („SHOOT“).
  - **Zielrahmen** (keine Mindestreichweite): doppelt, mit Name und Sensor
    (OPT/RDR); übernähme die Waffe das Ziel: Ecken; träfe sie: blau; wirkt sie
    nicht oder sieht ihr Suchkopf die Klasse nicht: gestrichelt, „NO EFF“;
    beschädigt „DMG“. Feuersignal unten mittig: „LOCKED“, darüber umrahmt
    „SHOOT“. Die Wahl bleibt auf dem Wrack („DESTROYED“, Zielbild zeigt es), bis
    der Spieler neu wählt; Wracks sind nicht wählbar und fehlen auf der Karte.
  - **Zielhilfe für Bomben** (Mk 82, Snakeye, bis 60° Querlage): Aufschlagpunkt
    (Kreis mit Punkt) mit Fall-Linie vom Flugbahnsymbol – liegt der Kreis auf dem
    Ziel, auslösen. Läge der Einschlag im Splitterbereich des weiterfliegenden
    Flugzeugs, wird alles zur Warnfarbe und „TOO LOW“ blinkt. Dieselbe
    Bahnrechnung wie die Bombe, 20-mal pro Sekunde in groben Schritten.
  - **Waffenkamera:** Die Maverick sendet ihr Bild auf die MFD-Seite „WPN CAM“,
    die nach dem Abschuss von selbst erscheint: Kamera an der Waffe (Effekt wie
    das Zielbild) mit festem Sichtfeld von 3° wie ein echter Suchkopf, das Ziel
    wächst im Anflug stetig; Blick zur Zielmitte („TRACK“) bzw. in Flugrichtung
    („SEARCH“), nach dem Einschlag „NO SIGNAL“. Die GBU-12 hat wie ihr Vorbild
    nur einen Lasersensor; ihren Einschlag zeigt das Zielbild (TGT).
  - **Anzeigen:** HUD-Warnfarbe blau, MFDs rot. HUD unten links Waffe mit Anzahl
    („MK-82 x2“) und Schacht („BAY 1 SAFE/DOORS/OPEN“, nicht ganz zu in
    Warnfarbe); WPN-Seite mit Bestand, Klappen (SAFE/DOORS/ARMED), Lenkart und
    Bereitschaft; Karte: Raketen als Strich, Bomben als Punkt, feindliche rot
    (Radar) bzw. orange (optisch). Meldungen unter dem Glas, etwa „MK-82 AWAY“,
    „CRITICAL HIT … DESTROYED“, „INEFFECTIVE ON …“, „FRAG WARNING“.
  - **Werte** (`spec.rs`; Größenordnungen aus öffentlichen Daten realer
    250-kg-Bomben und leichter Luft-Boden-Flugkörper, Wirkung nach Spielbalance):

    | Waffe | Masse, Cw·A | Fahrt, Beschl. | Dreh | Suchkopf | Wirk/Splitter | St. |
    |---|---|---|---|---|---|---|
    | Mk 82 | 241 kg, 0,020 m² | – | – | – | 30 / 180 m | 2 |
    | Snakeye | wie Mk 82; Schirm 0,9 m² ab 0,3 s | – | – | – | 30 / 180 m | 2 |
    | GBU-12 | 227 kg, 0,022 m² | – | 6°/s | Laser | 30 / 180 m | 2 |
    | AGM-65 | 210 kg | 700 m/s, 70 m/s² | 25°/s | 16 km, ±25° | 8 / 90 m | 2 |
    | AGM-88 | 360 kg | 650 m/s, 200 m/s² | 20°/s | 30 km, ±35° | 10 / 100 m | 2 |
    | AGM-84 | 520 kg | 270 m/s, 60 m/s² | 10°/s | 30 km, ±40° | 15 / 120 m | 1 |

    Flugzeit Maverick und HARM 70 s, Harpoon 160 s. Die Maverick beschleunigt
    gemächlich und bleibt so länger sichtbar (Wunsch des Spielers).
- **Abweichungen:** echte Ballistik statt Neigungsregeln, kein Lastabwurf; Bomben
  wirken nach Abstand auf alle Ziele im Radius; eigene Raketen ohne Bogen, gegen
  bewegte Ziele mit Vorhalt (damit Hineindrehen nicht rettet), ohne Ziel
  geradeaus statt mit sinkender Nase; Laserbombe mit Beleuchtung statt Gleitbombe;
  keine Auslöseklammer – sie bewährte sich im Spiel nicht, Aufschlagpunkt und
  Fall-Linie genügen; Vorausberechnung, Waffenkamera und blaue HUD-Warnfarbe sind
  eigene Zutaten; Einschläge alarmieren den Gegner nicht, Auslösen beendet den
  Zeitraffer nicht (M-034).
- **Status:** teilweise (es fehlt der Splitterschaden am eigenen Flugzeug, M-015).

### M-015 Bordsysteme und Schadensmodell

- **Original:** Treffer und Splitter beschädigen Bordsysteme. Ein Raketentreffer
  setzt so viele zufällige Schadensmarken aus einer festen Liste von acht
  Systemen, wie die Konfliktstufe plus eins vorgibt (auf schweren Stufen mehr),
  und zählt einen Trefferzähler hoch. Mit ihm wachsend sinkt der Höchstschub,
  zittert die Steuerung (in der Luft) und leckt Treibstoff; Raketenwarnung, Karte
  und Störsender fallen aus; viele Treffer verstärken das Durchsacken beim
  Strömungsabriss. Einen Lebensbalken gibt es nicht – verloren geht das Flugzeug
  über die Folgen (Absturz, kein Treibstoff). Eine Anzeige zeigt die Schäden.
- **Referenzstelle:** f19re `src_en/egcombat.c` (Splitterschaden nach eigenem
  Bombeneinschlag und nach Flugkörpertreffern, `bombTarget`), `egflight.c`.
- **Unser Design:** `f29_core::systems`: Triebwerk, Treibstoff, Hydraulik,
  Flugsteuerung, Radar, Optik, Waffenschacht, Anzeigen – je intakt, eingeschränkt
  (Restleistung 0..1) oder ausgefallen.
  - **Eine Schadensregel** für Raketen und Kanonen mit ihrer **Trefferstärke**
    (Rakete 1, Kanone je Muster ein Bruchteil, M-016): Ein Treffer verschlechtert
    im Mittel 2 × Stärke zufällige, verschiedene Systeme (Rakete genau zwei,
    Stärke 0,15 zu 30 % eines), intakt → halbe Leistung → ausgefallen. Das
    Triebwerk wird nur „beschädigt“ und verliert dann 8 % Höchstschub je Summe
    aller Trefferstärken (frühere zählen mit). Jeder Treffer verstärkt das
    Rütteln (bis sechs). Dieselbe Summe zählt bei den Jägern (M-022).
  - **Wirkung** an genau einer Stelle: Triebwerksstellung höchstens Restleistung;
    Leck bis 3 kg/s (volle Tanks in knapp 30 min leer); Klappen, Fahrwerk und
    Schachtklappen langsamer, ausgefallene Hydraulik blockiert; Drehraten bis
    auf die Notsteuerung (30 %); Radar- und Optikreichweite × Leistung,
    ausgefallenes Radar nicht einschaltbar; ausgefallener Schacht klemmt, kein
    Auslösen („BAY FAULT“); Anzeigen flackern unregelmäßig, ausgefallen dunkel
    (der mechanische Schubhebel bleibt). SYS-Seite: intakt gedämpft,
    eingeschränkt gelb mit Restleistung, ausgefallen rot umrahmt. Tests setzen
    Zustände über `Sim::set_condition`; `--no-damage` meldet nur (M-020).
- **Abweichungen:** eigene Systemliste; feste Stufen statt mit dem Trefferzähler
  wachsender Wirkung; fest zwei Systeme je Rakete; mehr Schubverlust je Treffer
  als im Original (Wunsch des Spielers); kein stärkeres Durchsacken.
- **Status:** teilweise (es fehlen der Splitterschaden eigener Bomben und die Zahl
  der Systeme je Treffer nach Schwierigkeit).

### M-016 Bordkanone

- **Original:** Eine fest eingebaute Kanone für Bodenziele im Tiefflug
  (Strafing), unabhängig von Schacht und Klappen, als eigene Waffenart (M-013)
  mit einer Zeile in der Wirkungstabelle (M-010). Die Jäger haben wie für ihre
  Raketen unbegrenzte Munition.
- **Referenzstelle:** keine (eigener Entwurf); zur Kanone des Originals M-010.
- **Unser Design:** `f29_core::gun`. Feuert, solange gehalten: Rücktaste bzw.
  Joystick-Taste 1 unabhängig von der Wahl, oder die Auslösetaste bei gewählter
  Kanone (M-017; Bits `GUN`, `TRIGGER` im Waffenwort); erster Schuss sofort,
  nicht am Boden.
  - **Patronen:** Massenpunkt (Mündungs- plus Flugzeuggeschwindigkeit,
    Schwerkraft, ohne Luftwiderstand), Streuung aus eigenem Zufallsstrom, nach 3 s
    verbraucht. Trifft die Bahn eines Ticks die Form eines intakten Ziels
    (senkrechter Zylinder aus Klassenradius und -höhe): 0,16 je Trefferstärke, bei
    **jedem** Ziel gleich, ohne Wirkungsmatrix und kritische Treffer – mit der
    M61A1 zerstören 50 Treffer jedes Ziel, auch gehärtete („GUN … DESTROYED“,
    leer „GUN EMPTY“). Flugzeuge: Strecke eines Ticks näher als 4 m an der Mitte
    (in beide Richtungen; Jäger M-022, eigenes Flugzeug M-015).
  - **Kanonen je Muster** (`GunSpec`, eigene Werte), alle 1000 m/s und 3 s
    Flugzeit (≈ 3 km); die F-29 hat 600 Schuss (12 s Dauerfeuer), die Jäger wie
    im Original unbegrenzt, begrenzt durch Feuerstöße (Pilotenkönnen, M-022):

    | | Muster | Schuss/s | Streuung | Stärke |
    |---|---|---|---|---|
    | F-29 | M61A1 | 50 | 5 mrad | 0,125 |
    | MiG-21 | GSh-23L | 50 | 8 mrad | 0,15 |
    | MiG-29 | GSh-30-1 | 25 | 6 mrad | 0,3 |

  - **HUD:** in AG das **Kanonenkreuz** (offenes Kreuz) am Geländeeinschlag einer
    jetzt abgefeuerten Patrone, nur innerhalb ihrer Flugzeit; in AA die
    **Vorhaltemarke** (Kreis mit Punkt) gegen den gewählten Jäger bis 3 km, aus
    Flugzeit, Fall und seiner Bewegung; bis 2 km im Erfassungskreis gilt er als
    erfasst. Unten links „GUN“ mit Munition. 3D: Leuchtspur, Wölkchen bei
    Einschlägen (am Ziel rot).
- **Abweichungen:** ohne Wirkungstabelle und kritische Treffer (Wunsch des
  Spielers).
- **Status:** umgesetzt.

### M-017 HUD-Modi und Waffenwahl

- **Original:** Das HUD zeigt je nach Aufgabe nur die passende Symbolik und vom
  Zielsystem nur das gewählte Ziel; Waffenart und Station haben je eine Taste.
- **Referenzstelle:** keine (eigener Entwurf).
- **Unser Design:** `HudMode` (`crates/f29-hud/src/hud.rs`), Taste H blättert
  NAV → AG → AA. Der Modus ist ein **Filter der Zielart** für Rahmen und
  Richtungstasten und selbst reine Anzeige: Die Taste sendet nur die Zielart mit
  (`PICK_AIR`, `PICK_RUNWAY`), die Simulation hält je Art eine Wahl – so bleiben
  Replays bitgleich, und beim Umschalten ist die Wahl der Art wieder da.
  - **Vierecke** nach den Stufen (M-013), gleich groß. Im HUD wie im Original nur
    das gewählte Ziel (alle wären zu unübersichtlich): doppelter, heller Rahmen,
    darüber Name bzw. Typ, darunter Sensor (Boden), Entfernung und Annäherung
    (Jäger) bzw. Seite und Entfernung (Landebahn), außen ein Randpfeil; erfasst
    mit Ecken. Die Karte zeigt alle der Modusart: gemeldet gestrichelt, entdeckt
    gedämpft, gewählt doppelt und hell, erfasst mit Ecken. TGT zeigt das gewählte
    Ziel des Modus (auch Jäger mit Kamerabild, Landebahn).
  - **Ein Erfassungskreis je Modus, eine Regel:** Liegt das gewählte Ziel im Kreis
    (AG ±6°, AA ±10°) und übernähme die Waffe es, ist es erfasst (M-014); die
    Kanone erfasst Jäger bis 2 km.
  - **NAV** (Start): Flugdaten, Navigationspunkt, Konfiguration, mit Fahrwerk in
    der Luft Gleitlinie und Sinkratenskala; wählbar nur Landebahnen (Name,
    „FRIENDLY“ bzw. „HOSTILE“, „NEUTRAL“ vorgesehen, Entfernung). **AG:**
    Bodenziele, Erfassungskreis (bei gewählter Kamera ihr Bildkreis, M-032),
    Kanonenkreuz, Bombenkreis bzw. Feuersignal. **AA:** Jäger, Erfassungskreis,
    der gewählte mit Typ, Entfernung (km), Annäherung (m/s), blau, sobald erfasst;
    mit Kanone die Vorhaltemarke. In AG und AA unten links die Waffe, rechts
    Radar, Blickrichtung, Entfernung; das Glas ist breit.
  - **Waffenwahl** (Tab): GUN → BAY 1 → … → BAY 4 → GUN, Startwahl ist die
    Kanone. Die Auslösetaste (Leertaste) feuert die gewählte Kanone, solange
    gehalten, sonst löst sie die Schachtwaffe aus; die Rücktaste feuert stets die
    Kanone.
- **Abweichungen:** Zielfilter mit eigener Wahl je Art und fester Erfassungskreis
  statt Rahmen nach Wirkungswert; Waffenart und Station auf einer Taste.
- **Status:** umgesetzt.

### M-020 Radar und Entdeckung (Stealth)

- **Original:** Wie leicht der Gegner das Flugzeug entdeckt, hängt von dessen
  eigener Sichtbarkeit ab: einer Zahl, die jedes Bild aus Flughöhe (höher =
  sichtbarer), Querlage, offenem Waffenschacht und eingeschaltetem Störsender
  wächst. Jede Radarstellung bewertet das Flugzeug mit einem Wert, der mit der
  Entfernung linear fällt, von der Geländeart unter dem Flugzeug abhängt (grobe
  Maskierung über Kartenzellen, keine echte Sichtlinie: Die Zelle trägt eine von
  vier Stufen, mit der der Entfernungsteil der Bewertung malgenommen wird,
  unabhängig von der Flughöhe; die unterste blendet das Radar ganz aus) und bei
  Doppler-Radaren
  vom Aspekt. Übersteigen Bewertung und Sichtbarkeit zusammen eine Schwelle,
  steigt die Alarmstufe der Stellung; ab einer zweiten meldet sie das Flugzeug,
  später schießt sie. Der Störsender erhöht die Sichtbarkeit, halbiert aber das
  Anwachsen der Alarmstufe und lenkt radargelenkte Flugkörper ab. Das Cockpit
  zeigt Bewertung gegen Sichtbarkeit als zwei Balken und die Reichweite
  gegnerischer Radare.
- **Referenzstelle:** f19re `src_en/egcombat.c` (Bewertung je Bedrohung,
  Alarmstufen), `src_en/egframe.c` (eigene Sichtbarkeit pro Bild).
- **Unser Design:** `crates/f29-core/src/radar.rs`.
  - **Stellungen:** Frühwarnradar (Impuls, 60 km, Umlauf 10 s; eigene
    Zielklasse abseits der Route), SAM-Feuerleitradar (Doppler, 30 km, 5 s),
    Kurzstrecken-Flugabwehr (Doppler, 12 km, 2,5 s). Eine zerstörte Stellung
    strahlt nicht mehr und vergisst. Feindliche Radare stehen mindestens
    35 km von der Heimatbasis, damit schon das Anrollen unentdeckt bleibt.
  - **Pings:** Jede Stellung pingt einmal je Antennenumlauf (versetzt je
    Stellung); nur dann wird gerechnet. Die Impulsstärke am Flugzeug fällt
    linear mit der Entfernung bis zur Reichweitengrenze. Doppler zählt nur die
    Bewegung auf die Stellung zu oder von ihr weg (reiner Querflug 25 %),
    Impuls ist richtungsunabhängig. Auch am Boden kommen Pings an.
  - **Gelände** (`radar::ground_factor`, Spielerentscheidung v0.3): Die
    Geländeart unter dem Flugzeug aus dem Grobraster (M-031), so weich
    gemischt und verzerrt wie die Landschaft (`Terrain::ground_mix`, kein
    Sprung an Zellgrenzen), verkürzt die Reichweite jedes Radars, auch des
    Bordradars feindlicher Jäger: Hügel auf 70 %, Gebirge auf 45 %, Stadt auf
    80 %, Wald auf 85 %, Dorf auf 90 % (Bodenechos), Wasser, Flachland und
    Wüste gar nicht; die schmalen Ketten erreichen den vollen Wert nur über
    dem Kamm. Voll gilt das bis
    100 m über Grund, bis 1000 m läuft es aus. Weil die Impulsstärke linear
    mit der Entfernung fällt, entdeckt jedes Radar dort im gleichen Verhältnis
    erst näher – nie gar nicht. Die Anzeigen (SIG-Säule, Pings) zeigen die
    verkürzte Stärke; die Reichweitenkreise bleiben beim Nennwert.
  - **Handbuch:** `docs/handbuch/radar.md` beschreibt die Mechanik für
    Spieler und nennt alle Stellschrauben.
  - **Stealth-Level** = 1 − Sichtbarkeit, ein Wert für alle Radare wie im
    Original. Sichtbarkeit = Grundwert 0,10 + Höhe über Grund bis 0,40
    (Arkustangens über (Höhe − 100 m) / 1500 m: 1000 m ≈ 0,14, 3000 m ≈ 0,28)
    + Querlage bis 0,30 bei 90° (Rückenflug wie waagerecht) + offener Schacht
    0,15 + eigenes Radar 0,15 + Störsender 0,15. Nicklage, Fahrwerk und
    Klappen zählen nicht – das Original kennt sie nicht. Höchstens 90 %:
    Ganz unsichtbar ist das Flugzeug nie. Kurz: tief und waagerecht fliegen.
  - **Alarmstufe:** Ein Ping, dessen Impuls stärker ist als das
    Stealth-Level, hebt die Stufe der Stellung um eins: 1 „RADAR DETECTED“
    (Reichweitenkreis auf der Karte), 2 „TRACKED“ (Ping-Ort gemerkt, Ziel von
    Patrouillen), 3 „RADAR ID“ (Raketenstart, M-021). Ohne Treffer sinkt die
    Stufe nach 30 s um eins; bei 0 „LOST CONTACT“.
  - **Beispiele:** Tief, waagerecht und clean (Stealth 90 %) über Flachland
    entdeckt eine SAM-Stellung das Flugzeug erst unter 3 km, ein
    Frühwarnradar unter 6 km (über Gebirge unter 1,4 bzw. 2,7 km); mit offenem
    Schacht (75 %) unter 7,5 bzw. 15 km (über Gebirge 3,4 bzw. 6,8 km);
    in 3000 m (≈ 62 %) unter etwa 11 bzw. 23 km, dort mit 90° Querlage
    (≈ 32 %) unter etwa 20 bzw. 41 km.
  - **Anzeigen:** SIG-Säule zwischen den MFDs mit einem Balken für das
    Stealth-Level; jeder Ping erscheint eine Sekunde als Querstrich in seiner
    Stärke – rot über dem Balken ein Treffer, gelb darunter keiner. Drei
    Alarmlampen, Radartyp PLS/DOP, Pieps des Radarwarnempfängers (Treffer
    doppelt und höher), Reichweitenkreise und gemerkter Ping-Ort auf der
    Karte.
  - **Testoptionen:** `--no-stealth` (Stealth-Level immer 0, jeder Ping in
    Reichweite trifft) und `--no-damage` (Treffer werden gemeldet, richten
    aber keinen Schaden an; ein Absturz bleibt tödlich); beide stehen im
    Replay.
- **Abweichungen:** Radare sehen durch das Gelände – die Geländeart verkürzt
  nur ihre Reichweite, und nur im Tiefflug (das Original rechnet sie in jeder
  Höhe), ohne Stufe, die ganz ausblendet; anders als unser optisches System
  braucht kein Radar eine Sichtlinie; eine feste Stufenleiter statt fein dosierter
  Alarmzuwächse; der Störsender bremst die Alarmstufe nicht, er wirkt nur auf
  Sichtbarkeit und Raketen (M-021); eine Säule statt zwei Balken.
- **Status:** umgesetzt.

### M-021 Boden-Luft-Bedrohungen

- **Original:** Radargeführte Flugabwehrraketen bedrohen das Flugzeug je nach
  Entdeckung und Reichweite. Ist die Alarmstufe einer Stellung fast voll,
  meldet sie „Radar ID“ und schießt bei den folgenden Bewertungen, wenn das
  Flugzeug in der Aufschaltreichweite ihres Typs ist, ihr Startplatz frei ist
  (eine Rakete je Stellung) und nicht zu viele fliegen (Grenze steigt mit der
  Konfliktstufe); reine Radarstellungen ohne Werfer schießen nicht, Munition
  ist unbegrenzt. Jeder Typ hat Aufschaltreichweite, Höchstgeschwindigkeit,
  Drehrate, Lenkart (Infrarot oder Radar in mehreren Generationen) und eine
  Flugzeit etwa für die Aufschaltreichweite; die Langstreckenrakete startet
  fast senkrecht. Die Rakete verfolgt das Ziel rein in Kurs und Steigwinkel
  (auf niedrigen Konfliktstufen träger), zielt um einen beim Annähern
  schrumpfenden Zuschlag darüber (Bogen), lenkt nur auf Ziele in einem
  Suchkegel (geprüft in der Peilung) und gibt auf, wenn das Ziel weit hinter
  ihr liegt. Weit weg (mehrere km) lenkt sie halbaktiv nur, solange ihre
  Stellung das Flugzeug verfolgt, sonst ungelenkt; nah sucht sie selbst, ohne
  Endphase mit mehr Leistung. Treffer sind rein geometrisch (näher als ihr
  Flugschritt, Höhe gewichtet), nicht gewürfelt, und beschädigen Bordsysteme,
  mit der Trefferzahl wachsend; das Triebwerk fällt nie aus, verliert aber
  nach seiner Beschädigung je Treffer Höchstschub und läuft nur bis dahin
  hoch. Leuchte und Warnton melden eine nahe gelenkte Rakete. Düppel (gegen
  Radar) und Fackeln (gegen Infrarot) sind begrenzt, höchstens drei in der
  Luft (ein weiterer Wurf zählt nur den Vorrat herunter), leer „erschöpft“.
  Ein Täuschkörper steht am Abwurfort und brennt einige Sekunden (auf höheren
  Stufen kürzer); im Suchkegel einer Rakete passender Lenkart bricht er ohne
  Würfeln ihre Aufschaltung – sie fliegt geradeaus und schaltet wieder auf,
  wenn er den Kegel verlässt und das Flugzeug noch darin liegt. Gegen moderne
  Radarlenkung wirkt er nur, wenn das Flugzeug ungefähr quer zur Rakete
  fliegt (Doppler-Kerbe, mit der Konfliktstufe enger). Der Störsender
  (zeitgesteuert mit Vorrat, länger als ein Täuschkörper) erhöht die
  Sichtbarkeit fest und halbiert das Anwachsen der Alarmstufen (M-020);
  Radarraketen verlieren durch ihn die Aufschaltung außerhalb einer mit der
  Konfliktstufe wachsenden Durchbrenn-Entfernung, ältere auf jede. Ein
  Infrarotstörer wirkt entsprechend.
- **Referenzstelle:** f19re `src_en/egcombat.c` (Flugkörper der Bedrohungen:
  `fireGroundThreat`, `updateThreatTargeting`, `samCanAcquireTarget`,
  `bombTarget`), `src_en/egframe.c` (Täuschkörper, `countermeasures`),
  `src_en/egflight.c` (Wirkung der Schäden).
- **Unser Design:** Feindliche Raketen sind dieselben Flugkörper wie die
  eigenen (M-014): gleiche Liste (`Munitions`), gleicher Flug und
  Annäherungszünder, Ziel ist das Flugzeug (`crates/f29-core/src/weapons/`,
  Start in `Sim::launch_sams`). Flak gibt es nicht (vom Spieler gestrichen).
  - **Start:** mit „RADAR ID“ (Stufe 3, M-020) und bei jedem weiteren Treffer
    auf Stufe 3 – wenn das Flugzeug in Reichweite ist, die Stellung keine
    Rakete in der Luft hat und das Regelset es zulässt (M-033): höchstens zwei
    feindliche Raketen zugleich (SAMs und Jäger zusammen), im offenen Krieg
    vier; im Kalten Krieg nur über feindlichem Gebiet. Munition unbegrenzt.
    Meldung „MISSILE LAUNCH SA-6 - <Stellung>“.
  - **Vier Typen** (`targets::air_defense`, `sam_of`; Werte in `spec.rs`):

    | | SA-13 Gopher | SA-6 Gainful | SA-11 Gadfly | SA-5 Gammon |
    |---|---|---|---|---|
    | Stellung | Kurzstrecken-Flugabwehr | SAM-Stellung | jede dritte SAM | Frühwarnradar |
    | Suchkopf, Reichweite | Infrarot, 10 km | Radar, 25 km | Radar, 25 km | Radar, 60 km |
    | Fahrt, Drehrate | 650 m/s, 60°/s | 650 m/s, 45°/s | 800 m/s, 50°/s | 450 m/s, 30°/s |
    | Suchkegel, Resistenz | ±22°, 5 % | ±26°, 15 % | ±35°, 50 % | ±26°, 5 % |
    | Start, Bogen | direkt, 0 | 45°, 1,5 s Boost, 0,1 | 45°, 1 s Boost, 0,05 | 85°, 4 s Boost, 0,25 |
    | Wirkradius, Flugzeit | 18 m, 20 s | 30 m, 50 s | 30 m, 40 s | 45 m, 150 s |

    Die SA-13 steht als Fahrzeug bei Zielgruppen und Horsten; die SA-6 ist der
    Allrounder; die SA-11 sieht breiter und übergeht die Hälfte der
    Täuschkörper; die SA-5 kommt über jeden Berg und von weit her, ist aber
    lange unterwegs – Zeit für Täuschkörper. Wirkradien und Drehraten sind
    bewusst kräftig, damit sich Störer und Täuschkörper lohnen.
  - **Flug:** Start im Startwinkel des Typs, mindestens 10° über der Linie
    zum Flugzeug (im Suchkegel), im Boost ungelenkt, gelenkt frühestens nach
    0,4 s. Dann zielt sie um Bogen × Entfernung über das Ziel (schrumpft beim
    Annähern auf null) und mit **Vorhalt** auf den Treffpunkt (Ort plus
    Geschwindigkeit × Restflugzeit, höchstens 10 s) – wie alle Lenkwaffen;
    bei stehenden Zielen ist das die reine Verfolgung. Suchkegel gelten nur in
    der Peilung, so findet auch eine steil gestartete Rakete ihr Ziel.
    Radarlenkung ist **halbaktiv**: Weiter als 5 km lenkt die Rakete nur, wenn
    ihre Stellung das Flugzeug beim letzten Antennenumlauf erfasst hat
    („painted“) – Stealth wirkt also auch im Anflug; näher lenkt ihr eigener
    Suchkopf. Die SA-13 hat den optischen Suchkopf der Maverick (Kegel,
    Reichweite, Sichtlinie) und braucht keine Beleuchtung: Nach dem Start
    helfen Gelände und Fackeln, nicht Stealth. Fällt das Flugzeug aus dem
    Suchkegel, ist es endgültig verloren; fehlende Beleuchtung, Störer und
    Täuschkörper wirken nur vorübergehend. Zünden kann die Rakete nur, solange
    ihr Suchkopf das Flugzeug hat; im Gelände bleibt sie wirkungslos.
  - **Gefährlichkeit:** Querflug soll das meiste abschütteln, Täuschkörper
    ergänzen mit einer gewissen Chance (Wunsch des Spielers). Mit Vorhalt
    zielt die Rakete vor das Flugzeug; liegt es weiter neben ihrer Nase als
    ihr Suchkegel, verliert sie es – umso eher, je langsamer sie gegenüber dem
    Flugzeug ist. Von vorn treffen fast alle; querab schüttelt man SA-5 und
    SA-13 sicher ab, die SA-6 meist, die SA-11 nicht (gegen sie helfen nur
    viele Düppel). Ein Ausweichhaken im letzten Moment ist bewusst keine
    Mechanik: Die F-29 baut zu langsam Last auf, um noch aus dem Wirkradius
    zu kommen. Prüfstand `crates/f29-core/tests/support/sam_report.rs`
    (Grundlage von `tests/sam.rs`), Aufruf
    `cargo run --release -p f29-core --example sam_report`: Trefferquoten
    vorn, hinten (Flucht), querab, mit einem bzw. drei Täuschkörpern und
    „T.+querab“ (werfen und abdrehen).
  - **Treffer** (M-015): Ein Raketentreffer (Stärke 1) verschlechtert zwei
    zufällige, verschiedene Systeme: intakt → halbe Leistung, eingeschränkt →
    ausgefallen. Das Triebwerk wird wie im Original nur „beschädigt“ und fällt
    nie aus; ab dann fehlen ihm 8 % Höchstschub je Trefferstärke aller Treffer,
    auch früherer (nach fünf Raketentreffern bleiben 60 %). Mit jedem Treffer
    rüttelt das Flugzeug stärker (bis sechs, dieselbe Störung wie Turbulenz,
    M-006), beschädigte Anzeigen flackern unregelmäßig; kein Lebensbalken.
    Meldung „SA-6 HIT“. Am Schubhebel sperrt eine rote Marke den verlorenen
    Bereich („MAX 84“ in Rot); der Hebel fällt darauf zurück und lässt sich
    nicht darüber schieben.
  - **Täuschkörper:** **Düppel** (Taste C) gegen Radar-, **Fackeln** (Taste T)
    gegen Infrarot- bzw. optische Lenkung, je 20; jeder Tastendruck wirft einen
    am Flugzeug aus (nicht am Boden), beliebig viele in der Luft; leer „CHAFF
    EMPTY“ bzw. „FLARES EMPTY“. Er steht und brennt 5 s. Liegt er im Suchkegel
    (Peilung) und in Reichweite einer Rakete passender Lenkart, **blendet** er
    sie („SA-6 DECOYED“): Sie fliegt geradeaus und hält ihr Ziel; danach hat
    sie das Flugzeug wieder, wenn es in ihrem Kegel liegt, sonst ist es
    verloren – also früh werfen und abdrehen. Sie übergeht ihn mit der
    Resistenz ihres Typs, einmal je Paar aus Rakete und Täuschkörper
    gewürfelt. Täuschkörper lenken nur Raketen gegen ihre Seite ab (unsere
    Fackeln keine eigene Maverick, die der Jäger unsere Luft-Luft-Raketen,
    M-022) und wirken nicht auf das Stealth-Level. Anzeige: HUD „CH 20 FL 20“,
    WPN-Seite, Karte (Kreuz bzw. Kreis), 3D-Wolke bzw. glühender Punkt.
  - **Störer:** **ECM** (Taste M) ein/aus gegen Radar-, **Infrarotstörer**
    (Taste U) 30 s gegen Infrarotlenkung, danach 60 s Abklingzeit; beide ohne
    Vorrat. Eine feindliche Rakete der gestörten Lenkart lenkt nicht auf das
    Flugzeug, solange sie weiter als 2 km weg ist (nah brennt sie durch);
    Täuschkörper wirken weiter. ECM kostet Sichtbarkeit wie der offene
    Schacht (M-020), der Infrarotstörer nichts. HUD „ECM“, „IRJ 25“, beim
    Abklingen „IRJ -40“.
  - **Warnung und Anzeige:** Solange eine feindliche Rakete das Flugzeug
    verfolgt, blinkt „MISSILE“ im HUD und ein schneller Wechselton warnt; ein
    Start beendet den Zeitraffer (M-034). Karte: feindliche Raketen als Strich,
    radargelenkte rot, optische orange; 3D mit Rauchspur. Detonationen
    (`f29-audio`): tiefer, von 90 auf 30 Hz fallender Bass mit übersteuertem,
    dunklem Krachen und Knistern – am Ziel lang und kräftig, Fehlschuss kurz
    und dumpf, am eigenen Flugzeug laut und metallisch, halbe Lautstärke bei
    1,5 km (Hörprobe `f29-headless sound` → `07c_detonationen.wav`).
  - **Suchköpfe** (alle Lenkwaffen): Ziele sind Bodenziele (sie stehen), das
    Flugzeug oder Jäger; Täuschkörper blenden nur. **Optisch**/Infrarot mit
    Sichtlinie (Maverick, SA-13, AIM-9, AA-8), **Radar** ohne Sichtlinie
    (gegen Bodenziele nur Schiffe, Harpoon; gegen Flugzeuge weiter als ein
    Tabellenwert halbaktiv: Radar-SAMs, AA-10 5 km, AIM-120 nie), **Anti-Radar**
    (HARM), **Laser** (GBU-12). Luft-Luft-Raketen beider Seiten sind damit nur
    Tabelleneinträge.
- **Abweichungen:** Vorhalt statt reiner Verfolgung (sonst rettet das
  Hineindrehen in die Rakete, im Luftkampf das Falscheste); ein Ziel außerhalb
  des Suchkegels ist endgültig verloren, damit Querflug abschüttelt. Start
  schon mit „RADAR ID“, damit sich Störer und Täuschkörper lohnen. Eigene
  Startprofile, teils breitere Suchkegel; Treffer im Wirkradius des Typs
  statt im Flugschritt; mehr Schubverlust je Treffer (Wunsch des Spielers).
  Täuschkörper mit gewürfelter Resistenz je Typ statt Lenkart-Generationen
  und Doppler-Kerbe, beliebig viele in der Luft (viele opfern, um eine Rakete
  doch noch abzuschütteln). Störer ohne Vorrat, eine Durchbrenn-Entfernung für
  alle; ECM bremst die Alarmstufen nicht (Aufwand ohne großen Nutzen). Die
  Konfliktstufe wirkt nur über das Regelset (M-033) auf Zahl und Erlaubnis der
  Starts, nicht auf Drehrate, Brenndauer oder Durchbrenn-Entfernung.
  Raketenwarnung ab dem Start, nicht erst in der Nähe.
- **Status:** umgesetzt.

### M-022 Feindliche Flugzeuge

- **Original:** Feindliche Jäger stehen auf feindlichen Flugplätzen (und
  Trägern) bereit und starten erst bei Alarm: wenn ein Radar am Boden oder im
  Flugzeug das Flugzeug erfasst; gemerkt wird der Ort des Kontakts. Liegt er
  vom Platz aus in Reichweite, startet ein bereitstehender Jäger mit
  Beschleunigung und Steigflug („<Platz> – <Typ> startet“). Statt eines
  Flugmodells ein Bewegungsmodell: Quer- und Nicklage folgen Sollwerten, die
  Kursänderung der Querlage, die Fahrt dem Typ (in steilen Kurven etwas
  weniger); wer zu tief sinkt, zieht hoch. Die Typen unterscheiden sich in
  Höchstfahrt, Reichweite (zugleich Flugdauer), Wendigkeit (Querlage,
  Rollrate), Radar und Bewaffnung. Bei frischem Alarm fängt der Jäger ab (zum
  gemerkten Ort, in der Höhe des Flugzeugs); nah dran wählt er sein Manöver
  aus einer Tabelle nach Aggressivität (aus Konfliktstufe und Nummer des
  Jägers, drei Stufen), Lage des Flugzeugs vor ihm und Kurswinkel zueinander
  (je acht Sektoren): Kurve bestimmter Stärke, Hochziehen mit pendelnder
  Querlage oder Abschwung, nicht unter eine Mindesthöhe. Weiter weg kurvt er
  auf das Ziel zu, bei vielen Bedrohungen dreht er ab, im Nahkampf fliegt er
  schneller. Ohne frischen Alarm patrouilliert er etwas höher als das
  Flugzeug zu einem eigenen Objekt ungefähr in dessen Richtung; Rotten fliegen
  in Formation, zu weit weg gibt er auf. Nach der Flugdauer landet er auf dem
  nächsten eigenen Platz („landet“) und steht wieder bereit. Sein Radar pingt
  wie eine Radarstellung, nur in einem Kegel um die Nase (einige Typen
  rundum); Treffer häufen sich an, über einer Schwelle gibt es Alarm und, nah
  genug (nachts kürzer), „Sichtkontakt <Typ>“; Störsender schwächen das
  Anhäufen. Seine Rakete (ein Typ der SAM-Tabelle, u. a. AA-2, AA-8, AA-10,
  AA-7; Infrarot oder Radar, auch „modern“; in derselben Liste) startet er
  ohne Vorrat im Takt seines Radars, wenn es das Flugzeug erfasst hat, dieses
  ungefähr vor der Nase, in Erfassungsreichweite und nicht zu nah ist (die
  Mindestentfernung sinkt mit der Konfliktstufe), sein Raketenplatz frei ist
  (eine je Jäger) und nicht zu viele Bedrohungen laufen („<Rakete> abgefeuert
  von <Typ>“). Sie lenkt wie eine SAM: weit weg nur, solange sein Radar das
  Flugzeug hält. Sehr nah und fast genau voraus (Kurs und Nicklage eng), bei
  schon länger laufendem Alarm, schießt er mit der Kanone in kurzen
  Abständen; kommt ihm eine Rakete nah, fliegt er eine harte Kurve zur Seite.
  Getroffen zieht er eine Rauchspur, stürzt ab und zählt als Abschuss; die
  Konfliktstufe macht die Jäger aggressiver, zahlreicher und schneller.
  Sidewinder und AMRAAM tragen dieselben Schächte wie alles andere und stehen
  in derselben Raketentabelle; die AMRAAM erfasst deutlich weiter, die
  Sidewinder ist deutlich wendiger. Das Cockpit zeigt die stärkste
  Radarbedrohung: Balken für die Stärke, Bogen in ihrer Richtung (um die
  Blickrichtung eines Radars mit Kegel bzw. die Peilung einer verfolgenden
  Bedrohung), ihren Namen und einen Warnton, lauter, sobald sie erfasst.
- **Referenzstelle:** f19re `src/egcombat.c` (`updateObjects` samt Kampfteil,
  `spawnEnemyAircraft`, `fireAirThreat`, `updateThreatAlert`), Tabellen der
  Flugzeug- und Raketentypen in `src/stubs.c`.
- **Unser Design:** `crates/f29-core/src/fighters.rs`; Raketen, Kanonen und
  Täuschkörper der Jäger in `crates/f29-core/src/sim/mod.rs` (`launch_air`,
  `fighter_guns`, `mark_threats`, `fighter_decoys`).
  - **Plätze:** Feindliche Fliegerhorste und ihre Jäger stehen in der
    Gebietsdatei (M-031); das eingebaute Gebiet hat RED 1 bis RED 3 mit je
    drei MiG-21 und zwei MiG-29 (Bahn 2,5 km, auf der Karte rot). In der Luft
    höchstens vier Jäger zugleich, im offenen Krieg sechs (Regelset, M-033).
  - **Typen** (reale Vorbilder, eigene Werte):

    | | MiG-21 | MiG-29 |
    |---|---|---|
    | Fahrt, Steigrate | 290 m/s, 110 m/s | 300 m/s, 150 m/s |
    | Querlage, Rollrate | 60°, 70°/s | 75°, 120°/s |
    | Flugdauer | 15 min | 20 min |
    | Radar (Umlauf 2 s) | Impuls 15 km, ±30° | Doppler 35 km, ±60° |
    | Rakete, Kanone (M-016) | AA-8 Aphid, GSh-23L | AA-10 Alamo, GSh-30-1 |
    | Widerstand (M-015) | 0,625 | 1 |

  - **Start:** bei Alarm – jüngster Ping-Ort einer Radarstellung ab Stufe 2
    (M-020) oder Radarkontakt eines Jägers in den letzten 15 s –, wenn der Ort
    innerhalb der halben Flugstrecke des Typs (Fahrt × Flugdauer / 2) liegt;
    je Platz rollt einer nach dem anderen. Meldung „RED 1 - MIG-29 TAKEOFF“.
  - **Flug** als Bewegungsmodell wie im Original: Querlage mit der Rollrate,
    koordinierte Kurve, Steigrate zur Sollhöhe, mindestens 250 m über Grund.
    Abfangen zum Kontaktort in dessen Höhe, dort Patrouille als Rennbahn
    (darüber, 12 km geradeaus, wenden – auf dem Rückweg schaut das Radar ihn
    an). Hat sein Radar das Flugzeug in den letzten 15 s erfasst, verfolgt er
    es direkt („MIG-21 RADAR CONTACT“); sein Kontakt alarmiert auch die
    anderen. Nach der Flugdauer fliegt er heim, gilt über dem Platz als
    gelandet („RED 1 - MIG-21 LANDED“) und steht wieder bereit.
  - **Eigenes Radar:** pingt im Umlauf (je Jäger versetzt) wie eine
    Radarstellung gegen dasselbe Stealth-Level (M-020), nur im Kegel um die
    Nase (Peilung); ein Treffer im letzten Umlauf beleuchtet das Flugzeug für
    die AA-10.
  - **Luft-Luft-Raketen** beider Seiten (Flug wie M-021):

    | | AIM-9 | AIM-120 | AA-8 | AA-10 |
    |---|---|---|---|---|
    | Suchkopf | optisch | Radar | optisch | Radar, halbaktiv |
    | Reichweite, Kegel | 8 km, ±25° | 30 km, ±30° | 7 km, ±22° | 25 km, ±25° |
    | Fahrt, Drehrate | 850 m/s, 40°/s | 1000 m/s, 30°/s | 750 m/s, 40°/s | 850 m/s, 30°/s |
    | Wirkradius, Flugzeit | 9 m, 25 s | 12 m, 60 s | 12 m, 20 s | 15 m, 45 s |
    | Resistenz | 60 % | 70 % | 10 % | 15 % |

  - **Raketen der Jäger:** Die AA-8 lenken Fackeln ab, die AA-10 (weiter als
    5 km nur, solange ihr Jäger das Flugzeug mit seinem Radar hält) Düppel.
    Gestartet wird im Takt seines Radars, wenn es das Flugzeug gerade erfasst
    hat und dieses innerhalb 30° vor der Nase und der Reichweite mal Anteil
    nach Können (unten) liegt, aber nicht näher als 1,5 km (im offenen Krieg
    1 km, M-033); eine Rakete je Jäger, Gesamtgrenze und Feuererlaubnis wie
    bei den SAMs (M-021), Vorrat unbegrenzt. Meldung „MISSILE LAUNCH AA-8 -
    MIG-21“; Treffer wie SAM-Treffer (M-015).
  - **Kanone:** feuert, solange er verfolgt und das Flugzeug näher als 1,2 km
    und innerhalb 3° vor der Nase liegt – in Feuerstößen (Länge nach Können,
    1,5 s Pause), unbegrenzt (M-016). Meldung „MIG-21 GUN HIT“, Leuchtspur
    sichtbar.
  - **Pilotenkönnen** (eigene Ergänzung): je Jäger beim Missionsstart aus dem
    Seed gleich verteilt gewürfelt, von der Gegnerqualität verschoben (Green
    −1, Regular 0, Veteran +1, Elite +2 Stufen; M-036):

    | | Feuerstoß | Täuschkörper ab | Rakete bis |
    |---|---|---|---|
    | Anfänger | 0,3 s | 1 km | volle Reichweite |
    | Erfahren | 0,6 s | 2 km | 70 % |
    | Ass | 1,0 s | 3,5 km | 50 % |

  - **Abschuss:** Eine Rakete zerstört; die Kanone summiert Trefferstärken bis
    zum Widerstand (M61A1: MiG-21 5, MiG-29 8 Treffer). Er stürzt mit Rauchspur
    ab (rollt ab, senkt die Nase), bleibt als Wrack liegen und startet nie
    wieder; Meldung „MIG-21 DESTROYED“.
  - **Ausweichen:** Verfolgt ihn eine eigene Rakete näher als 6 km, dreht er
    auf den näheren Kurs querab zu ihr; die MiG-29 wechselt je Rakete mit dem
    Abschwung (querab in den Tiefflug, wo Gelände den optischen Suchkopf
    abschatten kann). Ist die Rakete näher, als sein Können vorgibt, wirft er
    jede Sekunde einen passenden Täuschkörper (Fackel gegen optische, Düppel
    gegen Radarsuchköpfe), höchstens vier; ob sie folgt, entscheidet ihre
    Resistenz (M-021). Die Kurve allein hilft gegen AIM-9 und AIM-120 von vorn
    kaum (ihr Vorhaltwinkel bleibt im Suchkegel); härtere Kurven oder engere
    Suchkegel schwächten die Täuschkörper, den eigentlichen Schutz der Jäger.
  - **Eigene Luft-Luft-Raketen:** AIM-9 Sidewinder (vier je Schacht) übernimmt
    nur einen optisch entdeckten Jäger und braucht freie Sicht, AIM-120 AMRAAM
    (drei je Schacht) sucht mit eigenem Radar; beide Fire and Forget.
    Übergabe: der gewählte Jäger im 10°-Kreis und in Reichweite („erfasst“,
    Ecken), sonst der nächste im Suchkegel. Von vorn trifft ohne Täuschkörper
    jede, mit ihnen etwa jede zweite (`crates/f29-core/tests/air_to_air.rs`).
  - **Auswahl und Anzeige:** Im AA-Modus wählen I/J/K/L Jäger (M-017), ohne
    Wahl der nächste entdeckte in Blickrichtung; die Wahl bleibt, solange ein
    eigenes System ihn entdeckt (M-013). Karte: roter Pfeil in Flugrichtung,
    Kreis, solange er verfolgt; nur gemeldete oder entdeckte Jäger. 3D
    (`crates/f29-app/src/fighters.rs`): eigener Low-Poly-Deltajäger bis 15 km
    (nachts 5 km), in der Ferne mindestens gut ein Retro-Bildpunkt und ohne
    Dunst, damit er nicht erst im letzten Moment auftaucht.
- **Abweichungen:** Keine Manövertabelle: Die Jäger fliegen direkt auf das
  Flugzeug zu und weichen nur querab bzw. im Abschwung aus; kein Abdrehen bei
  vielen Bedrohungen, kein schnellerer Nahkampf. Patrouille als Rennbahn über
  dem Kontaktort statt zu eigenen Objekten, ohne Rotten und ohne Aufgeben
  wegen Entfernung; keine Träger. Das Jägerradar häuft nicht an: Ein Ping über
  dem Stealth-Level ist Kontakt, eine Sichtkontakt-Meldung gibt es nicht, der
  Störsender schwächt es nicht (er macht nur sichtbarer, M-020). Das Können
  wird je Jäger gewürfelt und regelt Feuerstoß, Täuschkörper und
  Startentfernung statt des Manövers; die Konfliktstufe wirkt nur über das
  Regelset (Zahl in der Luft, Mindestentfernung, Feuererlaubnis), nicht auf
  Aggressivität und Fahrt. Die Kanone feuert, solange er verfolgt, ohne
  Bedingung an die Dauer des Alarms. Kein Radarwarner mit Richtung der
  Bedrohung (Entscheidung des Spielers).
- **Status:** umgesetzt.

### M-031 Einsatzgebiete und Missionsgenerator

- **Original:** Vier feste Einsatzgebiete (Libyen, Persischer Golf, Nordkap,
  Mitteleuropa) mit Weltdatei, Geländekacheln und Kartenbild; jede Mission
  entsteht aus den festen Objekten eines Gebiets. Alle vier Gebiete sind
  gleich groß: ein Quadrat von gut 500 km Kantenlänge, abzulesen an der
  Entfernungsanzeige im Cockpit und an der Rasterabfrage. Die quadratische
  Karte mit festem Maßstab ist für Briefings in Planquadrate geteilt: zwei Buchstaben für
  ein großes Quadrat, zwei Ziffern für das Feld darin (Ost, dann Nord), je
  Gebiet mit eigenen Anfangsbuchstaben und Versatz, sodass die Kennungen wie
  echte Kartenangaben der Gegend wirken. Das Briefing nennt so die Lage der
  Ziele, der Startbasis und des Landeplatzes; die Karten im Flug zeigen keine
  Kennungen. Ein Grobraster trägt je Zelle Land oder
  Wasser (Ziele nur auf Land), eine Geländeart, die die Radarbewertung direkt
  verstärkt oder schwächt (grobe „Maskierung“ ohne Sichtlinie), und ein Merkmal
  für tagsüber spontan startende tragbare Flugabwehr. Die Objektliste nennt
  Anlagen, Startbasen (eigene Plätze und Träger, Start in Deckshöhe), Stellungen
  mit Radar und SAM-Typ und Flugzeugeinheiten (Patrouillen mit Basis, Typ,
  Treibstoff); Geländekacheln sind fertige Ausschnitte (Orte, Anlagen, Hügel) in
  mehreren Detailstufen. Das Grobraster ist in allen Gebieten gleich fein,
  16 × 16 Zellen von gut 30 km. Der Generator nimmt Primär- und Sekundärziel aus der
  Liste oder als Zufallspunkt auf Land, der zum nächsten Geländemerkmal rutscht,
  nicht zu weit auseinander, und eine eigene Basis nahe den Zielen (etwas
  Zufall, Vorrang für Träger); liegt die Gesamtstrecke außerhalb eines Bands,
  wird neu gewürfelt (begrenzt oft). Den Missionstyp regelt eine Tabelle nach
  Zielklasse, Gebiet und Kampagnenstufe; ein Gebiet hat zusätzlich feste
  Missionen. **Namen:** Jedes Objekt verweist auf zwei Einträge einer
  Namensliste des Gebiets, meist Ort und Art der Anlage, zusammengesetzt und
  bei Überlänge gekürzt. So heißt es in den Meldungen im Flug (eine Stellung
  startet eine Rakete, ein Flugplatz einen Jäger), auf der Kartentafel beim
  gewählten Objekt, bei der Heimatbasis und beim erfassten Ziel; die beiden
  Aufträge beschreibt das Briefing zusätzlich mit je einem Absatz aus einer
  eigenen Textdatei. Ortsbeschriftungen auf der Karte gibt es nicht.
- **Referenzstelle:** f19re `src_start/stgen.c` (`missionGenerate`,
  `parseWorld`, `runGenerator`, `findOrPlaceItem`, `formatGridRef`),
  `src_start/stparse.c` (Geländekacheln), `src_en/egcombat.c` (Rasterzellen in
  der Radarbewertung, `spawnSamThreat`), Maßstab aus `src_en/egtarget.c`
  (`buildRangeString`). Die Gebietsdateien (`*.wld`, `*.3dT`,
  `*.xxx`) liegen der Rekonstruktion nicht bei; ausgewertet wurden nur Aufbau,
  Maßstab und Regeln, keine Positionen. Namen: `src/egframe.c`
  (`buildStoreName`), `src_en/egcombat.c` (Meldungen), `src_start/stutil.c`
  (Briefing).
- **Unser Design:** Gebiete als Daten (Crate `f29-world`), vom Original die
  Struktur (Raster, Objektliste, Generatorregeln); die vier Gebiete des
  Originals (Libyen, Persischer Golf, Nordkap, Mitteleuropa) werden nach der
  echten Geografie mit echten Namen von Hand bzw. im Karteneditor angelegt
  (Spielerentscheidung v0.3). Das Spiel erzeugt keine Gebiete; das Format
  beschreibt `docs/GEBIETSFORMAT.md`.
  - **Gebietsdatei** (`data/theaters/*.ron`, Formatversion 3, eingebaut das
    Testgebiet „TEST RANGE“): Name, Seed, Grobraster 128 × 128 mit Gelände (`~ . h M
    d f C v`: Wasser, Flachland, Hügel, Gebirge, Wüste, Wald, Stadt, Dorf; je
    Art Höhenprofil und Farbe, Spielerentscheidung v0.3) und Seite (`F N H`: eigen, neutral,
    feindlich), Flugplätze (eigene Basis, Horste mit ihren MiG-21 und MiG-29)
    und alle Objekte mit Name, Klasse, Lage, Ausrichtung, „zivil“ und Ort
    (angezeigt „NAME AT ORT“, z. B. „RADAR AT BENGHAZI“). Das Raster
    formt die Landschaft (`Terrain::from_theater`: je Geländeart ein Höhenprofil,
    weich überblendet, verzerrte Zellgrenzen, darüber das Detailrauschen;
    Überblenden und Verzerrung in Zellen, Höhenformen in Metern); die
    Seiten (außerhalb neutral) gelten für Landung, Ausstieg (M-032) und Feuer
    (M-033). Größe und Raster sind fest (`f29_world::theater::SIZE_M`,
    `GRID_N`): jedes Gebiet 500 km im Quadrat, Zellen von knapp 4 km
    (Spielerentscheidung v0.3, wie im Original für alle Gebiete gleich).
  - **Missionsgenerator** (`f29_core::mission`): zwei Ziele aus den nichtzivilen
    Gebäuden, Bunkern, Brücken, abgestellten Flugzeugen, Panzern und
    ungeschützten Zielen (keine Radarstellungen, keine Schiffe), 80–220 km von
    der Heimatbasis und 20–80 km auseinander; je Ziel erst die Auftragsart, dann
    ein passendes Ziel (M-032), bis zu 50 Würfe; das nähere ist das Primärziel.
    Route: Startbasis → Primär- → Sekundärziel → Startbasis; landen darf man auf
    jedem Platz, das Gebiet entscheidet. `f29-headless airfields` listet Plätze,
    Route und Auftrag.
  - **Gebiete wählen:** Das Spiel liest beim Start alle Dateien aus
    `data/theaters/` (`--theaters ORDNER`), nach Name sortiert, und bietet sie
    im REGION-Menü an (höchstens vier); `--region NAME` startet direkt in
    einem. Ohne lesbare Datei bleibt das eingebaute Testgebiet. Die
    Pilotenakte merkt sich das Gebiet. `f29-headless theater-png` zeigt eine
    Vorschau einer Datei.
  - **Testgebiet** „TEST RANGE“ (`data/test/test_range.ron`, eingebaut):
    jede Geländeart, jede Seite, Heimatbasis, drei feindliche Horste, jede
    Objektklasse, etwa 90 Objekte; Grundlage aller Tests
    (`docs/GEBIETSFORMAT.md`, Abschnitt Testgebiet). Es löste das einmalig
    erzeugte NORTH COAST ab.
  - **Laden statt Backen:** Das Spiel liest die Gebietsdatei und rechnet die
    Landschaft daraus beim Laden, der Editor mit demselben Code für die
    Vorschau. Ein vorab gerechnetes Höhenbild gibt es nicht. Fertige Gebiete
    sollen später ins Programm eingebettet werden (Roadmap).
  - **Karte:** MFD-Kartenbild 4096² über das ganze Gebiet, parallel berechnet;
    Kartenstufen 0,625–640 km, außerhalb schwarz. Replays speichern Name und
    Prüfsumme des Gebiets.
  - **Planquadrate** (`f29_core::nav::grid_square`): 100-km-Quadrate mit zwei
    Buchstaben (Spalte von West, Zeile von Süd, „A“ am Gebietsrand), darin
    10-km-Felder mit zwei Ziffern (Ost, Nord), z. B. „CC84“. Das Gitter liegt
    auf vollen 10 bzw. 100 km um den Ursprung, also auf den Linien des
    MFD-Kartengitters; die Kanten des 500-km-Gebiets schneiden die äußeren
    Quadrate (A–F). Angegeben in der Lagebesprechung bei den Zielen und je
    Wegpunkt im Flugplan, auf ihrer Karte das 100-km-Gitter mit den
    Buchstaben (eingezoomt unter 60 km halber Breite auch die 10-km-Felder),
    auf der MSN-Seite das eigene Planquadrat neben dem Gebiet.
- **Abweichungen:** vorerst nur das Testgebiet; feste Startbasis, kein Träger;
  Namen aus Art und Ort im Objekt selbst statt einer Namensliste je Gebiet,
  kein eigener Beschreibungsabsatz je Auftrag; Ziele
  nur aus der Objektliste; das Band gilt für den Abstand zur Basis, nicht für
  die Gesamtstrecke; keine festen Missionen; keine Zellen für spontane tragbare
  Flugabwehr; Radare sehen durch das Gelände, die Geländeart verkürzt nur
  im Tiefflug ihre Reichweite (M-020). Planquadrate mit eigener
  Größe, Buchstaben ab „A“ statt gebietstypischer Kennungen, zusätzlich auf
  der Karte der Lagebesprechung und der MSN-Seite; Richtung und Entfernung von
  der Basis bleiben im Briefing.
- **Status:** teilweise (es fehlt ein grafischer Gebietseditor).

### M-032 Missionsauftrag, Bewaffnung und Auswertung

- **Original:** Der Generator legt zu den Zielen Auftragsart,
  Bewaffnungsvorschlag und Tageszeit fest; nach dem Flug wertet das Spiel
  Ausgang und Flugprotokoll aus.
  - **Aufträge** je Ziel nach einer Tabelle (Zielart, Gebiet, Konfliktstufe):
    zerstören, fotografieren (Kamera im Schacht), Sonderausrüstung absetzen,
    Fracht über einem Ort abwerfen bzw. an einen Ort liefern (mit Zeitlimit),
    Flugzeuge abfangen (Transporter auf dem Weg zu oder von einem Ort, ein
    Flugzeug über einem Ort). Die Tageszeit folgt aus den Zielen: manche
    Aufträge fliegen immer nachts, Fotoaufträge nie.
  - **Bewaffnungsvorschlag:** je eine Luft-Luft-Waffe kurzer und mittlerer
    Reichweite, eine Anti-Radar-Rakete, abwechselnd Lenkraketen oder Bomben, bei
    weiter Strecke statt einer Waffe ein Zusatztank; je Ziel wird ein Schacht
    ersetzt (Kamera, Sonderausrüstung bzw. eine zufällige, gegen die Zielklasse
    gut wirkende Waffe). Vor dem Start lässt sich umrüsten.
  - **Kamera:** Ausrüstung, kein Geschoss, Blick fest etwas unter die Nase. Mit
    gewähltem Bodenziel zeigt die Zielanzeige ihr Bild; der Auslöser macht „Bild
    n“ (kurze Verschlusszeit, Klickton, ohne Vorratsgrenze). Es zählt am
    stehenden Aufklärungsziel, wenn eine Summe aus Kursabweichung, Abweichung des
    Blickwinkels nach unten und Entfernung unter einer mit der Konfliktstufe
    engeren Grenze liegt (nah heran, Nase genau darauf): „Ziel fotografiert“ im
    Protokoll. Ohne gewähltes Ziel findet sie ein Objekt, wo ihr Blick auftrifft.
  - **Ausgang:** gelandet (eigener Platz), ausgestiegen (misslingt bei hoher
    Fahrt oder starker Neigung) oder tot; dann zählt das Gebiet darunter:
    Ausstieg über Feindesland heißt Gefangenschaft, Landen in neutralem Gebiet
    „internationaler Zwischenfall“.
  - **Wertung:** Das Protokoll (Zerstörungen mit Zielart, Abschüsse, Wegpunkte,
    Entdeckung durch Radar bzw. Sicht, Ausstieg, Landung) wird der Reihe nach
    nach Tabellen je Kampagnenstufe bewertet: am meisten fürs Primärziel
    (zerstört, fotografiert, beliefert), weniger fürs Sekundärziel, nachträglich
    mehr, wenn auch das Primärziel fällt. Sonst nach Art: bewaffnete
    Militäreinheiten und Flugzeuge positiv (Jäger, Transporter, Hubschrauber
    verschieden), eigene Einheiten, Ziele im eigenen Gebiet und zivile negativ,
    unbewaffnete je nach Stufe gar nicht oder als unerlaubter Angriff. Jede
    Identifizierung kostet; wer genau den Entdecker später zerstört, bekommt den
    Abzug zurück. Wegpunkte außerhalb des eigenen Gebiets bringen Kleinstpunkte
    (je nach Stufe jeder oder nur ein Teil). Ein negativer Zwischenstand wird bei
    Landung bzw. Ausstieg null. Faktoren: Gebiet, Missionsart, Schwierigkeit,
    Konfliktlage, nach einem Ausstieg der Ausgang (gerettet, gefangen,
    Zwischenfall); lange Flüge zählen in Zeitstufen leicht mehr.
  - **Nachbesprechung:** ein Bild zum Ausgang, dann die Karte mit dem Flugweg,
    Ereignis für Ereignis mit Stelle, Uhrzeit, Text, Primär-/Sekundärziel und
    Zwischenwertung; am Ende Gesamtwertung und Ausgangstext (Ziele erreicht oder
    nicht, Landung, Ausstieg, Tod, Zwischenfall). Danach Statistik, Beförderung
    und Orden (M-030) und eine Abschlussseite mit Bild und Zufallstext.
- **Referenzstelle:** f19re `src_start/stgen.c` (`runGenerator`, Ende:
  Vorschlag der Bewaffnung), `src_start/stutil.c` (`drawStoreIcons`,
  Bewaffnungsbildschirm), `src_en/egtarget.c` (Kamera, Zielanzeige),
  `src_end/enbrief.c` (`calcMissionScore`), `src_end/enmain.c` (Ablauf der
  Nachbesprechung), `src_end/endevt.c` (Ausgangstexte, Gebiet am Landeort),
  `src/egkeys.c` (Schleudersitz).
- **Unser Design:** `f29_core::mission`, `flightlog.rs`, `sim/score.rs`,
  `f29_hud::debrief`.
  - **Aufträge:** Angriff oder Aufklärung je Ziel, erst die Art, dann ein Ziel;
    fotografierbar sind Gebäude, Bunker, Brücken und abgestellte Flugzeuge
    (Radarstellungen nicht: zu gefährlich nah). Aufklärung je Ziel: Kalter Krieg
    immer, begrenzt 50 %, offener Krieg 20 %; ohne passendes Ziel Angriff. Nacht
    nur mit `--night` (M-013) und nie mit einem Aufklärungsziel.
  - **Beladungsvorschlag** (`Mission::loadout`, ohne `--loadout`): AIM-9, HARM,
    je Ziel die Kamera (nur einmal) bzw. GBU-12 gegen Gebäude, Bunker, Brücken,
    sonst Maverick; reicht der Treibstoff nicht (M-036), Zusatztank statt HARM.
  - **Kamera** (`cam`, KS-87): bleibt im Schacht; offene Klappen, im Flug,
    Querlage wie bei jeder Schachtwaffe (M-014). Der Auslöser macht „PHOTO n“
    (Verschlussklang), unbegrenzt, höchstens alle 0,8 s. Sie blickt 10° unter die
    Rumpflängsachse; im AG-HUD ist ihre Sichtlinie mit dem Bildkreis von 5° das
    Zielinstrument. Ein Foto zählt, wenn das gewählte Ziel steht, optisch
    entdeckt, höchstens 1,5 km entfernt und im Bildkreis ist; beim
    Aufklärungsziel „PRIMARY/SECONDARY PHOTOGRAPHED“. Die TGT-Seite zeigt im
    AG-Modus ihr Bild (Bildhöhe = Bildkreis; sonst BAY CLOSED bzw. ON GROUND),
    Ziel, Entfernung, Fotozahl und „IN FRAME“, wenn ein Foto jetzt zählen würde.
  - **MSN-Seite:** STRIKE bzw. RECON, je Ziel ATK/RCN mit Name und Entfernung
    bzw. DONE, Regelset und Gebiet unter dem Flugzeug (M-033).
  - **Flugprotokoll** (`LogEntry`, mit Tick): Start (von der Bahn), Wegpunkte
    (3 km, ohne Start und Landeplatz), Entdeckung (je Radarstellung bzw. Jäger
    beim ersten Mal), Radar-ID, Zerstörungen, Fotos, Abschüsse, Treffer,
    ROE-Verletzungen, Landung, Ausstieg, Pilot unten, Wrack, Absturz – die eine
    Quelle für Auftrag, Einsatzregeln und Wertung. Alle Teilsysteme melden in
    einen Ereignisstrom für Protokoll, Meldungen, Klang und Effekte.
  - **Missionsende:** nach einem Flug auf einer Bahn zum Stehen gekommen; das
    Gebiet entscheidet (eigen MISSION COMPLETE, neutral INTERNATIONAL INCIDENT,
    feindlich CAPTURED). **Schleudersitz** Strg+E: gerettet bis 450 kt und 60°
    Quer- bzw. Längsneigung, sonst tödlich; dann ebenso das Gebiet.
  - **Ausstieg in Bildern:** ohne HUD und Brett, die Kamera am Piloten. Der Sitz
    stößt ihn mit 30 m/s hoch, dann fällt er taumelnd wie die Fallschirmbombe
    (Masse, Widerstand); nach 2 s öffnet sich der Schirm, er sinkt mit gut 6 m/s,
    pendelt leicht (kein Wind), Kopfblick per Maus. Das Flugzeug stürzt in einer
    Spirale ab (Rauch, Explosion). Ende, wenn der Pilot unten ist (am Schirm
    Zeitraffer, M-034), misslungen nach 1,5 s; alles in der Simulation (Replays).
  - **Wertung** (`Sim::score`), je Protokolleintrag der Reihe nach:

    | Eintrag | Punkte |
    |---|---|
    | Primärziel erledigt (zerstört bzw. fotografiert) | +500 |
    | Sekundärziel | +200; mit erledigtem Primärziel +400 (fällt es später, bekommt seine Zeile die Differenz) |
    | Radar- oder Flugabwehrstellung / anderes militärisches Ziel | +50 / +25 |
    | MiG-21 / MiG-29 abgeschossen | +100 / +150 |
    | Wegpunkt außerhalb des eigenen Gebiets | +5 |
    | Entdeckung je Radar bzw. Jäger (erstes Mal) | −Abzug des Regelsets (M-033); zurück, wenn man genau diesen Entdecker zerstört |
    | ziviles Ziel zerstört | −200 (keine zusätzliche ROE-Verletzung: ein Abzug je Tat) |
    | ROE-Verletzung | −100 |

    Die Summe ist mindestens 0; darauf der Reihe nach die Faktoren **Ausgang**
    (auf eigenem Gebiet gelandet ×1, über eigenem Gebiet ausgestiegen ×0,5,
    neutrales Gebiet ×0,25, gefangen, tot oder abgestürzt ×0), **Regelset**
    (M-033), **Gegnerqualität** und **Flugverhalten** (M-036) und **Flugzeit**
    ab dem Start (ab 20 / 30 / 40 min ×1,1 / ×1,2 / ×1,3 – der Treibstoff reicht
    für weite Wege, nicht fürs Trödeln).
  - **Nachbesprechung** (Bildschirme wie die Menüs, M-036): MISSION DEBRIEF
    (Ausgang, Aufträge DONE / NOT DONE), MISSION EVENTS (Zeit, Text, Punkte; 14
    je Bildschirm), MISSION RATING (Ereignissumme, Faktoren, Gesamt), mit einem
    Piloten aus dem Dienstplan zuletzt die Ehrung (M-030). Die Auswahl
    „CONTINUE“ (Leertaste bzw. Auslöser) blättert, danach geht es mit einer neuen
    Mission zum Dienstplan; R fliegt neu, Escape beendet. `f29-headless replay`
    gibt die Wertung mit aus.
- **Abweichungen:** nur Angriff und Aufklärung (Flugzeuge als Missionsziele
  später, M-036); die Tageszeit wählt nicht der Generator; der Vorschlag hat
  keine Luft-Luft-Rakete mittlerer Reichweite und keinen Zufall; die Kamera
  prüft feste Grenzen (Entfernung, Bildkreis) statt einer mit der Konfliktstufe
  engeren Summe und nur am gewählten, optisch entdeckten Ziel; Ausgangsfaktor
  für jeden Ausgang; keine Faktoren für Gebiet und Missionsart (es gibt je nur
  eines), die Schwierigkeit über Gegnerqualität und Flugverhalten; keine eigenen
  Einheiten; Abschüsse nur von MiG-21 und MiG-29; keine Karte mit dem Flugweg
  in der Nachbesprechung (Wunsch des Spielers).
- **Status:** teilweise (es fehlen Bilder zum Ausgang; Beförderung und Orden:
  M-030).

### M-033 Regelsets (Konfliktlage)

- **Original:** Die Konfliktstufe der Mission – von kaltem bis offenem Krieg –
  bestimmt an vielen Stellen, wie gefährlich der Gegner ist (gleichzeitige
  Bedrohungen, Mindestentfernung der Jägerraketen, Aggressivität, Anstieg der
  Alarmstufen) und wie die Mission bewertet wird.
- **Referenzstelle:** keine.
- **Unser Design:** `f29_core::rules` (`Conflict`, `Rules`), je Mission eines
  (`--rules cold|limited|war`, Menü „Auftrag“, im Replay); Standard ist der
  offene Krieg.

  | | Kalter Krieg | Begrenzt | Offener Krieg |
  |---|---|---|---|
  | Gegner feuern | nur über feindl. Gebiet | überall | überall |
  | feindliche Raketen zugleich | 2 | 2 | 4 |
  | Jäger in der Luft | 4 | 4 | 6 |
  | Jägerraketen nicht unter | 1,5 km | 1,5 km | 1 km |
  | Abzug je Entdeckung (M-032) | 30 | 10 | 0 |
  | Wertungsfaktor (strenger = schwerer) | ×1,5 | ×1 | ×0,75 |
  | Aufklärung je Ziel (M-032) | immer | 50 % | 20 % |

  - **Kalter Krieg:** Jäger fangen ab und verfolgen, SAMs und Jäger (Raketen
    und Kanone) feuern aber nur über feindlichem Gebiet. Angreifen darf man nur,
    was uns laut Protokoll entdeckt hat (Radarstellung mit Alarm, Jäger mit
    Radarkontakt); jeder Treffer auf anderes Militärisches ist eine Verletzung
    der Einsatzregeln („ROE VIOLATION“, −100), zivile Ziele kosten ohnehin ihren
    eigenen Abzug. Die Aufträge sind nur Aufklärung.
  - **Gebiet** unter dem Flugzeug aus dem Raster (M-031); die MSN-Seite zeigt
    Regelset und FRIENDLY/NEUTRAL/HOSTILE, das Briefing das Regelset mit seinen
    Einschränkungen. Das Können der Jäger regelt die Gegnerqualität (M-036).
- **Abweichungen:** Aggressivität und Anstieg der Alarmstufen hängen nicht vom
  Regelset ab (feste Stufenleiter, M-020).
- **Status:** umgesetzt.

### M-034 Zeitraffer

- **Original:** Per Taste läuft die Simulation beschleunigt, eine zweite Taste
  schaltet zurück; im Cockpit steht ein Hinweis. Der Raffer endet von selbst
  beim Ausfahren des Fahrwerks, beim Start einer feindlichen Rakete und wenn
  eine Bedrohung stark genug wird.
- **Referenzstelle:** f19re `src/egkeys.c` (`recalcTimeScale`,
  `exitTimeAccel`, Tastenbehandlung), `src/egcombat.c` (Abbruch bei Bedrohung
  und Raketenstart), `src/egtacmap.c` (Anzeige).
- **Unser Design:** Shift+T schaltet 1× → 2× → 4× → 1× (Joystick:
  `time_warp`), Anzeige „2X“/„4X“ (`f29-app`, `sim.rs`). Das Frontend rechnet
  je Echtzeit-Takt entsprechend mehr 120-Hz-Takte; die Simulation bleibt
  unverändert, das Replay enthält nur die Takte. Abbruch
  (`Sim::interrupts_time_warp`): Fahrwerk draußen (dann lässt er sich auch
  nicht einschalten), Raketenstart gegen das Flugzeug (SAM oder Jäger),
  steigende Alarmstufe einer Radarstellung, Treffer (Rakete oder Kanone), Ende
  der Mission; nach dem Ausstieg darf am Schirm gerafft werden.
- **Abweichungen:** eine Taste schaltet reihum durch zwei Stufen; jede
  steigende Alarmstufe bricht ab statt einer Bedrohungsschwelle; auch Treffer
  brechen ab.
- **Status:** umgesetzt.

### M-035 Blick aus dem Cockpit

- **Original:** Tasten schalten den Blick aus dem Cockpit nach vorn, links,
  rechts und hinten; die Kamera bleibt am Flugzeug und dreht nur gegenüber dem
  Rumpf. Außer vorn kein HUD.
- **Referenzstelle:** f19re `src/egkeys.c` (Ansichtstasten), `src/egflight.c`
  (Kamerawinkel je Ansicht, HUD nur vorn).
- **Unser Design:** Strg+I/J/K/L = vorn, links, hinten, rechts (angeordnet wie
  die Zieltasten); Shift+F1 setzt auf vorn zurück, auch aus der
  Verfolgeransicht. Seitlich und hinten Vollbild ohne Brett und HUD, mit einer
  festen, gefüllten Silhouette: seitlich Cockpitrand und Kanzelbogen, hinten
  Sitz, Kopfstütze und Kanzelrahmen (`f29-app`, `view.rs`, `cockpit.rs`). Nur
  Darstellung, nicht in Simulation und Replay.
- **Abweichungen:** keine; die Silhouetten sind eigene Formen.
- **Status:** umgesetzt.

### M-030 Karriere, Szenarien, Orden

- **Original:** Ein Pilot fliegt eine Folge von Missionen in mehreren
  Einsatzgebieten und bei unterschiedlichen Konfliktstufen. Er sammelt Punkte,
  steigt im Rang auf, erhält Orden und kann gefangen genommen werden oder
  fallen. Statistik, Beförderung und Orden folgen auf die Nachbesprechung
  (M-032); der Dienstplan führt die Piloten (M-036).
  - **Beförderung:** sieben Ränge vom Second Lieutenant bis zum Brigadier
    General. Nach jedem Einsatz prüft die Nachbesprechung die Schwellen von
    oben bis zum eigenen Rang, jede aus Gesamtpunkten, Zahl der Einsätze und
    Punkteschnitt je Einsatz; erfüllt der Pilot eine davon ganz, steigt er um
    genau einen Rang. Der höchste Rang kommt nur mit dem letzten Einsatz vor
    dem Ruhestand, der bei einer festen Höchstzahl an Einsätzen folgt; ein
    tödlicher Ausgang geht dem Ruhestand vor.
  - **Ablauf der Ehrung:** nach der Wertung ein Bild mit Rangabzeichen und
    Ordensleiste, dazu je Ereignis ein Satz: Orden (mit Verwundung),
    Einsatzspange, zurückgehaltener Orden, Beförderung, verzögerte
    Beförderung – oder, wenn nichts davon geschah, dass der Pilot im Rang
    bleibt. Nach einem tödlichen Ausgang heißt es „posthum“.
  - **Orden:** fünf Stufen, vergeben allein nach der Wertung des einzelnen
    Einsatzes (nicht nach der Summe der Laufbahn). Je Einsatz höchstens ein
    Orden: der höchste, dessen Schwelle erreicht ist. Dieselbe Stufe noch
    einmal verlangt ein Vielfaches ihrer Schwelle (beim zweiten Mal das
    Doppelte, beim dritten das Dreifache usw.); verfehlt ein Einsatz die
    nächste Schwelle der hohen Stufen, kann er eine niedrigere bringen. Die
    höchste Auszeichnung gibt es nur einmal (danach zählen die Stufen
    darunter), die niedrigste nur begrenzt oft. Für eine Verwundung einmalig
    eine eigene Auszeichnung, auch zusätzlich zu einem Orden; alles auch
    posthum. Dazu Spangen für erreichte Einsatzzahlen (zwei Paare, die höhere
    ersetzt die niedrigere) und eine für den ersten erfolgreichen Übungsflug.
    Im Übungsflug nennt die Nachbesprechung nur, was es gegeben hätte.
  - **Gefangennahme:** Ein anstehender Orden bleibt aus und zählt auch nicht
    für die nächste Schwelle (die Gefangennahme hat die Tarnkappentechnik
    preisgegeben), die Beförderung verzögert sich; der
    Pilot bleibt aktiv. Tod: Status gefallen, eigener Todesbildschirm mit der
    Ursache.
- **Referenzstelle:** f19re `src_end/enbrief.c` (`checkPromotion`,
  `checkAwardCodes`, Todesbildschirm), `src_end/endevt.c` (Meldungen zu
  Orden, Gefangennahme, Ruhestand), `src_end/enmain.c` (Ablauf, Status).
- **Unser Design:** Aufbau wie im Original, Rang- und Ordensnamen aus dem
  Original (Spielerentscheidung), eigene Schwellen nach unserer Wertung
  (M-032), Tabellen in `f29_core::roster`. Die Akte (`Pilot`) führt Rang,
  Punkte, Einsätze, Status, Orden je Stufe und das Purple Heart; die Bänder
  folgen aus der Zahl der Einsätze. `Pilot::fly` verbucht einen Einsatz
  (`Mission`: Wertung, Ausgang, Treffer, Einstellungen) und meldet, was er
  bewirkt hat (`Report`).
  - **Maßstab** (gemessen an 300 erzeugten Missionen je Regelset, Route
    400–480 km, rund 35 min Flug): ein voller Erfolg (beide Ziele, heil
    gelandet, REGULAR, REALISTIC) bringt im OPEN WAR rund 820, LIMITED 1090,
    COLD WAR 1640 Punkte; Gegnerqualität ×0,75–1,5 und Flugverhalten
    ×0,75–1 kommen dazu. Tod und Gefangennahme bringen 0 Punkte.
  - **Ränge:** 2ND LT, 1ST LT, CAPTAIN, MAJOR, LT COLONEL, COLONEL, BRIG
    GENERAL. Beförderung in den nächsten Rang ab (Einsätze, Gesamtpunkte,
    Schnitt): 3 / 1500 / 300, 8 / 5000 / 450, 15 / 12 000 / 600, 25 / 22 000 /
    700, 40 / 36 000 / 800, 60 / 45 000 / 700. Ruhestand nach 60 Einsätzen,
    tot nach Absturz oder misslungenem Ausstieg (geht vor).
  - **Orden** (Punkte des Einsatzes; der n-te ab dem n-Fachen): AIRMAN'S
    MEDAL 250 (höchstens neunmal), DISTINGUISHED FLYING CROSS 600, SILVER STAR
    1000, AIR FORCE CROSS 1500, CONGRESSIONAL MEDAL OF HONOR über 2000
    (einmal). PURPLE HEART einmal, sobald das eigene Flugzeug getroffen
    wurde.
  - **Bänder:** LONGEVITY SERVICE RIBBON ab 5 Einsätzen, ab 15 WITH OAK LEAF
    CLUSTER; EXPEDITIONARY SERVICE RIBBON ab 30, ab 50 WITH OAK LEAF CLUSTER.
  - **Ehrung** (`f29_hud::debrief::honors`, nach MISSION RATING): Titel Rang
    und Name, darunter PROMOTED TO, PROMOTION DELAYED BY CAPTURE, AWARDED
    (Orden, Purple Heart), das neue Band, RETIRED FROM ACTIVE DUTY oder NO
    PROMOTION OR DECORATION – nach dem Tod „POSTHUMOUSLY“; darunter alle
    Auszeichnungen, höchste zuerst, Orden mit Anzahl. Vorerst nur Text;
    Rangabzeichen, Orden und Bänder als Grafik liefert der Spieler nach. Die
    Seite zeigt den Einsatz an einer Kopie der Akte, eingetragen wird er erst
    beim Übergang zum Dienstplan (M-036).
- **Abweichungen:** Schwellen eigene. Namen der Einsatzbänder eigene (die des
  Originals stehen nur in seinen Spieldaten). Purple Heart für einen Treffer
  am eigenen Flugzeug (die Bedingung des Originals ist in der Rekonstruktion
  nicht lesbar). Kein Übungsflug, also keine Übungsspange und keine
  Hinweise, was es gegeben hätte. Ein zurückgehaltener Orden nach
  Gefangennahme entfällt, weil sie bei uns ohnehin keine Punkte bringt.
  Einsatzgebiete und Szenarien folgen (M-031).
- **Status:** teilweise (Laufbahn umgesetzt; es fehlen weitere Einsatzgebiete
  und die Grafiken der Ehrung).

### M-036 Menüführung vor dem Flug

- **Original:** Zwischen den Flügen führt eine feste Folge von Bildschirmen zum
  Start; aus der Nachbesprechung geht es zurück an den Anfang.
  1. **Flugzeugerkennung:** eine Silhouette einem Namen zuordnen; falsch heißt
     nur Übungsflüge (eigentlich der Kopierschutz, in der englischen Fassung
     fest auf „richtig“ gepatcht).
  2. **Dienstplan:** zehn Plätze, je Zeile Rang und Name, Gesamtpunkte,
     Zahl der Einsätze und Status (aktiv, im Ruhestand, gefallen). Eine Zeile
     wählen übernimmt den Piloten samt seiner Einstellungen (Gebiet,
     Konfliktlage, Missionsart, Schwierigkeit) in die folgenden Menüs; Piloten
     im Ruhestand oder gefallene lassen sich nicht wählen, eine kurze Meldung
     sagt warum. Mit der zweiten Auswahltaste gewählt (welche, zeigt die
     Rekonstruktion nicht), legt eine Zeile einen neuen Piloten an und
     überschreibt den bisherigen Eintrag – ein eigenes Löschen gibt es nicht.
     Der Name kommt per Tastatur mit blinkender Schreibmarke (Rücktaste nimmt
     ein Zeichen zurück, Escape leert, Enter übernimmt); der neue Pilot beginnt
     im niedrigsten Rang ohne Punkte und Einsätze. Die Dienstplandatei hält
     alle Plätze und den zuletzt gewählten Piloten; sie wird beim Öffnen des
     Dienstplans geladen und gleich wieder gespeichert, nachdem der Eintrag des
     zuletzt geflogenen Piloten zurückgeschrieben ist.
  3. **Auftrag:** die aktuelle Wahl annehmen oder der Reihe nach in je einer
     Liste ändern (kostenlos): Gebiet (vier), Konfliktstufe (drei), Missionsart
     (Luft-Luft, Angriff, Übung), Gegnerqualität (vier Stufen), Flugverhalten
     (kein Absturz, leichte oder realistische Landungen).
  4. **Lagebesprechung:** Gebietskarte mit Start, Primär-, Sekundärziel und
     Landeplatz; Menü mit Zieltexten (je Auftrag eine Seite mit Ziel, Ort,
     Gitterkoordinate, ggf. Frist), Radarstellungen (Kreise, durchgezogen
     Doppler, gepunktet Puls), Raketenreichweiten, Flugplätzen, Flugplan,
     besonderen Ereignissen und „Raum verlassen“. Die Mission entsteht beim
     ersten Betreten.
  5. **Bewaffnung:** Waffe einem Schacht zuweisen auf einem Bild der Unterseite
     mit den vier Stationen und je einem Waffensymbol, dazu benötigter gegen
     mitgeführten Treibstoff (geschätzt allein aus der Routenlänge, im Briefing
     „Fuel Estimate“). Vorschlag: je ein Schacht für Primär- und Sekundärziel,
     ab einer bestimmten Länge Zusatztreibstoff in einem der anderen.
  6. **Schlussauswahl:** neue Mission, Lagebesprechung, Bewaffnung, Start.
     „Neue Mission“ kostet nichts (gilt nur als „unpatriotisch“) – so lässt sich
     eine ungünstige Mission (weite Strecke, Kamera belegt einen Schacht)
     tauschen.
  7. Nach dem Flug: Nachbesprechung, Beförderung und Orden (M-030), dann
     zurück zum Dienstplan. Die Nachbesprechung (ein eigenes Programm) ändert
     nur den Eintrag des Piloten im Arbeitsspeicher: zu Beginn ein Einsatz
     mehr, dann Punkte, Beförderung und Orden, am Ende der Status – im
     Ruhestand mit Erreichen der Höchstzahl an Einsätzen, gefallen nach einem
     tödlichen Absturz. In die Datei gelangt das erst beim nächsten Öffnen des
     Dienstplans. Wer das Spiel vorher beendet (Alt+Q, etwa auf dem
     Todesbildschirm), verwirft den Einsatz: Er zählt nicht, die Laufbahn
     bleibt geschützt – ein bekannter Kniff der Spieler. Übungsflüge zählen
     nicht.
- **Referenzstelle:** f19re `src_start/stmain.c` (Zustandsautomat der
  Bildschirme), `src_start/stmenu.c` (Erkennung, Dienstplan `sub_193EE`,
  Auftragswahl, Auswahllisten, Lagekarte), `src_start/stpinp.c`
  (Dienstplandatei, Namenseingabe), `src_start/stmap.c` (Auswahl mit zweiter
  Taste), `src_end/enmain.c` (Ablauf der Nachbesprechung, Status), `src_start/stutil.c` (`printMission`,
  Briefingtext; `drawStoreIcons`, Bewaffnung). Bewaffnung und Schlussauswahl
  sind in der Rekonstruktion leer, die Menütexte liegen nur in den Spieldaten;
  diese Teile nach dem Handbuch (gamesdatabase.org, „F-19 Stealth Fighter“).
- **Unser Design:** Ablauf eins zu eins (`f29-app/src/menu.rs`); `--fly`,
  `--start` oder `--screenshot` überspringen die Menüs. Mit Menü ist die erste
  Mission zufällig, sonst fest (`--seed` gilt immer); im Menü steht die
  Simulation still, trägt aber schon die Mission.
  - **Ablauf:** Titelbild (nur beim Start) → Erkennung (Platzhalter) →
    Dienstplan → Auftrag (ACCEPT MISSION oder CHANGE CHOICES durch die Listen) →
    INTELLIGENCE BRIEFING → FINAL CHOICES (SELECT NEW MISSION, INTELLIGENCE
    BRIEFING, ARM YOUR PLANE, BEGIN MISSION; im Textfeld die Übersicht des
    Briefings). Neue Mission
    = nächster Seed mit denselben Einstellungen und vorgeschlagener Beladung,
    zurück zum Dienstplan; ebenso nach der Nachbesprechung. REGION bietet die
    geladenen Gebiete an (M-031), die Missionsart vorerst einen Eintrag;
    Konfliktlage = Regelset (M-033).
  - **Dienstplan** (`f29_core::roster`, `f29-app/src/roster.rs`): Die Tabelle
    der zehn Plätze ist die Auswahl (Kopfzeile PILOT, POINTS, MISSIONS,
    STATUS; gewählte Zeile invers, aktive Piloten hell, andere gedämpft; die
    Auswahl unten bleibt leer). Bestätigen auf einem aktiven Piloten übernimmt
    Konfliktlage, Gegnerqualität und Flugverhalten aus seiner Akte und führt
    zum Auftrag; auf einem leeren Platz beginnt die Namenseingabe; bei einem
    Piloten im Ruhestand oder einem gefallenen sagt die Statuszeile warum. Entf
    oder rechte Maustaste auf einem belegten Platz fragt nach (REPLACE PILOT
    oder CANCEL, vorgewählt CANCEL), dann folgt die Namenseingabe: A–Z, 0–9,
    Leerzeichen, Bindestrich und Punkt, groß, höchstens 16 Zeichen, feste
    Schreibmarke; Rücktaste nimmt zurück, Enter übernimmt, Escape bricht ab
    (der alte Eintrag bleibt). Der neue Pilot beginnt im niedrigsten Rang ohne
    Punkte und Einsätze mit den eingestellten Werten; der Dienstplan bleibt
    offen. In die Akte kommt ein Einsatz erst beim Übergang von der
    Nachbesprechung zum Dienstplan: ein Einsatz mehr, die Punkte der Wertung
    (M-032), die geflogenen Einstellungen; gefallen nach Absturz oder
    misslungenem Ausstieg, nach einer Gefangennahme bleibt der Pilot aktiv.
    Wer vorher mit Escape oder Alt+Q beendet, verwirft den Einsatz wie im
    Original. Datei `roster.ron` (RON) im Datenordner des Benutzers – Linux
    `$XDG_DATA_HOME` bzw. `~/.local/share/f29-silent-sniper/`, macOS
    `~/Library/Application Support/f29-silent-sniper/`, Windows
    `%APPDATA%\f29-silent-sniper\` – oder `--roster DATEI`; gespeichert nach
    jeder Änderung. Eine unlesbare Datei bleibt unangetastet (dann speichert
    das Spiel nicht). Rang, Orden und Ruhestand: Laufbahn (M-030). Ältere
    Dateien ohne Orden lassen sich weiter lesen.
  - **Gegnerqualität** (`Opponents`): verschiebt das Können der Jäger (M-022)
    um −1 / 0 / +1 / +2 Stufen (GREEN, REGULAR, VETERAN, ELITE = alle Asse);
    Wertung ×0,75 / ×1 / ×1,25 / ×1,5.
  - **Flugverhalten** (`Landings`, Landegrenzen M-042): realistisch = unsere
    Grenzen; leicht = Sinkrate, Quer- und Nicklage ×1,5 (gut bis 6 m/s, Bruch ab
    10,5 m/s), ohne Fahrwerk weiter Bruch; kein Absturz = Aufsetzen auf der Bahn
    zerstört nie (auch ohne Fahrwerk), keine Turbulenz in Bodennähe. Wertung ×1
    / ×0,9 / ×0,75. Beide stehen in `Spawn` und damit im Replay.
  - **Lagebesprechung** (`f29_hud::intel`, Texte `Sim::briefing_*`): rechts im
    Textfeld die nordausgerichtete Karte des Einsatzraums (Route mit Rand) über
    dem Höhenbild des Geländes, immer mit Basis, gestrichelter Route und den
    beiden Zielen; links der Text zum Thema am Cursor. Themen: TARGETS (je Ziel
    Auftrag, Name, Entfernung und Richtung von der Basis, Kamera nötig; auf der
    Karte hervorgehoben mit P/S und Name), THREATS (Einsatzregeln, Stellungen
    entlang der Route je Typ mit Radar- und Raketenreichweite, feindliche
    Fliegerhorste; auf der Karte alle Radarkreise, gepunktet Impuls,
    durchgezogen Doppler, und die Horste), FLIGHT PLAN (Wegpunkte mit
    Strecke und geplantem Verbrauch je Abschnitt und Rest, M-009,
    Gesamtstrecke, Treibstoff, Tageszeit; Route hervorgehoben),
    LEAVE BRIEFING (weiter zur Schlussauswahl; die Ansicht bleibt beim
    letzten Thema, das dann gedämpft invers markiert ist). Die Karte zeichnen
    Linien in Fensterauflösung, die Beschriftung setzt die UI. Maus über der
    Karte (`MapView`): das Rad zoomt zum Zeiger hin (je Raste ×0,85), mit
    gedrückter linker Taste geht die Karte mit dem Zeiger mit; vom ganzen
    Gebiet bis 25 km Kantenlänge (darunter würde das Höhenbild mit rund 120 m
    je Bildpunkt nur gröber), nie über den Gebietsrand. Jede neue Mission
    beginnt wieder mit der Route, beim Wechsel der Themen bleibt der
    Ausschnitt.
  - **Briefing-Übersicht** (`Sim::briefing`): je Auftrag Art, Ziel und
    Entfernung, Routenlänge, Treibstoff nötig und mitgeführt, CAMERA
    REQUIRED, Regelset mit Einschränkungen, Tageszeit. Bedarf (`fuel_required_kg`):
    Routenlänge × 11 kg/km (gemessen mit dem Autopiloten bei 250 m/s in 3000 m)
    plus 450 kg Reserve; reicht der Haupttank (5200 kg) nicht, kommt ein
    Zusatztank in den Vorschlag (M-032).
  - **Bewaffnung** (ARMING; `f29_hud::arming`, `store_art.rs`): im Textfeld
    links FUEL REQUIRED / CARRIED (Warnfarbe bei Mangel; mitgeführt = Haupttank
    plus volle Zusatztanks) und die vier Schächte mit Kurzname und Stückzahl,
    rechts die ganze Unterseite der F-29 nach dem Grundriss des 3D-Modells,
    Nase links, mit vier flachen Schächten 2 × 2 im breiten hinteren Teil (vorn
    1 oben und 2 unten, hinten 3 und 4), darin die Grafik der Ausrüstung. Die
    Liste (zehn Ausrüstungen, EMPTY, ARMING COMPLETE) steht in der Auswahl in
    drei Spalten zu vier Zeilen. Bestätigen nimmt die Waffe (sie bleibt
    gedämpft invers), die Pfeile wählen einen Schacht (in der Tabelle invers,
    im Bild doppelt gerahmt), Bestätigen legt sie ab. Grafiken:
    `assets/stores/<code>.svg` (Code wie bei `--loadout`, etwa 2:1, Nase links,
    Text als Pfade), mit resvg gerastert; die mitgelieferten sind schlichte
    Platzhalter.
  - **Zusatztank** (`tank`): 1000 kg Treibstoff, voll 1120 kg, einer je
    Schacht. Gewählt pumpt der Auslöser im Flug in jedem HUD-Modus ohne Klappen
    in die Haupttanks („FUEL TRANSFERRED“); was nicht passt, geht verloren, der
    Schacht ist danach leer. WPN-Seite FUEL, READY.
  - **Bedienung:** Pfeile, W/S oder Blick vorn/hinten (I/K, Joystick) wählen,
    auf der Bewaffnung auch links/rechts; Leertaste, Enter bzw. Auslöser
    bestätigen. Maus: Die Wahl folgt dem Zeiger, sobald er sich bewegt
    (ruhend lässt er die Tasten gewähren), ein Linksklick bestätigt; auf dem
    Titelbild genügt ein Klick irgendwohin, in der Nachbesprechung auf
    CONTINUE. Bewaffnung: Klick auf eine Waffe nimmt sie, dann wählt der
    Zeiger den Schacht (Tabelle oder Bild), Klick legt sie ab; ein Klick auf
    eine andere Waffe tauscht sie. Lagebesprechung: Themen wechseln mit der
    Maus nur per Klick, damit die Ansicht auf dem Weg zur Karte nicht
    wechselt; LEAVE BRIEFING folgt dem Zeiger wie überall. Escape und Alt+Q
    beenden das Spiel (außer bei der Namenseingabe). Getroffen wird
    aus denselben Maßen, mit denen die Bildschirme aufgebaut sind
    (`ui::hit`).
  - **Darstellung** (`ui.rs`, Inhalt aus `f29-hud` als `Page`): Alle
    Bildschirme außerhalb des Cockpits, auch die Nachbesprechung, zeichnet
    bevy_ui in Departure Mono in ein Bild mit 640 × 400 Pixeln, ohne Glättung
    ganzzahlig vergrößert (Schriftpixel scharf, gleich groß). Überall Titel
    invers, Textfeld fester Größe und so breit wie der Titel, darunter die
    Auswahl (höchstens vier Zeilen, gewählte invers) an derselben Stelle, keine
    Bedienhinweise, kein Blinken. Auf der Bewaffnung liegen Umriss, Schächte
    und Grafiken in Fensterauflösung darüber. Das Titelbild ist eine
    Rastergrafik `assets/title.png` (16:10, z. B. 320 × 200 oder 640 × 400,
    Pixelart), bildfüllend ohne Glättung; die mitgelieferte ist ein Mockup.
- **Abweichungen:** Die Lagebesprechung fasst die Menüpunkte zu vier Themen
  zusammen (die Auswahl hat höchstens vier Zeilen), Raketenreichweiten stehen
  im Text statt als eigene Kreise; Richtung und Entfernung zusätzlich zu den
  Planquadraten (M-031), keine Fristen und
  besonderen Ereignisse; Mausbedienung der Menüs und der Karte (zoom- und
  verschiebbar) ist unsere Ergänzung; die Erkennung ist ein Platzhalter;
  „leicht“ lockert die Landegrenzen weniger als das Original, weil unsere
  schon großzügig sind (mehr wäre ein Aufprall). Dienstplan: Rückfrage vor
  dem Überschreiben, Entf bzw. rechte Maustaste statt der zweiten
  Auswahltaste, Escape bricht die Namenseingabe ab statt sie zu leeren, feste
  statt blinkender Schreibmarke; gespeichert wird gleich nach jeder Änderung
  statt beim nächsten Öffnen – Alt+Q wirkt trotzdem wie im Original, weil der
  Einsatz erst beim Übergang zum Dienstplan eingetragen wird.
- **Status:** teilweise (es fehlt das Erkennungsrätsel).
