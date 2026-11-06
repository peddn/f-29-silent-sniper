# Gebietsformat

Ein Einsatzgebiet ist eine Textdatei im RON-Format. Sie legt alles fest, was
das Gebiet ausmacht: Geländeart und Seite je Rasterzelle, Flugplätze und
alle Objekte. Das Spiel erzeugt nichts davon selbst; Gebiete entstehen von
Hand oder im Karteneditor. Diese Beschreibung ist der Vertrag zwischen
Editor und Spiel.

Code: `crates/f29-world/src/theater.rs` (`Theater::parse`, `Theater::to_text`).

## Dateien und Auswahl

- Jedes Gebiet ist eine Datei `*.ron` in `data/theaters/`. Das Spiel liest
  beim Start alle Dateien dieses Ordners; `--theaters ORDNER` nimmt einen
  anderen. Unlesbare Dateien meldet es mit Pfad und Fehler auf der Konsole
  und lässt sie aus.
- Im Menü (REGION) stehen die Gebiete nach Name sortiert, höchstens vier.
  `--region NAME` startet direkt in einem Gebiet (Groß-/Kleinschreibung egal).
- Findet das Spiel keine Datei, nimmt es das eingebaute Testgebiet
  NORTH COAST.
- Replays speichern Name und Prüfsumme des Gebiets. Ein Replay lässt sich
  nur mit genau demselben Gebiet abspielen – wer ein Gebiet ändert, macht
  seine alten Replays unbrauchbar.

## Koordinaten

- Meter, `north` (Nord) und `east` (Ost), Ursprung in der Gebietsmitte.
- Das Gebiet ist ein Quadrat mit der Kantenlänge `size_m`, reicht also von
  −`size_m`/2 bis +`size_m`/2 in beiden Richtungen.
- Richtungen in Grad, rechtweisend: 0 = Nord, 90 = Ost.

## Aufbau

```ron
(
    format: 2,
    name: "LIBYA",
    seed: 1986,
    size_m: 500000.0,
    grid: (
        ground: [ "~~~~", "~Cdd", ".vdh", "ffhM" ],
        side:   [ "NNNN", "NFNH", "FNHH", "FNHH" ],
    ),
    airfields: [
        (name: "HOME", kind: Home, north: -120000.0, east: -150000.0, heading_deg: 90.0),
        (name: "BENINA", kind: Enemy, north: 80000.0, east: 60000.0, heading_deg: 157.5, mig21: 4, mig29: 2),
    ],
    objects: [
        (name: "RADAR", class: EarlyWarning, north: 95000.0, east: 40000.0, heading_deg: 0.0, place: "BENGHAZI"),
        (name: "MOSQUE", class: Building, north: 81000.0, east: 52000.0, heading_deg: 30.0, civilian: true),
    ],
)
```

| Feld | Bedeutung |
|---|---|
| `format` | Version dieses Formats, derzeit `2`. Das Spiel lehnt höhere Versionen ab; ältere liest es weiter. |
| `name` | Name des Gebiets, eindeutig; erscheint im Menü und im Replay. |
| `seed` | Startwert des Feinrauschens der Landschaft (Hügelformen, Grate). Ein anderer Wert ergibt bei gleichem Raster eine andere Feinform. |
| `size_m` | Kantenlänge in Metern. |
| `grid` | Raster aus `ground` und `side` (siehe unten). |
| `airfields` | Flugplätze; **der erste muss die eigene Basis sein**. |
| `objects` | Alle Objekte (Ziele und Ziviles). |

## Raster

`ground` und `side` sind je eine Liste von `n` Zeichenketten mit `n`
Zeichen (`n` ≥ 2, für beide gleich). Zeile 0 ist der Nordrand, Spalte 0 der
Westrand. Die Mitte der Zelle (Zeile `r`, Spalte `c`) liegt bei

- north = `size_m`/2 − (`r` + 0,5) · `size_m`/`n`
- east = −`size_m`/2 + (`c` + 0,5) · `size_m`/`n`

**Gelände** (`ground`):

| Zeichen | Art |
|---|---|
| Zeichen | Art | Landschaft | Farbe | Radar (Tiefflug) |
|---|---|---|---|---|
| `~` | Wasser | Meer, Seen | blau | 100 % |
| `.` | Flachland | eben, Wiesen mit Waldflecken | grün | 100 % |
| `h` | Hügel | hügelig | grün | 70 % |
| `M` | Gebirge | Grate, oben Fels und Schnee | grau/weiß | 45 % |
| `d` | Wüste | eben mit flachen Dünen | Sand | 100 % |
| `f` | Wald | eben bis wellig, ganz Wald | dunkelgrün | 85 % |
| `C` | Stadt | sehr eben, bebaut | grau | 80 % |
| `v` | Dorf | eben, verstreute Siedlungen zwischen Wiesen | oliv/grün | 90 % |

Aus den Geländearten formt das Spiel die Landschaft: Grundhöhe und Anteile
von Hügeln und Graten je Art, weich zwischen den Zellmitten übergeblendet,
die Grenzen leicht verzerrt (etwa 0,4 Zellen), darüber das Feinrauschen.
Die Farbe folgt der Art der verzerrten Zelle; steile Hänge sind immer Fels,
Ufer Strand. Die Radarspalte nennt, wie weit feindliche Radare im Tiefflug
noch reichen (Handbuch, Kapitel Radar). Häuser als Kulisse in Stadt und
Dorf gibt es noch nicht.

**Seite** (`side`):

| Zeichen | Seite |
|---|---|
| `F` | eigen (friendly) |
| `N` | neutral |
| `H` | feindlich (hostile) |

Die Seite unter dem Flugzeug entscheidet über Landung und Ausstieg (gerettet,
Zwischenfall, Gefangennahme), im Kalten Krieg auch darüber, ob der Gegner
feuern darf. Außerhalb des Gebiets gilt neutral.

## Flugplätze

| Feld | Bedeutung |
|---|---|
| `name` | Name (echt), z. B. „BENINA“. |
| `kind` | `Home` (eigene Basis) oder `Enemy` (feindlicher Horst). |
| `north`, `east` | Mitte der Bahn. |
| `heading_deg` | Bahnrichtung; das Spiel rundet auf 22,5°-Schritte. |
| `mig21`, `mig29` | nur feindliche Horste: Zahl der Jäger je Typ (fehlt = 0). |

Die Bahn liegt auf der Geländehöhe an ihrer Mitte (mindestens 12 m, also
nie im Wasser); das Gelände ringsum wird für sie eingeebnet.

## Objekte

| Feld | Bedeutung |
|---|---|
| `name` | Art des Objekts, z. B. „RADAR“, „SUPPLY DUMP“. |
| `class` | Zielklasse (Tabelle unten): bestimmt Form, Härte und Verhalten. |
| `north`, `east` | Lage; die Höhe ergibt sich aus dem Gelände. |
| `heading_deg` | Ausrichtung. |
| `civilian` | zivil (fehlt = nein): nicht angreifen, kostet sonst Punkte. |
| `place` | Ort (echter Name, fehlt = keiner). Angezeigt wird „NAME AT PLACE“, z. B. „RADAR AT BENGHAZI“ – in Briefing, Meldungen und auf der MSN-Seite. |

| Klasse | Art |
|---|---|
| `Soft` | ungeschützt: Lastwagen, Lager, Depots |
| `Armored` | Panzer, Schützenpanzer |
| `Building` | Hallen, Kommandostände, Kraftwerke |
| `Hardened` | Bunker, Flugzeugschutzbauten |
| `Bridge` | Brücken und Dämme |
| `ParkedAircraft` | abgestellte Flugzeuge |
| `ShipSmall` | kleine Schiffe (Patrouillenboote) |
| `ShipLarge` | große Schiffe (Fregatten, Frachter) |
| `AirDefense` | SAM-Stellung mit Radar (Doppler, 30 km) |
| `EarlyWarning` | Frühwarnradar (Impuls, 60 km) |
| `ShortRangeSam` | Kurzstrecken-Flugabwehr (Doppler, 12 km) |

## Was das Spiel prüft

Eine Datei, die eine dieser Regeln verletzt, ist unlesbar:

- `format` nicht höher als die Version des Spiels;
- Raster `n` × `n` in beiden Ebenen, nur die Zeichen oben;
- der erste Flugplatz ist die eigene Basis.

## Was ein gutes Gebiet beachtet

Das Spiel prüft das nicht, der Editor sollte es aber anzeigen:

- **Missionen:** Der Missionsgenerator wählt zwei Ziele aus den nichtzivilen
  Objekten der Klassen `Building`, `Hardened`, `Bridge`, `ParkedAircraft`,
  `Armored` und `Soft`, 80–220 km von der eigenen Basis und 20–80 km
  voneinander entfernt. Ohne solche Paare gibt es keine Mission.
- **Radare:** feindliche Radare mindestens 35 km von der eigenen Basis,
  damit schon das Anrollen unentdeckt bleibt.
- **Wasser und Land:** Schiffe nur auf Wasser, alles andere nur an Land.
- **Abstände:** Objekte mindestens etwa 120 m voneinander und 2,5 km von
  Bahnen.
- **Menge:** Jede Radarstellung zählt für die Entdeckung und jede SAM für
  den Beschuss. Weniger, aber gut platzierte Stellungen spielen sich wie im
  Original.

## Versionen

| Version | Änderung |
|---|---|
| 1 | erste beschriebene Fassung: Formatversion, Ort je Objekt, eigene Basis zuerst |
| 2 | Geländearten Wüste `d`, Wald `f`, Stadt `C`, Dorf `v` |
