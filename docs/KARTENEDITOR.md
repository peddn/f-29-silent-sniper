# Karteneditor

Mit dem Karteneditor entstehen die Einsatzgebiete (M-031). Er liest und
schreibt Gebietsdateien nur über `f29_world::Theater` (`Theater::parse`,
`Theater::to_text`), also genau im Format von
[`GEBIETSFORMAT.md`](GEBIETSFORMAT.md). Seine Landschaftsvorschau rechnet
derselbe Code wie das Spiel (`Terrain::from_theater`, Satellitenkarte aus
`f29-render`). Ein vorab gerechnetes Höhenbild gibt es nicht: Die Textdatei
ist die Karte.

Code: `crates/f29-editor` (egui/eframe, ohne Bevy).

```bash
cargo run --release -p f29-editor
cargo run --release -p f29-editor -- data/theaters/libyen.ron --unterlage karten/libyen.png
```

Ohne Angabe öffnet der Editor die zuletzt bearbeitete Datei mit ihrer
Unterlage; beim allerersten Start ein neues Gebiet `data/theaters/neu.ron`.
Fehlt eine angegebene Datei, legt er ein neues Gebiet mit diesem Namen an
(ganz Flachland, neutral, die eigene Basis HOME in der Mitte) und schreibt es
beim ersten Speichern. Das fertige Programm liegt nach dem Bauen unter
`target/release/f29-editor` und lässt sich auch direkt oder über eine
Verknüpfung starten.

**Was der Editor sich merkt:** die zuletzt bearbeitete Datei, zu jeder
Gebietsdatei ihre Unterlage (beim Öffnen wird sie wieder geladen, ohne
gemerkte Unterlage ist keine zu sehen), Fenstergröße und Panelbreiten.
Gespeichert wird beim Schließen des Fensters, unter Linux in
`~/.local/share/f29-editor/app.ron`, unter Windows in
`%APPDATA%\f29-editor\data\app.ron`, unter macOS in
`~/Library/Application Support/f29-editor/app.ron`. Wer die Datei löscht,
startet wie beim ersten Mal.

## Stand

| Teil | Stand |
|---|---|
| Neu, Öffnen, Speichern, Speichern unter | fertig |
| Zuletzt bearbeitete Datei und Unterlagen merken | fertig |
| Gelände und Seite malen (Pinsel, Füllen), Rückgängig | fertig |
| Kartenunterlage, Wasser aus Maske | fertig |
| Landschaftsvorschau | fertig |
| Flugplätze und Objekte setzen, verschieben, bearbeiten, Ort aus der Ortsliste | fertig |
| Reichweiten, Ringe um die Basis | fertig |
| Prüfungen (Missionspaare, Radarabstand, Schiffe auf Wasser) | folgt |

## Fenster

- **Oben:** Datei und Verlauf (Rückgängig, Wiederholen).
- **Links:** Name und Seed des Gebiets, Bearbeiten (Ebene Gelände, Seite
  oder Objekte), Ansicht, Unterlage.
- **Mitte:** die Karte, Norden oben, mit den 100-km-Planquadraten des Spiels
  (Buchstaben von West bzw. Süd, wie im Briefing).
- **Unten:** Lage unter der Maus (Zeile und Spalte, Nord und Ost in km,
  Planquadrat, Gelände, Seite, Objekt) und die letzte Meldung.

Flugplätze erscheinen als Quadrate mit ihrer Bahn als Strich (blau die eigene
Basis, violett feindliche Horste), Objekte als Punkte (rot militärisch, gelb
Radar und Flugabwehr, türkis Schiffe, weiß zivil).

## Malen

| Eingabe | Wirkung |
|---|---|
| linke Maustaste | malt mit dem gewählten Wert (Pinsel) bzw. füllt die zusammenhängende Fläche (Füllen) |
| rechte Maustaste | nimmt den Wert unter der Maus auf |
| Mausrad | zoomt um den Mauszeiger |
| mittlere Maustaste oder Leertaste + Ziehen | verschiebt |
| 1 … 8 | Geländeart `~ . h M d f C v` bzw. Seite `F N H` |
| E | Ebene wechseln (Gelände → Seite → Objekte) |
| B, F | Pinsel, Füllen |
| `[`, `]` | Pinsel kleiner, größer (1, 3, 5, 9, 15 Zellen) |
| L | Zellen ↔ Landschaft |
| U | Unterlage ein/aus |
| Strg+S | speichern |
| Strg+Z, Strg+Y (Strg+Umschalt+Z) | rückgängig, wiederholen |

Ein Pinselstrich ist ein Schritt im Verlauf, ebenso Füllen, „Wasser aus
Maske“ und ein neuer Seed; der Verlauf hält 200 Schritte. Gebiets- und
Bilddateien lassen sich ins Fenster ziehen: `.ron` öffnet, `.png` wird
Unterlage.

**Seed:** Er wird beim Anlegen gesetzt und danach unverändert gelesen und
geschrieben. Ändern lässt er sich nur, solange weder Objekte noch feindliche
Horste stehen, und mit Warnung: Ein neuer Seed verschiebt Grate und Hügel
überall im Gebiet und macht alte Replays ungültig.

## Objekte

In der Ebene **Objekte** (Taste E) setzt und bearbeitet man Flugplätze und
Objekte.

| Eingabe | Wirkung |
|---|---|
| A, N | Werkzeug Auswählen, Setzen |
| linke Maustaste (Setzen) | legt die gewählte Art an der Stelle an |
| linke Maustaste (Auswählen) | wählt, was unter der Maus liegt; Ziehen verschiebt |
| Entf | löscht die Auswahl |
| Strg+D | dupliziert die Auswahl (1 km östlich) |
| Esc | hebt die Auswahl auf |

- **Arten:** feindlicher Horst (AIRBASE) und die Zielklassen des Spiels, mit
  „zivil“ als zivile Variante. Neue Objekte heißen wie im Spiel (EW RADAR,
  SAM SITE, BRIDGE …); wechselt man Klasse oder „zivil“, wechselt ein solcher
  Standardname mit, ein eigener Name bleibt.
- **Ort:** Liegt neben der Unterlage ihre Ortsliste (`libyen_orte.txt` zu
  `libyen.png`), trägt der Editor beim Setzen den nächsten Ort bis 40 km ein;
  angezeigt wird dann „EW RADAR AT BANGHAZI“. Ein neuer Horst heißt nach dem
  nächsten Ort. Die Ortsliste nutzt die Schreibweisen von Natural Earth
  (BANGHAZI statt BENGHAZI); man korrigiert sie im Feld. „nächster“ trägt
  nach dem Verschieben den nun nächsten Ort ein.
- **Eigenschaften:** Name, Klasse, zivil, Ort, Richtung und Lage (Nord, Ost in
  km); bei Flugplätzen Bahnrichtung (22,5°-Schritte) und die Zahl der MiG-21
  und MiG-29. Neue Horste haben zwei MiG-21. Eine Änderung im Feld ist ein
  Schritt im Verlauf, sobald das Feld den Fokus abgibt.
- **Eigene Basis:** Es gibt sie genau einmal; sie bleibt der erste Flugplatz
  und lässt sich verschieben, aber weder löschen noch duplizieren.
- **Liste:** alle Flugplätze und Objekte; ein Klick wählt und zentriert.
- **Lage:** auf ganze Meter, nie außerhalb des Gebiets.

**Reichweiten und Ringe** (Ansicht): Kreise um Frühwarnradare (gelb), SAM und
SHORAD (rot) mit den Reichweiten des Spiels (`f29_core::radar`), um die
eigene Basis der Ring für feindliche Radare (35 km, orange) und das
Missionsband (80 und 220 km, blau).

## Ansicht

- **Zellen:** jede Rasterzelle in der Farbe ihrer Geländeart (Palette des
  Spiels). Ab etwa 12 Pixeln je Zelle erscheint das Zellgitter.
- **Landschaft:** das Satellitenbild des Spiels (wie Karte und
  `f29-headless theater-png`), knapp 0,5 km je Pixel. Es wird nach jeder
  Änderung im Hintergrund neu gerechnet; solange es nicht aktuell ist, dreht
  sich unten rechts ein Zeichen.
- **Seiten einfärben:** eigen blau, neutral gelb, feindlich rot; beim Malen
  der Seiten stärker.

## Unterlage und Wassermaske

Die **Unterlage** ist ein PNG, das genau das 500-km-Quadrat zeigt (Norden
oben, beliebige Größe). Der Editor legt es mit einstellbarer Deckkraft über
die Karte; Projektion und Zuschnitt sind Sache der Bilddatei.

**Wasser aus Maske** übernimmt Küsten und Seen aus einem Graustufen-PNG
desselben Zuschnitts: Zellen, die im Mittel dunkel sind, werden Wasser, helle
Wasserzellen Flachland; die übrigen Landarten bleiben. Danach malt man
Gebirge, Wüste, Wald, Städte und Seiten von Hand.

Die Unterlagen der vier Gebiete liegen nicht im Repository. Sie entstehen aus
Natural Earth (gemeinfrei), azimutal abstandstreu um den Mittelpunkt des
Gebiets (Kugel, R = 6371 km; Maßstabsfehler in den Ecken unter 0,1 %):

| Gebiet | Mittelpunkt | Gedacht als eigene Basis |
|---|---|---|
| Libyen | 31,4° N 20,6° O | erfunden, in der Wüste südlich von Benghazi |
| Persischer Golf | 26,3° N 55,9° O | Ras al-Khaimah |
| Nordkap | 69,5° N 31,3° O | Kirkenes |
| Mitteleuropa | 50,6° N 10,2° O | Rhein-Main |

Missionsziele liegen 80–220 km von der eigenen Basis (M-031); der Ausschnitt
ist deshalb um die Basis gewählt.

## Arbeitsablauf

1. Neues Gebiet anlegen (Name, Seed, Datei), Unterlage laden.
2. „Wasser aus Maske“, dann Gelände malen; in der Landschaftsansicht prüfen.
3. Seiten malen.
4. Eigene Basis an ihren Platz ziehen, feindliche Horste und Objekte setzen.
5. Speichern und probefliegen:
   `cargo run -p f29-app -- --theaters data/theaters --region NAME --start air`.
