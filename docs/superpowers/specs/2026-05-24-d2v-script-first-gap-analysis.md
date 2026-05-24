# D2V2019 → DBlicious: Script-First-Lücken-Analyse

Date: 2026-05-24
Status: Draft — awaiting user review
Typ: **Lücken-Analyse, kein Bauauftrag.**

## 0. Frage

> Was fehlt, um die **fertigen** (funktionierenden) D2V2019-Features in DBlicious abzubilden? Welche Controls fehlen? Wie weit lassen sich die D2V-Eigenheiten mit der neuen Script-Sprache (Q0009) **direkt im `examples/d2v/`** abbilden — und wo verläuft die Grenze zu Rust/Server?

Diese Analyse aktualisiert die ältere Gesamt-Gap-Analyse (`2026-05-20-d2v-on-dblicious-gap-analysis.md`) gegen den heutigen Stand und dreht die Perspektive: nicht „welcher Rust-Domain-Code fehlt", sondern „**wie viel geht datengetrieben im example dir, ohne server-Crate-Code zu schreiben**".

Bewusst ausgeklammert (laut Glossary [[d2v-domain-glossary]] „nie funktional / Stub / obsolet"): `Company`, `DatevEntryGroup`, `DatevEntryChangeTracking`, StarMoney↔DATEV-Reconciliation, `StarMoneyCreditCard`, `DescriptionSplitter`, StarMoney-Import (CSV-Banken stattdessen).

---

## 1. Fundament-Update (was sich seit 2026-05-20 geändert hat)

Die ältere Gap-Analyse listete fast alles als 📋/🚧. Heutiger **verifizierter** Code-Stand (gegen `server/src/`, `shared/src/`, nicht gegen ROADMAP-Marker — die sind teils veraltet):

| Baustein | Stand | Pfad |
|---|---|---|
| Source-Architektur (Track B, 0.6 B0–B5) | ✅ komplett | `server/src/source/` |
| `examples/d2v/` Datenport (17 Entities) | ✅ als foreign-sqlite | `examples/d2v/entities/*/binding.json` |
| Number-Sequences (1.7.1) | ✅ | `server/src/sequences/` |
| FX-Rate-Store (1.7.2) | ✅ | `server/src/fx/` |
| Period-Locks (1.7.3) | ✅ | `server/src/period_locks/` |
| GoBD-Append-Only (1.7.4) | ✅ | `shared/src/settings.rs` + `server/src/schema.rs` |
| State-Machine (1.7.5) | ✅ | `server/src/state_machine/`, `shared/src/state_machine.rs` |
| Job-Scheduler + Cron (1.7.7) | ✅ | `server/src/jobs/` |
| File-Storage Local-FS + S3 (1.7.8) | ✅ (S3-Roundtrip nur ignore-Smoke) | `server/src/storage/` |
| PDF-Backend Typst (1.7.9) | ✅ Renderer (Plan; pdf_templates-Tabelle offen) | `server/src/pdf/` |
| CSV+XLSX-Export Service (1.7.14) | ✅ (GraphQL-Resolver-Anbindung offen) | `server/src/export/` |
| IntEnum-FieldType (G7) | ✅ | `shared/src/lib.rs` |
| Script-Sprache (Q0009, alle Blocker remediated) | ✅ | `server/src/script/`, `client/src/script/` |

**Konsequenz:** Der ERP-Unterbau, den die alte Analyse als großen Backlog sah, steht zu großen Teilen. Die verbleibenden Lücken sind enger und konkreter (Abschnitt 5).

**Noch nicht gebaut (D2V-relevant):**

| Fehlt | D2V-Bedarf |
|---|---|
| Aggregation-Queries (1.7.12) | SuSa-Summen, Calculation-Baum-Werte, Konto-Saldo |
| Bulk-Import (1.7.15) | DATEV-Import, Bank-CSV |
| `FieldType::Tree` (1.7.20) | Bilanz/GuV-Baum (DatevCalculation) |
| `FieldType::Iban` | IBAN-Validierung StarMoney-Konten |
| `FieldType::Bitflags` (G3) | `DatevAccountType` |
| FK-Referenz-Picker (U1) | jede Buchungs-Form (Konto-/Bank-Auswahl) |
| Report-View-Control (U3) | DatevAccountView, SuSaView, Calculation-View |
| Save-Report/Preview (U2) | GenerateAccountEntries-Vorschau |
| Operations-Resource (F1) / Long-Running-Action (U6) | Invert, GenerateAccountEntries-Persistierung |
| App-Context/YearSelector (U5) | globaler Jahres-Filter |

---

## 2. Scope: fertige D2V-Features

Aus `examples/d2v/entities/` (17 Entities verfügbar) plus Glossary-Klassifikation. **Fertig** = funktional im Alt-System:

1. **DatevAccount** + Kontenrahmen (`DatevAccountType`-Bitflags) — *extrem wichtig*
2. **DatevEntry** (Buchung) + `ValueType` SOLL/HABEN
3. **DatevEntryStack** (Import-/Edit-Marker, aktiv genutzt)
4. **DatevCalculation**-Baum (Bilanz/GuV, rekursiv)
5. **StarMoney-Stammdaten** (Bank, Account, BookingText) + IBAN
6. **StarMoneyEntry** (40+ Spalten flach; Importer veraltet, Daten fertig)
7. **SuSa** (Summen-/Saldenliste)
8. **GenerateAccountEntries** → **DatevAccountEntry** (computed Sub-Buchungen) — *extrem wichtig, T5/T6*
9. **Jahresabschluss** (PDF, Layout 1:1)
10. **Voucher** (Excel-Export **pro Buchung**)
11. **YearSelector** (globaler Jahres-Kontext, aktiv)
12. **IBAN-Helper** (Modulo-97 + BIC-Lookup)
13. **Invert / Generalumkehr** (Storno-Gegenbuchung)

---

## 3. Kontur der Script-Sprache heute (Q0009)

**Was ein Script kann** (verifiziert gegen `shared/src/script/capability.rs` + `model.rs`):

- **Provider-Slots** (`ProviderSlot`): `Formatter`, `Filter`, `Computed`, `Validator`, `RowAction`. Ein `script:<id>` kann pro Property/Spalte/Zeile als Implementierung eingesetzt werden (Resolution-Kette aus Phase 1.5).
- **Capabilities**: `ReadOwnEntities`, `ReadAllEntitiesWhereAllowed` (→ `db.entities`/`db.entity`-Reads durch die Sandbox-Gate), `ComputeOnly` (Arithmetik/String/Array/Map — die fünf Rhai-Packages), `EmitUiNode` (UI-Subtrees), `ReadI18n` (`ctx.t`).
- **Sandbox-garantiert** (nach Q0009-Remediation): Capability-Gate feuert im echten eval-Pfad; unmaskable Errors (Denied/Timeout/Memory) sind per try/catch nicht umgehbar; Memory/Timeout-Limits greifen.

**Die harte Grenze — Scripts können NICHT schreiben:**

- `db.patch` (WriteEntity) ist `ServerOnlyFunction` — sowohl Client als auch Preview-Host lehnen ab. `audit.log` ebenso.
- Ein Script ist also reines **Read + Compute + Format + Validate + UI-Emit**. Jede Mutation (neue Buchung, persistierte Sub-Buchung, Storno, Import) liegt außerhalb.

**Konsequenz für „direkt im `examples/d2v/`":** Alles, was eine D2V-Eigenheit **anzeigt, formatiert, validiert oder read-time berechnet**, kann als Script-Datei im example dir liegen. Der Lade-Pfad **steht bereits** (verifiziert): `examples/d2v/scripts/<id>.rhai` (Source) + `examples/d2v/scripts/<id>.manifest.{json,toml}` (kind + manifest) → beim Boot in die `scripts`-Tabelle geseedet → `formatter_id`/`filter_id`/… = `"script:<id>"` in `columns.json`/`settings.json` löst über `provider_lookup` auf. Alles **Schreibende** kann es nicht.

---

## 4. Feature-für-Feature-Matrix

Legende Script-Slot: F=Formatter, Fi=Filter, C=Computed, V=Validator, R=RowAction, UI=EmitUiNode. „read-time" = Script bei jeder Anzeige; „persistiert" = gespeicherter Wert (= Server-Write).

| # | D2V-Feature | Script-Slot | read-time-Script | persistiert/Server | fehlende Control | fehlende Fähigkeit |
|---|---|---|---|---|---|---|
| 1 | **DatevAccount**-CRUD + Kontenrahmen | F (Nummer/Typ-Format), V (Pflichtfelder) | Format/Validate ✅ | — (reines Stammdaten-CRUD) | — | `FieldType::Bitflags` für `account_type` (sonst IntEnum-Workaround) |
| 2 | **DatevEntry** + ValueType | F (SOLL/HABEN-Darstellung), V (Soll==Haben-Balance) | ✅ Formatter + Validator script-first | — (CRUD via generischer Editor) | **U1 FK-Picker** (Konto-Auswahl) | FK-Picker; sonst nur ID-Text |
| 3 | **DatevEntryStack** Filter-Marker | Fi (nach Stack filtern) | ✅ als Filter-Script | — | — (ggf. **U5** für globalen Kontext) | — |
| 4 | **DatevCalculation**-Baum | C (Knoten-Werte read-time), UI (Baum rendern) | **teilweise**: Script kann Werte je Knoten read-time aus `db.entities` summieren; aber Baum-Layout + Rekursion braucht **U3 Report-View** | persistiert nur nötig, wenn Snapshot/Performance verlangt | **U3 Report-View**, **`FieldType::Tree`** | **Aggregation (1.7.12)** für Summen ohne N+1-Reads |
| 5 | **StarMoney-Stammdaten** + IBAN | V (IBAN-Modulo-97 via ComputeOnly) | ✅ IBAN-Validator als Script machbar (reine Arithmetik) | — | — | optional `FieldType::Iban` (Komfort; Script-Validator reicht) |
| 6 | **StarMoneyEntry**-CRUD | F (Description-Format), Fi | ✅ Anzeige/Filter | — (Import = T13, separat) | **U1** (Bank-/BookingText-Picker) | Bank-CSV-Import = **Bulk-Import (1.7.15)**, server |
| 7 | **SuSa** (Summen/Salden) | C (read-time), UI | **read-time möglich** für kleine Datenmengen (Script liest Entries, summiert); **Server besser** bei großen Mengen | persistiert/Server via **Aggregation (1.7.12)** empfohlen | **U3 Report-View** | **Aggregation (1.7.12)** — sonst Script-N+1 |
| 8 | **GenerateAccountEntries** → DatevAccountEntry | C (Vorschau read-time), R (Trigger) | **Vorschau ja** (Computed-Script berechnet Sub-Buchungen read-time); **Persistieren NEIN** | **persistiert nötig** (DATEV-Abgleich verlangt gespeicherte, exportierbare Sub-Buchungen) → **Server-Operation** | **U2 Save-Report** (Diff-Vorschau), **U6/F1** (Persistier-Operation) | **WriteEntity-Server-Pfad** oder **F1 Operations-Resource** |
| 9 | **Jahresabschluss** PDF | — (Daten via C, Render via PDF-Backend) | Datenaufbereitung read-time möglich | PDF-Erzeugung = Server (1.7.9 ✅) | — (pdf_templates-Tabelle offen) | `pdf_templates`-Persistenz + Designer (1.7.9-Folge) |
| 10 | **Voucher** (Excel pro Buchung) | R (Export-Trigger) | Daten read-time | Excel-Erzeugung = Server (1.7.14-Service ✅) | **F2 Single-Entity-Excel-Template** | XLSX-Resolver-Anbindung (1.7.14-Folge) |
| 11 | **YearSelector** globaler Filter | Fi (pro Liste) | Filter-Script pro Liste ✅, aber **kein globaler Kontext** | — | **U5 App-Context** | globaler reaktiver Kontext-Channel |
| 12 | **IBAN-Helper** | V | ✅ vollständig script-first (Modulo-97 = ComputeOnly) | — | — | (optional `FieldType::Iban` als Komfort) |
| 13 | **Invert/Generalumkehr** | R (Trigger), C (Vorschau) | **Vorschau ja**, **Buchung schreiben NEIN** | **persistiert nötig** → Server | **U6 Long-Running-Action** / **F1** | **WriteEntity-Server-Pfad** |

---

## 5. Lücken-Cluster (konsolidiert)

### 5.1 Fehlende Controls (Client-Komponenten)

| Control | Für welche D2V-Features | Größe | Status heute |
|---|---|---|---|
| **U1 FK-Referenz-Picker** (Suche + display_template) | DatevEntry (Konto), StarMoneyEntry (Bank/BookingText) | M | `Reference` zeigt nur ID; `picker.rs` ist Implementations-Picker (Phase 1.5), **kein** FK-Picker |
| **U3 Report-View** (Cross-Table-Aggregation, Tree/Pivot) | DatevCalculation, SuSa, DatevAccountView | L | fehlt; `EntityTable` ist single-table |
| **U2 Save-Report** (Diff-Vorschau vor Commit) | GenerateAccountEntries | M-L | fehlt |
| **U5 App-Context** (globaler YearSelector) | alle Buchungs-Listen | S-M | fehlt; Filter ist pro Tabelle |
| **U6 Long-Running-Action** (Trigger + Progress + Report) | GenerateAccountEntries, Invert | M | `row_actions.rs` kann CRUD-Verben, keine Custom-Action mit Report |

### 5.2 Fehlende Script-/Framework-Fähigkeiten

| Fähigkeit | Warum D2V-relevant | Anmerkung |
|---|---|---|
| **Aggregation-Queries (1.7.12)** | SuSa, Calculation-Werte, Saldo ohne N+1-Script-Reads | größte Hebelwirkung für die Auswertungs-Features; Computed-Scripts könnten read-time aufrufen statt selbst zu iterieren |
| **`FieldType::Tree` (1.7.20)** | DatevCalculation-Baum | rekursive Parent-Child-Darstellung |
| **`FieldType::Bitflags` (G3)** | `DatevAccountType` | IntEnum (G7 ✅) ist ein Teil-Workaround, deckt aber keine Flag-Kombinationen |
| **`FieldType::Iban`** | StarMoney-Konten | optional — Script-Validator (ComputeOnly) deckt die Prüfung bereits ab |
| ~~example-dir Script-Layout~~ | — | **KEINE Lücke (verifiziert):** der Loader lädt Provider-Scripts bereits aus dem data-dir — `examples/<set>/scripts/<id>.rhai` (Source) + `<id>.manifest.{json,toml}` (kind + manifest), via `server/src/example/loader.rs::load_scripts`. Beim Boot geseedet; `script:<id>` in columns/settings löst über `provider_lookup` auf (heute remediated). Der script-first-Pfad steht. |

### 5.3 Harte Server-Grenzen (nicht script-abbildbar)

| Operation | Warum nicht Script | Lösung |
|---|---|---|
| GenerateAccountEntries **persistieren** | Write + DATEV-Abgleich | F1 Operations-Resource oder WriteEntity-Server-Pfad |
| Invert/Storno **schreiben** | Write | dito |
| DATEV-/Bank-CSV-**Import** | Write + Transaktion + Rollback | Bulk-Import (1.7.15) |
| Jahresabschluss-**PDF**, Voucher-**XLSX** | Binär-Erzeugung | Server-Backends (1.7.9/1.7.14 ✅) — Script liefert höchstens die Daten read-time |

---

## 6. Kernantwort: Was geht script-first im `examples/d2v/`?

**Geht vollständig script-first (Datei im example dir, kein Rust):**

- **Formatter**: ValueType SOLL/HABEN, Konto-Nummern-Darstellung, Decimal/Money-Format, Description-Aufbereitung.
- **Validator**: Soll==Haben-Balance, IBAN-Modulo-97, Pflichtfeld-/Plausibilitätsregeln. (IBAN braucht **kein** eigenes FieldType — ein ComputeOnly-Script reicht.)
- **Filter**: Stack-Filter, einfache Jahr-/Status-Filter pro Liste.
- **read-time Computed (kleine Mengen)**: Saldo/Summe je Zeile, GenerateAccountEntries-**Vorschau**, einfache Calculation-Knoten-Werte.

**Geht script-first nur mit einer fehlenden Control:**

- Calculation-Baum, SuSaView, DatevAccountView → brauchen **U3 Report-View** (das Script liefert die Werte, die Control das Layout).
- Konto-/Bank-Auswahl in Formularen → braucht **U1 FK-Picker**.
- Globaler Jahres-Filter → braucht **U5 App-Context**.

**Geht NICHT script-first (zwingend Server/Rust):**

- Persistieren von computed Sub-Buchungen (GenerateAccountEntries-Commit), Invert-Buchung, Import. → Write-Grenze.
- Effiziente Aggregation über große Buchungsmengen (SuSa/Calculation produktiv) → **1.7.12**, nicht read-time-Script.
- PDF/XLSX-Binär-Erzeugung → Server-Backends (bereits ✅).

**Trade-off read-time vs. persistiert (pro abgeleitetem Wert):**

- *read-time-Script*: maximal datengetrieben, sofort im example dir, kein Server-Code — aber kein persistenter DATEV-Abgleich und N+1-Risiko ohne Aggregation. Gut für Vorschau, kleine Mengen, Validierung.
- *persistiert/Server*: nötig, sobald der Wert exportiert/abgeglichen werden muss (DATEV) oder die Datenmenge Aggregation verlangt. Script kann dann nur die **Vorschau/Berechnungslogik** beisteuern, nicht den Commit.

---

## 7. Priorisierte Folge-Lücken (geordnet, kein Bauauftrag)

Sortiert nach Hebelwirkung für „möglichst viel D2V script-first". (Der example-dir Script-Provider-Loader ist **bereits vorhanden** — kein Eintrag mehr.)

1. **U1 FK-Referenz-Picker** — entsperrt jede Buchungs-Form. M.
2. **Aggregation-Queries (1.7.12)** — entsperrt SuSa/Calculation/Saldo effizient; Computed-Scripts rufen sie statt selbst zu iterieren. M.
3. **U3 Report-View** — entsperrt die drei Auswertungs-Sichten. L.
4. **U5 App-Context (YearSelector)** — globaler Jahr-Filter. S-M.
5. **`FieldType::Tree` (1.7.20)** — Calculation-Baum. M.
6. **U2 Save-Report + U6/F1** — GenerateAccountEntries/Invert (die schreibenden Kern-Features). M-L.
7. **Bulk-Import (1.7.15)** + Bank-CSV-Mapping — Import-Pfad. M-L.

**Schnellster sichtbarer Erfolg:** Da der Loader-Pfad steht, lassen sich **Formatter-/Validator-/Filter-Scripts** (ValueType SOLL/HABEN, IBAN-Prüfung, Stack-Filter) **heute schon** ohne jede neue Lücke als `examples/d2v/scripts/`-Dateien anlegen — ein guter erster Pilot, falls aus dieser Analyse später doch Bau wird.

`FieldType::Bitflags` (G3) und `FieldType::Iban` sind **niedrig** — IntEnum bzw. Script-Validator decken den Bedarf provisorisch.

## 8. Decisions

1. Diese Analyse ist **keine Bau-Spec**; sie ordnet die Lücken gegen den heutigen (deutlich weiter fortgeschrittenen) Stand neu.
2. Die **Write-Grenze** der Script-Sprache ist die zentrale Trennlinie: Read/Compute/Format/Validate/UI → script-first im example dir; alles Schreibende → Server.
3. **read-time vs. persistiert** wird pro abgeleitetem Wert entschieden (Abschnitt 6): Vorschau/kleine Mengen read-time, DATEV-abgleichbare/große Mengen persistiert.
4. Der **Loader lädt Provider-Scripts aus dem data-dir bereits** (verifiziert) — der script-first-Pfad steht. Höchster Hebel danach: **U1 FK-Picker** + **Aggregation (1.7.12)**.
5. Die alte Gap-Analyse (`2026-05-20-…`) bleibt als Track-D/Pattern-Inventar gültig; diese hier ist die script-first-Brille darauf.

## 9. Referenzen

- `docs/superpowers/specs/2026-05-20-d2v-on-dblicious-gap-analysis.md` — ältere Gesamt-Analyse (U1–U7, F1/F2, T1–T23).
- `docs/superpowers/specs/2026-05-19-d2v-data-port-design.md` — Track A + T-Backlog.
- `docs/superpowers/specs/2026-05-20-d2v-target-schema-design.md` — Ziel-Schema.
- `docs/reviews/Q0009-review.md` — Script-Sprache (remediated).
- `shared/src/script/{capability,model}.rs` — Provider-Slots + Capabilities.
- `examples/d2v/entities/*/` — Datenport-Layout (binding/columns/editor/settings).
- Memory [[d2v-domain-glossary]] — fertig-vs-unfertig-Klassifikation.
- Memory [[d2v-on-dblicious-migration]] — Track-Struktur.
