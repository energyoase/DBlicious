# DBlicious Schema-Sprache: Konzept-Lücken & Erweiterungs-Backlog

Date: 2026-05-20
Status: Backlog — gesammelt beim D2V-Ziel-Schema-Design

## 1. Problem

Beim Modellieren des Ziel-Schemas für den D2V-Neubau (siehe [`2026-05-20-d2v-target-schema-design.md`](./2026-05-20-d2v-target-schema-design.md)) sind sieben Konzepte aufgefallen, die im aktuellen `DbSchema`-Vokabular (`shared/src/lib.rs` Z. 317–532) **kein direktes Gegenstück** haben. Jede dieser Lücken hat heute einen pragmatischen Workaround (Domain-Service-Code oder Konvention), aber für eine reife ERP-tauglichere Schema-Sprache (Phase 1.7-nah) wäre eine native Repräsentation wertvoll.

Dieses Dokument bündelt alle sieben Lücken in einem zentralen Backlog, damit sie nicht in einzelnen Specs versanden. Jede Lücke ist als eigenständige spätere Spec ausspielbar.

## 2. Lücken-Katalog

### G1 — Soft-Delete als first-class Konzept

**Was fehlt:** Kein nativer Soft-Delete-Mechanismus. `DbColumn` hat kein Flag wie `soft_delete_marker`, und `AuditRole` kennt nur `CreatedAt/UpdatedAt/CreatedBy/UpdatedBy`.

**Symptom:** Jedes Domain-Schema, das Soft-Delete will, muss eine `deleted_at: DateTime, nullable`-Spalte als Konvention führen, und der Server muss überall den Default-Filter `WHERE deleted_at IS NULL` selbst durchsetzen. Vergessen heißt: gelöschte Zeilen tauchen in Listen wieder auf. Es gibt keinen Server-Default, kein Restore-API, keine GoBD-konforme Hard-Delete-Sperre.

**Heutiger Workaround:** Konvention. `deleted_at`-Spalte im Schema, Filter im Resolver pro Entity-Type. Dokumentiert in jedem Schema-Doc neu.

**Empfehlung:**
- Neues `AuditRole::DeletedAt` (und optional `DeletedBy`) — Server filtert automatisch, wenn eine Spalte diese Rolle trägt.
- `EntitySettings.soft_delete: bool = false` — explizites Opt-in pro Entity-Type.
- Mutationen `restore(entity_id)` und `purgeDeleted(entity_id)` als generische GraphQL-Resolver.
- Permission-Op `Restore` ergänzen (Phase 0.7 Resource-Op-Matrix).

**Triggering use cases:** D2V-Journal (rechtssichere Stornierung), Phase 1.7.4 (GoBD-Append-Only), jede ERP-Anwendung mit Audit-Anspruch.

**Größe:** M.

---

### G2 — CHECK-Constraints / Spalten-Invarianten

**Was fehlt:** `DbColumn` hat `default_value: Option<String>` und `nullable`, aber kein `check_expression`. Auch `DbTable` kennt keine Tabellen-CHECK-Constraints (mehrspaltig).

**Symptom:** Invarianten wie `value > 0` (D2V `journal_entry.value`), `month BETWEEN 1 AND 12` (D2V `journal_stack.month`), `number BETWEEN 1 AND 99999` (D2V `account.number`) lassen sich nicht im Schema festhalten. Wenn ein Plugin oder eine zukünftige Source die Tabellen direkt beschreibt, sind diese Garantien futsch.

**Heutiger Workaround:** Domain-Service prüft vor Save. Kein DB-Level-Schutz. CSV-Importer, fremde Tools oder Plugins können kaputte Werte einbringen.

**Empfehlung:**
- `DbColumn.check: Option<CheckExpression>` und `DbTable.checks: Vec<NamedCheck>`.
- `CheckExpression` als typisierter Mini-AST (analog `GuardExpr` aus Phase 1, `shared/src/builder/guard.rs`): `Compare { left: Ref, op, right: Literal }`, `In { ref, values }`, `Between { ref, min, max }`, `IsNull/IsNotNull`, kombiniert mit `And/Or/Not`.
- Engine-Mapping: Postgres/SQLite/MSSQL haben native `CHECK` — direkt durchreichen. Wo nicht: Source-Layer prüft vor `INSERT`/`UPDATE`.

**Triggering use cases:** D2V-Journal-Invarianten, Phase 1.7 (ERP-Bausteine), Validatoren-Migration aus DATEV-Domain (siehe `Model/DatevAccount.cs:213-301`).

**Größe:** L (wegen der Mini-DSL).

---

### G3 — Bitflags-Spalten als typisierter Spalten-Modus

**Was fehlt:** Keine native Bitflags-Repräsentation. `DbColumnType::Integer` plus `FieldType::Integer` ist semantisch zu schwach: der Editor zeigt eine Zahl, nicht eine Multi-Check-Box mit Flag-Namen.

**Symptom:** D2V's `account.account_type` ist ein 9-Bit-Set (BS, Profit, Loss, Debitor, Creditor, VatPossible, Automatic, Special, NoEbCheck). Im dblicious-CRUD-Editor erscheint das als nackte Integer-Spalte. Im Rust-Domain-Layer muss man jedes Mal von Hand mit `bitflags!`-Crate konvertieren.

**Heutiger Workaround:** Plain `Integer`-Spalte, Rust-Seite mit `bitflags!`-Crate. Im Editor keine sinnvolle Darstellung.

**Empfehlung:**
- Neue `FieldType::Bitflags { values: Vec<BitflagValue> }` mit `BitflagValue { bit: u8, label_key: String, default: bool }`.
- Standard-Editor `bitflags-checkbox-group`, der die Bit-Positionen rendert.
- `DbColumnType` bleibt `Integer` — die `FieldType`-Verfeinerung reicht (Schema-Layer = "wie speichern", Field-Layer = "wie darstellen").

**Triggering use cases:** D2V `account_type`. Andere ERP-Use-Cases mit Set-of-Flags (User-Rollen, Berechtigungs-Masken, Status-Sets).

**Größe:** M.

---

### G4 — Computed / Generated Spalten

**Was fehlt:** `DbColumn` hat `ColumnGenerated` (Never/OnAdd/OnAddOrUpdate) für **System-gefüllte** Werte, aber keine Ausdrucks-basiert generierten Spalten wie `GENERATED ALWAYS AS (factor * value) STORED`.

**Symptom:** D2V `journal_line.debit_value` und `credit_value` sind aus `factor * value` plus Vorzeichen ableitbar — die Berechnung lebt heute im Domain-Service (`Model/DatevAccountEntry.cs`). Bei einer Postgres-Source könnte das eine GENERATED-Spalte sein, mit DB-Garantie.

**Heutiger Workaround:** Domain-Service setzt die Spalten bei Save. Bei Postgres-`UPDATE`s ohne dblicious-Layer (Plugin, Migration) keine Garantie.

**Empfehlung:**
- `DbColumn.compute: Option<ComputeExpression>` mit kleinem AST (`Op { left, op, right }`, Spalten-Refs, Literale, simple Funktionen wie `ABS`, `COALESCE`).
- Engine-Mapping: Postgres + SQLite (ab 3.31) + MSSQL haben `GENERATED`-Spalten — direkt durchreichen. Wo nicht (z. B. ältere SQLite-Versionen): Source-Layer berechnet vor `INSERT`/`UPDATE`.
- `DbColumn.compute_stored: bool` — `true` = persistiert, `false` = virtuell (nur bei `SELECT` berechnet).

**Triggering use cases:** D2V `journal_line.debit/credit_value`, jede Buchhaltungs-App mit abgeleiteten Geld-Spalten.

**Größe:** L.

---

### G5 — Materialisierte Views / Schema-Level-Caches

**Was fehlt:** `DbSchema` kennt nur Tabellen, keine View-Definitionen.

**Symptom:** D2V's frühere `DatevCalculationValue`-Tabelle war ein händisch verwalteter Cache aus aggregierten Account-Balances. Im neuen Schema entfällt die Tabelle, weil das zur Laufzeit berechnet wird — aber für große Datenmengen (mehrere Jahre, Hunderte Konten) wäre ein materialisierter Cache wertvoll. Postgres bietet das nativ, dblicious kann es heute nicht ausdrücken.

**Heutiger Workaround:** Server-Session-Cache im Domain-Service. Verloren bei Server-Neustart, nicht persistent, nicht von anderen Clients abgreifbar.

**Empfehlung:**
- Neue Top-Level-Entität `DbView { id, name, definition: ViewDefinition, materialized: bool, refresh: RefreshStrategy }` in `DbSchema`.
- `ViewDefinition` als typisierte Query-DSL (Subset von SQL: SELECT-Liste, FROM/JOIN, WHERE, GROUP BY) — analog zu `GuardExpr`/`CheckExpression`.
- `RefreshStrategy`: `OnDemand`, `Scheduled { cron }`, `OnDependencyChange`.
- Engine-Fallback: Postgres = `MATERIALIZED VIEW`, andere = regelmäßiger `INSERT INTO cache_table SELECT ...` per Job-Scheduler (Phase 1.7 ERP-Baustein "Job-Scheduler").

**Triggering use cases:** D2V Calculation-Caching, Phase 1.7 Aggregations-Building-Block.

**Größe:** L–XL (eigene Query-DSL).

---

### G6 — Partielle Indexes

**Was fehlt:** `DbIndex` hat `unique: bool`, `column_ids: Vec<String>`, aber kein `where_clause` für gefilterte/partielle Indexes.

**Symptom:** Klassiker: ein UNIQUE-Index auf `account.number` soll nur greifen, wo `deleted_at IS NULL`. Sonst kollidiert ein gelöschtes Konto mit einem neu angelegten gleicher Nummer. Heute keine Möglichkeit, das im Schema festzuhalten.

**Heutiger Workaround:** Voll-Index. Konflikt-Handling im Domain-Service (`account_uniqueness_checker`-Pattern).

**Empfehlung:**
- `DbIndex.where_clause: Option<CheckExpression>` (gleiche AST wie G2).
- Engine-Mapping: Postgres + SQLite haben native partial indexes; MSSQL hat "filtered indexes"; MySQL nicht — dort Source-Layer-Workaround (Trigger oder App-Level-Check).

**Triggering use cases:** Jede Schema-mit-Soft-Delete-Kombination (also fast jedes ERP-Schema), Optimierung selektiver Hot-Path-Queries.

**Größe:** S (klein, wenn G2 schon existiert; sonst Mini-DSL mitziehen).

---

### G7 — Typsichere Enums in der Schema-Sprache

**Was fehlt:** `FieldType::Enum { values: Vec<String> }` ist auf String-Werte gepinnt. Schema-Seite kennt keinen Integer-basierten Enum mit benannten Werten.

**Symptom:** D2V `journal_entry.value_type` ist ein Integer (0=SOLL, 1=HABEN). Im Wire-Format will man `"SOLL"`/`"HABEN"` haben, in der DB einen Integer für Speicherplatz und Index-Eignung. Heute muss man entweder die Spalte als `Text` führen (Speicher-/Performance-Kosten) oder die Mapping-Logik manuell in jeder Resolver-Schicht wiederholen.

**Heutiger Workaround:** Spalte als Integer, Rust-Domain mit `TryFrom<i32>`, im Editor `FieldType::Enum` mit String-Mapping in der Resolver-Schicht.

**Empfehlung:**
- `FieldType::IntEnum { values: Vec<IntEnumValue> }` mit `IntEnumValue { value: i32, label_key: String, wire_name: String }`.
- Server konvertiert beim Lesen `i32 → wire_name`, beim Schreiben umgekehrt, automatisch.
- `DbColumnType` bleibt `Integer` — die `FieldType`-Verfeinerung trägt die Semantik.

**Triggering use cases:** D2V `value_type`, `booking_text.category`, `calculation.value_type`, `calculation.calculation_type`. Generell jede Status-/Typ-Spalte in ERP-Apps.

**Größe:** S.

---

## 3. Priorisierung

| Lücke | Größe | Priorität | Begründung |
|---|---|---|---|
| G1 Soft-Delete | M | **hoch** | Triggern Phase 1.7.4 (GoBD), praktisch alle Domain-Apps |
| G6 Partielle Indexes | S | **hoch** | Direkter Folgeschritt von G1 (Unique mit Filter); kleine Implementierung |
| G7 IntEnum | S | **hoch** | Klein, direkter Wert für jede Status-Spalte |
| G2 CHECK-Constraints | L | mittel | Wertvoll, aber Workaround "Domain-Service prüft" ist akzeptabel — eine Mini-DSL ist nicht trivial |
| G3 Bitflags | M | mittel | Wertvoll für UI, aber nicht Schema-blockierend |
| G4 Computed Spalten | L | niedrig | Selten zwingend; Domain-Service-Berechnung reicht meist |
| G5 Materialized Views | L–XL | niedrig | Eigene Query-DSL, hoher Aufwand — Session-Cache reicht heute |

Empfohlene Reihenfolge: **G1 → G6 → G7 → G2 → G3 → G4 → G5**.

## 4. Beziehung zur bestehenden Roadmap

- Keine harten Abhängigkeiten zu Phase 0.6 (Source-Architektur) oder Phase 0.7 (Auth/Permissions) — die Lücken sind orthogonal.
- G1 ist ein **Vorbedingung-Kandidat für Phase 1.7.4** (GoBD-Append-Only) — dort wird Soft-Delete benötigt.
- G2 und G6 sind Voraussetzung für stabile Multi-Mandanten-Indexes (Phase 0.7 `tenant_id` UNIQUE-Indexes).
- G5 könnte alternativ als Phase-1.7-ERP-Baustein ("Aggregations") realisiert werden — wäre dort allerdings nicht Schema-Sprach-Erweiterung, sondern eigenständiger Layer.

## 5. Entscheidung

Diese sieben Items werden **nicht** sofort umgesetzt — der Status ist "Backlog gesammelt". Sobald eine konkrete Domain-Anwendung (D2V-Neubau Track C oder ein anderer Use Case) eines der Items dringend braucht, wird daraus eine eigenständige Spec.

Bis dahin: die Workarounds im Domain-Service sind ausreichend dokumentiert in [`2026-05-20-d2v-target-schema-design.md`](./2026-05-20-d2v-target-schema-design.md) §"Was dblicious heute (noch) nicht abdeckt".

## 6. Referenzen

- `C:\Users\jz\source\DBlicious\shared\src\lib.rs` Z. 317–532 — aktuelle Schema-Sprache (`DbSchema`, `DbColumn`, `DbColumnType`, `AuditRole`, `ColumnGenerated`, `DeleteBehavior`, `RelationKind`).
- `C:\Users\jz\source\DBlicious\shared\src\builder\guard.rs` — `GuardExpr` als Vorbild für eine typisierte Mini-DSL (relevant für G2, G6, G4).
- [`2026-05-19-dblicious-source-architecture-design.md`](./2026-05-19-dblicious-source-architecture-design.md) — Source-Trait, in dem viele dieser Lücken via Source-Layer-Fallback abgefangen werden müssten.
- [`2026-05-20-d2v-target-schema-design.md`](./2026-05-20-d2v-target-schema-design.md) — D2V-Ziel-Schema, das diese Lücken konkret aufzeigt.
- `ROADMAP.md` Phase 1.7 — ERP-Plattform-Bausteine, mit denen einige dieser Lücken thematisch koppeln.
