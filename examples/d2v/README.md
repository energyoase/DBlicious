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

Track A-Metadaten sind komplett (17 Entitäten, 4 mit Composite-PK, 6 read-only).
Track B (foreign-sqlite-Source, Composite-PK, SQL-Pushdown, Read-only-Bindings,
per-entity-Binding-Loader) ist ebenfalls fertig — der Loader parst alle
17 Entity-Verzeichnisse sauber (siehe `server/tests/loader_d2v.rs`, 7 Test-Cases).

Was noch fehlt:

- Fixture-`.db` mit Mini-Auszug der echten Schemas (Plan-Task 25).
- Integrations-Test gegen die Fixture (Plan-Task 26).
- Manueller Smoke-Test gegen eine Kopie der Produktions-DB (Plan-Task 27).
- Post-Port-Cleanup (Spec §11): Enum-FieldType für `valueType`/`accountType`,
  feinere `precision` pro Decimal-Spalte, EN-Translations polieren.
