# Etappe 2: Luftkampf – Einschätzung (noch nicht begonnen)

Vorhanden und wiederverwendbar: einheitlicher Raketenflug (Kegel, Vorhalt,
Drehrate, Resistenz, Blenden, Querflug schüttelt ab), Täuschkörper mit
Werfer (`owner`, locken nur Raketen der Gegenseite), Kanone, Zielwahl,
Erfassungskreis, Kartenfarben.

| Teil | Aufwand | Inhalt |
|---|---|---|
| Schaden an Jägern | klein, ~40 Z. | Schadenswert; Rakete zerstört, Kanone fester Anteil; Absturz, Meldung, Abschuss |
| Kanone gegen Luft | klein–mittel, ~60 Z. | Treffer gegen Jäger (Schrittstrecke prüfen); Vorhaltemarke als reine Anzeige im AA-Modus |
| AIM-9 | klein, ~50 Z. | IR-Suchkopf, Start im 10°-Kreis, Fire and Forget; Umbau: Raketenziel „Jäger“ (`Track`), Waffenschritt bekommt die Jäger |
| AIM-120 | mittel, ~40 Z. | über 10 km Lenkung über eigenes Radar (an, Ziel gehalten), sonst Ziel verloren; darunter eigener Suchkopf |
| Feindliche Täuschkörper | klein, ~30 Z. | nahe Rakete → Jäger wirft passende, kleiner Vorrat |
| HUD/Waffenwahl | klein, ~50 Z. | Vorhaltemarke, Feuersignal, AIM-9/AIM-120 in Schächten |

Gesamt ~250–300 Zeilen + ~150 Zeilen Tests; Komplexität mäßig. Risiko:
Abstimmung – Jäger ohne Ausweichen sind leichte Beute.

Offene Entscheidungen (Vorschläge):
- a) Kanonentreffer bis Abschuss: MiG-21 ~10, MiG-29 ~15.
- b) Ausweichen der Jäger: A) noch nicht, nur Täuschkörper (empfohlen);
  B) querab abdrehen bei Rakete (~20 Z. mehr).
- c) AIM-120: Radar an und Ziel halten bis 10 km, sonst verloren.
- d) Je Schacht 2 AIM-9 oder 1 AIM-120.
