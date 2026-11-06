# Arbeitsregeln für F-29 Silent Sniper

- Sprache mit dem Spieler: Deutsch.
- **Im Zweifel das Original (F-19 Stealth Fighter) als Vorlage nehmen.** Die
  Referenz liegt nur zum Lesen vor: erst lesen, dann in eigenen Worten in
  `docs/MECHANICS.md` beschreiben – keine Werte und keinen Code übernehmen.
- **Code so kurz und klar wie möglich halten.**
- **Vor jedem Vorhaben abwägen, wie stark es die Komplexität erhöht und ob es
  sich lohnt** – und das dem Spieler mitteilen, bevor gebaut wird.
- Neue Mechaniken zuerst diskutieren, wenn der Spieler darum bittet.
- Echte Waffennamen sind erlaubt.

## Git

- **Größere Unterpunkte** (ein Roadmap-Punkt, eine neue Mechanik, ein Umbau)
  auf einem eigenen Branch von `main` (`feature/<kurzname>`), pushen und als
  Pull Request gegen `main` öffnen. Der Spieler testet lokal und merged selbst;
  nicht selbst nach `main` mergen.
- Kleine Korrekturen (Tippfehler, Doku) dürfen direkt auf `main`.
- Zu jedem Branch die Befehle nennen, mit denen der Spieler hineinwechselt,
  testet und zurück nach `main` geht.
