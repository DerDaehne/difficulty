# difficulty

- Verbindliche Sprachbeschreibung: `docs/spec.md`. Begründungen, Beispiele und Fahrplan: `PLAN.md`.
- Compiler in Rust (`cargo`), Backend erzeugt C11 für den System-C-Compiler (gcc).
- Jede Sprachänderung aktualisiert Spec, `llms.txt` und Golden-Tests im selben Commit.
- Öffentliches Repo: keine internen Daten (Ticketnummern, interne Notizen, lokale Pfade, private Adressen) in Dateien oder Commits. Commits mit der noreply-Identität.
