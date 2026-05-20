# D2V 2019 — Daten-Port (`examples/d2v/`)

Dieses Verzeichnis enthält die DBlicious-Metadaten (Navigation, Spalten, Security,
Translatables) für den D2V-Daten-Port — **Track A** der D2V→DBlicious-Migration.

## Voraussetzungen

1. **`d2v.db` liegt NICHT in diesem Repo.** Lege eine Kopie der Produktions-DB an
   einem Pfad deiner Wahl ab. **Niemals auf die Original-Produktions-Datei zeigen.**
2. Setze die Umgebungsvariable:

   ```sh
   export D2V_LEGACY_URL="sqlite:///absoluter/pfad/zu/d2v-kopie.db"
   ```

   Alternativ `.env`-Datei im Repo-Root (ist in `.gitignore` eingetragen).

3. Die DBlicious-eigene DB (Auth, Audit, Translatables, Builder-Designs) kann
   ebenfalls konfiguriert werden:

   ```sh
   export DBLICIOUS_DATABASE_URL="sqlite://./dblicious-d2v.db"
   ```

   Default ist `sqlite://./dblicious-d2v.db` (Datei im Repo-Root).

## Starten

```sh
cargo run -p server -- --data-dir ./examples/d2v
```

## Datenschutz

Die `d2v.db` enthält echte Buchungsdaten. Nur mit einer Kopie arbeiten;
Original niemals auf einem Entwicklungsrechner einchecken oder weitergeben.

## Stand

Track A ist **noch nicht lauffähig** — Track B (DBlicious DB-Layer: foreign-sqlite-Source,
Composite-PK-Support) muss zuerst implementiert werden. Die Metadaten hier sind
bereits vollständig und dienen als Eingabe für Track-B-Design und -Tests.
