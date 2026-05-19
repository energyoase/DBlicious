# D2V2019 → DBlicious: Gesamt-Lücken-Analyse

Date: 2026-05-20
Status: Draft — awaiting user review

## 1. Frage

> Was fehlt, um **alle** D2V2019-Funktionalität (ob heute funktionierend oder nur angedacht) in DBlicious umzusetzen?

Dieses Dokument ist die **konsolidierte Gap-Analyse über alle drei Migrations-Tracks** (siehe Memory [[d2v-on-dblicious-migration]]):

- **Track A** = Datenport (`examples/d2v/`) — `d2v.db` über DBlicious-CRUD sichtbar.
- **Track B** = DBlicious-Source-Architektur (Phase 0.6) — Voraussetzung für Track A.
- **Track C** = Funktions-Ports — die eigentliche D2V-Domain-Logik.

Bereits existierende Specs decken Teile ab; dieses Dokument identifiziert, **was darüber hinaus fehlt** und in keinem Plan steht — vor allem **UI-Patterns** und **cross-entity-Operationen**, die in der bisherigen Roadmap nicht erscheinen.

## 2. Inventar D2V2019 (vollständig)

Erhoben aus `C:\Users\jz\source\D2V2019\D2V2019\`. Kategorisiert nach Verantwortlichkeit.

### 2.1 Domain-Modelle (`Model/`)

| Klasse | Rolle | Zustand laut User |
|---|---|---|
| `Company` | Kreuzref DATEV↔StarMoney | nie funktional |
| `DatevAccount` + `DatevAccounts` (Konstanten) + `DatevAccountType` (Bitflags) + `KnownKeys` | Kontenrahmen + Spezialkonten | funktional, **extrem wichtig** |
| `DatevAccountEntry` | computed Sub-Buchungen | Output von `GenerateAccountEntries` |
| `DatevEntry` + `DatevEntryBase` | Buchung | funktional |
| `DatevEntryStack` | Import-/Edit-Marker | aktiv genutzt |
| `DatevEntryGroup` | Stub (nur `Id`) | unklar — am Datenport evaluieren (Backlog T9b) |
| `DatevEntryChangeTracking` + `…ImportGroup` | Append-only-Historie | programmiert, nie debugged |
| `DatevCalculation` + `DatevCalculationEntry` + `DatevCalculationValue` + `DatevCalculationType` | Bilanz/GuV-Baum | funktional |
| `DatevInvertPairSearchResult` | Runtime-Suche | Funktional, FunctionControl |
| `DatevToStarMoneySearch` + `…Result` + `…ResultType` + `…Stage` | Reconciliation | nie funktional |
| `StarMoneyBank`, `StarMoneyAccount`, `StarMoneyBookingText` | Stammdaten | funktional |
| `StarMoneyEntry` | Bank-Buchung (40+ Spalten flach) | funktional, aber Importer veraltet |
| `StarMoneyCreditCard` + `…Entry` | Kreditkarte | nie funktional |
| `StarMoneyCompareToDatev` | Reconciliation-Runtime | nie funktional |
| `SuSaEntry` + `…Base` + `SuSaCompareEntry` + `SuSaDifference` | Summen-/Saldenliste | funktional |
| `ValueType` | Enum SOLL/HABEN | funktional |
| `EntityBase` | Audit/RowVersion | funktional |

### 2.2 Lib (`Lib/`) — Services und Helpers

| Datei | Zweck |
|---|---|
| `Logger.cs` | Logging |
| `Extensions.cs` | LINQ-/String-Utilities |
| `EntityException.cs` | typisierte Exceptions |
| `IbanHelper.cs` | IBAN Modulo-97-Validierung, BIC-Lookup |
| `DescriptionSplitter.cs` | Heuristik auf StarMoney `Description1..14` |
| `CSVView.cs` | generischer XLSX/CSV-Loader |
| `ImportExport.cs` | Basis-Klasse |
| `DatevAccountImportExport.cs` | DATEV-Kontenplan Import/Export |
| `DatevEntryImportExport.cs` | DATEV-Buchungen Import/Export |
| `DatevEntryChangeTrackingImportExport.cs` | History I/O |
| `DatevCalculationImportExport.cs` | Bilanz-Baum I/O |
| `StarMoneyAccountImportExport.cs` | Bank-Konten I/O |
| `StarMoneyEntryImportExport.cs` | Bank-Buchungen Import (heute irrelevant — CSV der Banken stattdessen) |
| `JahresabschlussHelper.cs` | Jahresabschluss-Berechnung |

### 2.3 PDF (`D2VPdf/`)

| Datei | Zweck |
|---|---|
| `Document.cs`, `Template.cs`, `PageType.cs`, `Entry.cs`, `Data.cs` | PDF-Engine-Wrappers |
| `JahresabschlussBase.cs`, `Jahresabschluss.cs` | Jahresabschluss-Layout — **1:1 zu übernehmen** (User-Klarstellung) |

### 2.4 UI-Controls (`UI/`)

Pro Entity-Type ein `*Controls/`-Ordner mit gleichbleibendem Muster:

| Pattern | Beispiel | Vorkommen |
|---|---|---|
| **`List.xaml`** | `DatevEntryControls/List.xaml` | je Entity-Type |
| **`Editor.xaml`** | `DatevEntryControls/Editor.xaml` | je Entity-Type |
| **`Selector.xaml`** | `StarMoneyAccountSelector.xaml`, `AccountSelector.xaml`, `CompanySelector.xaml`, `BankSelector.xaml`, `StarMoneyBookingTextSelector.xaml`, `ValueTypeSelector.xaml` | FK-Picker mit Autocomplete |
| **`SaveReport.xaml`** | `DatevEntryControls/SaveReport.xaml`, `CompanyControls/SaveReport.xaml`, `DatevAccountControls/SaveReport.xaml`, `ContextControls/ContextSaveReport.xaml` | Preview-vor-Commit |
| **`Import.xaml`** / **`Export.xaml`** | `DatevEntryControls/Import.xaml`, `…/Export.xaml`, `JA/Export.xaml` | spezielle CSV-/XLSX-/PDF-Pfade |
| **`SearchList.xaml`** | `DatevAccountControls/SearchList.xaml`, `DatevEntryControls/SearchList.xaml` | filterbare Suche |
| **`Filter.xaml`** | `DatevEntryControls/DatevEntryFilter.xaml` | strukturierter Filter-Builder |
| **Cross-Table-View** | `DatevAccountEntryControls/DatevAccountView.xaml`, `SuSaControls/SuSaView.xaml`, `DatevCalculationControls/View.xaml`, `DatevCalculationChildren.xaml`, `DatevCalculationChild.xaml`, `StarMoneyEntryControls/CompareToDatevList.xaml` | Auswertungs-/Aggregations-Sichten quer über Tabellen |
| **Function-Controls (Wizard)** | `FunctionControls/CalcStarMoney.xaml`, `…/PublishDatev.xaml`, `…/GenerateDatevExternalIds.xaml`, `…/FindDatevInversePairs.xaml`, `…/FullImportExport.xaml`, `…/FindDatevForStarMoney/{Search,Conflict,Confirmation,Select}.xaml` | mehrstufige Operationen, **nicht** entity-CRUD |
| **Test/Debug** | `DatevEntryControls/DatevEntryToAccountEntriesTest.xaml` | "Was-bewirkt-diese-Buchung"-Vorschau |
| **Context-Trigger** | `ContextControls/UpdateDatevAccountEntries.xaml` | "Berechnete Felder neu erzeugen"-Button mit Bericht |
| **App-Shell** | `ContentDocker.xaml`, `EditorView.xaml`, `ListView.xaml`, `YearSelector.xaml`, `JA/Export.xaml` | Layout-Docking, Global-Year-Context |
| **Base-Klassen** | `ListBase.cs`, `EditorBase.cs`, `SelectorBase.cs`, `ImportBase.cs`, `ExportBase.cs`, plus pro-Entity-Type `…Base.cs` (z.B. `DatevEntryListBase.cs`) | gemeinsame Logik |
| **Comparer** (`Comparer/`) | `DatevEntryPrimaryAccountComparer.cs` etc. | Sortier-/Match-Helpers |
| **Converter** (`Converter/`) | `DecimalToIntConverter`, `EnumValueTypeToStringConverter`, `BoolToVisibility…`, `EmptyString…`, `ErrorDictionaryToString…` | XAML-Bindings-Konverter (Formatter) |

### 2.5 Globale UI-Eigenschaften

- **YearSelector** als globaler Kontext: alle Buchungs-/SuSa-Listen filtern automatisch auf den gewählten Year.
- **ContentDocker**: das App-Shell-Layout (Master-Detail + Reports + Wizard-Panel).
- **Mandant-Konzept**: im Code angedeutet (`ContextControls/`), in Praxis nie genutzt — kein Migrations-Bedarf.

## 3. Status pro Funktion gegen heutigen DBlicious-Stand

Legende: ✅ vorhanden / 🚧 in spec geplant (uncommitted) / 📋 Backlog (T#) / ❌ keine Spur.

### 3.1 Datenzugriff

| Item | Status | Anker |
|---|---|---|
| Generische CRUD-Tabelle pro Entity | ✅ | `client/src/components/table/` |
| Fremde SQLite-DB lesen | 🚧 | Track B: `foreign-sqlite` Source (Phase 0.6 B1, branch `feat/phase-0.6-source-architecture`) |
| Composite-PKs | 🚧 | Track B §3.5, `shared::source::EntityId` |
| Read-only-Bindings pro Entity | 🚧 | Track B §5.4 |
| Spalten-Mapping PascalCase ↔ camelCase | 🚧 | Track B §3.3 `EntityBinding.column_map` |
| Andere SQL-Engines (Postgres/MySQL) | 📋 | Phase 0.6 B2 |
| Nicht-DB-Sources (REST/File/Memory) | 📋 | Phase 0.6 B3 |

### 3.2 Schema-Sprache (für ein D2V-Ziel-Schema)

| Item | Status | Anker |
|---|---|---|
| Surrogate-PKs, FK, Indexes, Audit, Concurrency-Token | ✅ | `shared::DbSchema` |
| Soft-Delete (G1) | 🚧 | `docs/superpowers/plans/2026-05-20-g1-soft-delete-first-class.md` |
| CHECK-Constraints (G2) | 🚧 | `…/g2-check-constraints.md` |
| Bitflags-Spalten (G3) | 🚧 | `…/g3-bitflags-field-type.md` |
| Computed/Generated Spalten (G4) | 🚧 | `…/g4-computed-columns.md` |
| Materialized Views (G5) | 🚧 | `…/g5-materialized-views.md` |
| Partielle Indexes (G6) | 🚧 | `…/g6-partial-indexes.md` |
| IntEnum (G7) | 🚧 | `…/g7-int-enum-field-type.md` |

### 3.3 D2V-Domain-Funktionen (Track C — T1–T23 aus `2026-05-19-d2v-data-port-design.md` §10)

| # | Funktion | Status |
|---|---|---|
| T1 | Companies cross-ref | 📋 |
| T2 | DATEV-Konten Liste/Editor/Import/Export | 📋 |
| T3 | DATEV-Buchungen Liste/Editor/Filter/Suche | 📋 |
| T4 | DATEV-Buchungen Import/Export | 📋 |
| **T5** | **`GenerateAccountEntries`** (USt-Auto-Split, Hospitality, Skonto) | 📋 **extrem wichtig** |
| **T6** | DATEV-Account-Entries als computed Sub-Buchungen | 📋 **extrem wichtig** |
| T7 | DATEV-Calculations | 📋 |
| T8 | DATEV-Entry-Change-Tracking | 📋 |
| T9a | DATEV-Entry-Stacks als Filter-Marker | 📋 |
| T9b | DATEV-Entry-Groups — Stand klären | 📋 |
| T10 | Invert/Generalumkehr | 📋 |
| T11 | StarMoney-Banken/Konten | 📋 |
| T12 | StarMoney-Buchungen Liste/Editor | 📋 |
| T13 | Bank-CSV-Import (config-driven Mapping) | 📋 |
| T14 | StarMoney-BookingTexts Katalog | 📋 |
| T15 | StarMoney-CreditCard | 📋 (war nie funktional) |
| T16 | StarMoney↔DATEV-Abgleich | 📋 (war nie funktional) |
| T17 | SuSa | 📋 |
| **T18** | Jahresabschluss PDF + Export | 📋 **Layout 1:1** |
| T19 | Voucher (Excel-Export pro Buchung) | 📋 |
| T20 | YearSelector | 📋 |
| T21 | DescriptionSplitter | 📋 (Nutzen kritisch prüfen) |
| T22 | IBAN-Helper | 📋 |
| T23 | CSV-View | 📋 |

### 3.4 ERP-Plattform-Bausteine (Phase 1.7 — bereits in Roadmap)

| Phase-1.7-Paket | D2V-Bedarf | Status |
|---|---|---|
| 1.7.1 Number-Sequences (gapless) | DATEV-ExternalIds | 📋 |
| 1.7.2 FX-Rate-Store | StarMoney FX-Spalten (`origin_value`, `origin_currency`, `foreign_fee`) | 📋 |
| 1.7.3 Period-Locks | aktive Jahresgrenze für Buchungen | 📋 |
| 1.7.4 GoBD Append-Only | `is_published` Lock | 📋 |
| 1.7.5 State-Machine | Buchungs-Lifecycle (Draft → Posted → Inverted) | 📋 |
| 1.7.7 Job-Scheduler | nichts D2V-spezifisches direkt | irrelevant für D2V |
| 1.7.8 File-Storage | Belege/Anhänge (nicht im Alt-System gebraucht) | optional |
| 1.7.9 PDF-Generierung | **Jahresabschluss** (T18) | 📋 |
| 1.7.12 Aggregation-Queries | **Calculation-Bäume** + SuSa | 📋 |
| 1.7.13 Volltextsuche | DATEV-Entry-Suche (heute via SearchList) | optional |
| 1.7.14 CSV/Excel-Export | DATEV-Export + Voucher (T19) | 📋 |
| 1.7.15 Bulk-Import | DATEV-Import + Bank-CSV (T4, T13) | 📋 |
| 1.7.20 Tree-FieldType | Calculation-Baum | 📋 |

## 4. Tatsächliche Lücken

Die folgenden vier Cluster sind **in keinem heutigen DBlicious-Spec oder Plan** abgedeckt — sie sind das, was über Track B + G1–G7 + T1–T23 + Phase 1.7 hinaus zusätzlich fehlt.

### 4.1 UI-Pattern-Lücken

D2V hat wiederkehrende UI-Muster, für die DBlicious heute kein Pendant hat. Eine Funktions-Port-Spec (Track C) ist ohne diese Patterns nicht umsetzbar.

#### U1 — Typed FK Selector (Picker mit Autocomplete + Rich Format)

**D2V**: `AccountSelector.xaml`, `StarMoneyAccountSelector.xaml`, `CompanySelector.xaml`, `BankSelector.xaml`, `StarMoneyBookingTextSelector.xaml`, `ValueTypeSelector.xaml`. Jeder Selector lädt die Ziel-Tabelle, bietet Suche, zeigt mehrere Spalten als Auswahl-Format, gibt die FK-Spalte zurück.

**DBlicious heute**: `FieldType::Reference` zeigt die ID als Text. Es gibt einen Picker-Registry-Stub (`client/src/components/registries/picker.rs`), aber kein produktives "FK-Picker mit konfigurierbarem Format und Suche". Phase 0.6 Source-Arch §9 verweist auf "FK-Auto-Resolution" als spätere Backlog-Item.

**Lücke**: kein konfigurierbarer FK-Picker. Heutiges `FieldType::Reference` ist nicht ausreichend.

**Empfehlung**: neue Spec **U1 — Referenz-Picker** mit:
- `FieldType::Reference { target_entity, display_template: String, search_columns: Vec<String> }`
- Client-Component, die per GraphQL eine paginierte Suche auf der Ziel-Entity feuert.
- Verbindet sich mit Phase 1.5-Implementations-Resolution (Picker-Registry).

**Größe**: M. **Trigger**: jede D2V-Buchungs-Editor-Funktion (T3, T11–T12).

#### U2 — SaveReport / Preview-vor-Commit

**D2V**: `*SaveReport.xaml` pro Entity + `ContextSaveReport.xaml`. Vor dem Speichern erscheint ein Bericht, was sich ändern wird (neue Zeilen, geänderte Zeilen, abgeleitete Berechnungen z.B. aus `GenerateAccountEntries`).

**DBlicious heute**: Editor speichert direkt. Keine Preview-Schicht. `client/src/components/table/builder_preview.rs` ist ein Builder-Live-Preview, kein Save-Report.

**Lücke**: kein "Preview-Diff vor Commit"-Pattern. Für Track C T5 (GenerateAccountEntries) ist das essentiell — der User muss sehen, welche Account-Entries automatisch erzeugt werden, bevor er bestätigt.

**Empfehlung**: neue Spec **U2 — Mutation-Preview-Pipeline**:
- Server-Mutation `previewSave(entity_type, payload) -> SavePreview { creates, updates, deletes, computed_rows }` ohne Commit.
- Client-Modal mit Diff-Tabelle.
- Kommit-Mutation referenziert die Preview-ID (Server hält die Berechnung kurz vor).
- Hängt mit Track C T5/T6 zusammen (computed Sub-Buchungen).

**Größe**: M-L. **Trigger**: T5, T6, T15-Bulk-Import.

#### U3 — Master/Detail Cross-Table View

**D2V**: `DatevAccountView.xaml` zeigt pro Konto alle Buchungen mit Soll/Haben-Spalten, aggregiert nach Datum, mit Saldo-Spalte. `SuSaView.xaml` und `CompareTest.xaml` sind analoge Sichten. `DatevCalculation/View.xaml` rendert den Bilanz-Baum rekursiv mit Werten pro Jahr.

**DBlicious heute**: `EntityTable` ist single-table-CRUD. Keine "join + aggregate + render"-Sicht. Phase 1.7.12 (Aggregation-Queries) liefert Daten, aber kein UI-Pattern.

**Lücke**: keine UI-Komponente für Cross-Table-Aggregation mit Tree-/Pivot-Layout.

**Empfehlung**: neue Spec **U3 — Report-View-Component**:
- `report.json` pro Report-Type, deklariert Source-Query (über 1.7.12 Aggregation), Spalten, Tree/Pivot/Linear-Layout.
- Client-Komponente analog zu `EntityTable`, aber für Read-only-Joins.
- Generisch genug für DatevAccountView, SuSaView, Calculation-Baum.

**Größe**: L. **Trigger**: T6 (DatevAccountEntry als Account-View), T7 (Calculations), T17 (SuSa).

#### U4 — Multi-Step-Wizard

**D2V**: `FindDatevForStarMoney/{Search,Conflict,Confirmation,Select}.xaml` — vier Schritte einer Reconciliation. `JA/Export.xaml` (multi-page-Setup + Generierung). `FullImportExport.xaml`.

**DBlicious heute**: kein Wizard-Pattern. Editor ist eine Form, keine Sequenz.

**Lücke**: keine "Steps + State + Cancel/Back"-Komponente.

**Empfehlung**: neue Spec **U4 — Wizard-Pattern**:
- `wizard.json` pro Operation: `steps: [{id, schema, validator, computed_inputs}]`.
- Client-Komponente, die Steps reaktiv rendert.
- Server-State pro Wizard-Session (analog Preview in U2).
- Verbindet sich mit U2 (letzte Wizard-Stage = Save-Report).

**Größe**: M. **Trigger**: T16 (Reconciliation), T13 (Bank-CSV-Import mit Mapping-Wizard), T18 (Jahresabschluss-Setup).

#### U5 — Globaler Kontext (YearSelector)

**D2V**: `YearSelector.xaml` setzt globales `ActiveYear`-Property. Alle Listen filtern darüber automatisch.

**DBlicious heute**: kein globales Filter-Context-Konzept. Filter ist pro Tabelle.

**Lücke**: kein "App-Level-Context, der in Queries automatisch propagiert wird".

**Empfehlung**: neue Spec **U5 — App-Context-Channel**:
- `shared::AppContext { active_year: Option<i32>, active_tenant: Option<String>, … }` als reaktiver Client-Signal + Server-Header.
- Pro Entity-Type optionales `context_filter` in Settings: `{"active_year": "$column.year"}`.
- Server-Resolver mergt Context in `FilterCriteria`.

**Größe**: S-M. **Trigger**: T20, aber auch alle Buchungs-Listen.

#### U6 — Per-Row-Action-Trigger mit Bericht

**D2V**: `ContextControls/UpdateDatevAccountEntries.xaml` — Button "Neu berechnen", Operation läuft, danach Bericht. `DatevEntryToAccountEntriesTest.xaml` ist die Test-Vorschau dazu.

**DBlicious heute**: `row_actions.rs` rendert pro-Row-Buttons, aber sie sind heute nur CRUD-Verben. Keine "Custom-Action mit progress + report"-Form.

**Lücke**: kein "lange laufende per-Row- oder per-Selection-Action mit UI-Bericht".

**Empfehlung**: neue Spec **U6 — Long-Running-Action-Pipeline**:
- Action-Registry (`shared::ActionDefinition { id, kind: RowAction | BulkAction | EntityAction, … }`).
- Server-Side: Action liefert `Stream<ActionEvent>` für Progress + Report.
- Client-Side: Modal mit Progress-Bar, Final-Report-Sicht.
- Eng verbunden mit Phase 2 (Plugin-getriebene Actions).

**Größe**: M. **Trigger**: T5 (regenerate), T10 (Invert), T16 (Reconciliation).

#### U7 — Strukturierter Filter-Builder

**D2V**: `DatevEntryFilter.xaml` — UI mit mehreren Filterzeilen (Datum-Range, Account, Stack, Inverted, Published).

**DBlicious heute**: pro Spalte ein Filter (`filters/`), aber kein "saved filter / combine-filter / strukturierter Filter-Editor mit AND/OR".

**Lücke**: kein Filter-Builder; nur per-Spalten-Quickfilter.

**Empfehlung**: neue Spec **U7 — Filter-Builder + Saved-Filters**:
- Erweiterung von `shared::FilterCriteria` um boolsche Kombinatoren (heute implizit AND).
- Client: drag-and-drop Filter-Builder analog zu Phase-1-Builder-Canvas.
- Persistenz pro User in `saved_filters`.

**Größe**: M. **Trigger**: T3 (Buchungs-Suche), generisch für jeden Listen-Use-Case.

### 4.2 Cross-Entity-Operationen

D2V's `FunctionControls/` sind **keine Entity-CRUDs** — sie sind Operationen, die viele Entities lesen/schreiben (PublishDatev, CalcStarMoney, GenerateExternalIds, FindInversePairs, FullImportExport, FindDatevForStarMoney-Wizard). DBlicious's Resolver-Schicht ist heute strikt entity-orientiert.

**Lücke**: kein "Operation = Top-Level-Resource" Konzept.

**Empfehlung**: neue Spec **F1 — Operations-Resource**:
- Neue GraphQL-Resource neben `entities` heißt `operations`.
- `OperationDefinition { id, name, input_schema, kind: SyncResult | LongRunning, permissions, … }`.
- Server-Resolver `executeOperation(id, input) -> OperationResult` mit optionalem Progress-Stream (verbindet sich mit U6).
- Navigation-Schicht zeigt Operations als eigenen Bereich (analog zu Entity-Liste).
- Phase-2-Plugins können Operations als Triggers liefern.

**Größe**: M-L. **Trigger**: T1, T10 (Invert ist eigentlich eine Operation, nicht ein Entity-Edit), T16, T13.

### 4.3 Bookkeeping-spezifische Domain-Helfer

| Item | Status | Empfehlung |
|---|---|---|
| **IBAN/BIC-Validator** als FieldType | ❌ | neues `FieldType::Iban`, Validator via `iban_validate`-Crate, BIC-Lookup in `bookkeeping_config`. **S**. T22. |
| **Datev-Konto-Nummern-Klassifikation** (`GetAutomaticTypeByNumber`) | ❌ | reine Rust-Funktion, kein Framework-Bedarf — gehört in den Domain-Crate von Track C T2. |
| **Voucher** (Excel-Export pro Buchung) | ❌ | Phase 1.7.14 (Excel/CSV-Export) deckt Listen-Export, aber **nicht** "Excel pro einzelner Buchungs-Zeile mit gestaltetem Layout". → eigene Spec **F2 — Single-Entity-Excel-Template**, knapp. **S**. T19. |
| **DescriptionSplitter** (Heuristik auf Bank-Description-Feldern) | ❌ | reines Rust, kein Framework-Bedarf — gehört in Domain-Crate. **S**. T21 (Nutzen kritisch prüfen). |
| **CSV-Layout-Mapping** (Bank-CSV mit wechselnden Layouts) | ❌ | erweitert Phase 1.7.15 (Bulk-Import) um "Mapping-Profil pro Quelle". Nicht harte Lücke, aber Phase-1.7.15-Spec muss das explizit aufnehmen. **M-Add-On**. T13. |

### 4.4 Compliance/Append-Only

Phase 1.7.4 (GoBD-Append-Only) und 1.7.3 (Period-Lock) decken den User-Need. **G1 Soft-Delete** ist Vorbedingung für 1.7.4. Keine zusätzliche Lücke.

**Hinweis**: `DatevEntryChangeTracking` (alte Verkettung) wird durch das geplante `audit_log` (Phase 0.7.6, `server/src/audit.rs` existiert) ersetzt — keine Migration der Verkettung selbst.

### 4.5 Layout & App-Shell

D2V's `ContentDocker.xaml` ist ein klassisches Master-Detail-Docking-Layout. DBlicious heute hat einfaches Routing (`/entities/:type`).

**Lücke**: kein Docking/Multi-Pane-Layout. Heute schaltet der User zwischen Routen.

**Empfehlung**: **kein eigener Spec nötig** — DBlicious's Builder-Foundation (Phase 1, `client/src/builder/`) zielt darauf ab, UI-Trees zu komponieren. Multi-Pane-Layout kann als Builder-`UiNode`-Typ realisiert werden, sobald der Builder in Production geht.

**Status**: 📋 (implizit Phase 1, kein eigener Eintrag nötig).

## 5. Konsolidiertes Lücken-Backlog

Sortiert nach Größe + Hebelwirkung. Empfohlene Reihenfolge.

| ID | Lücke | Größe | Trigger / Wo gebraucht | Reihenfolge |
|---|---|---|---|---|
| B1 | foreign-sqlite Source | M | Track A blockiert | 1 (in-flight) |
| G1 | Soft-Delete | M | jede Buchung mit Storno | 2 |
| G7 | IntEnum | S | `value_type`, `category`, `method` | 3 |
| G6 | Partial Indexes | S | mit G1 zusammen | 4 |
| **U1** | **Referenz-Picker** | **M** | **jede D2V-Form** | **5** |
| **U5** | **App-Context (YearSelector)** | **S-M** | **alle Buchungs-Listen** | **6** |
| **U2** | **Save-Report (Preview-vor-Commit)** | **M-L** | **T5/T6** | **7** |
| **U3** | **Report-View-Component** | **L** | **T6, T7, T17** | **8** |
| **F1** | **Operations-Resource** | **M-L** | **T1, T10, T13, T16** | **9** |
| **U6** | **Long-Running-Action-Pipeline** | **M** | **T5, T16** | **10** |
| **U4** | **Wizard-Pattern** | **M** | **T13, T16, T18** | **11** |
| G2 | CHECK-Constraints | L | Robustheit; nicht blockierend | 12 |
| G3 | Bitflags-FieldType | M | `account_type` | 13 |
| G4 | Computed Spalten | L | optional (Domain-Service deckt) | 14 |
| **U7** | **Filter-Builder + Saved-Filters** | **M** | **T3** | **15** |
| **F2** | **Single-Entity-Excel-Template** | **S** | **T19 (Voucher)** | **16** |
| G5 | Materialized Views | L-XL | Performance-Optimierung | 17 |
| Phase-1.7-MVP | 1.7.1 → 1.7.4 → 1.7.9 → 1.7.12 | je M | DATEV-Funktions-Ports | parallel zu 5–11 |
| T1-T23 | D2V-Domain-Specs | je M-L | jeweils nach Voraussetzungen | 18+ |

**Fett**: neu durch dieses Dokument identifizierte Lücken, in keinem heutigen DBlicious-Spec oder Plan.

## 6. Empfohlene Roadmap-Verortung

| Phase | Heute-Inhalt | Empfehlung |
|---|---|---|
| 0.6 (Source-Arch) | B0+B1 specced, in flight | unverändert |
| 0.6 | — | **U1 Referenz-Picker** dazunehmen (verbindet sich mit Phase 1.5 Picker-Registry, ist aber unabhängig benutzbar) |
| 0.7 (Auth) | unverändert | — |
| 1 (Builder) | unverändert | **U7 Filter-Builder** mit aufnehmen (Builder-Canvas-Variante) |
| 1.5 (Impl-Resolution) | unverändert | **U1 + U7** verbinden sich hier |
| **neue Phase 1.8 — UI-Patterns** | – | **U2, U3, U4, U5, U6, F1, F2** als Cluster (Phase 1.6/1.7 sind bereits belegt — Builder-Persistenz bzw. ERP-Bausteine). Diese Patterns sind nicht D2V-spezifisch — jedes ernsthaft genutzte ERP braucht sie. |
| 1.7 (ERP-Bausteine) | unverändert | Phase 1.7.15 (Bulk-Import) muss CSV-Layout-Mapping (T13) explizit aufnehmen |
| 2+ | unverändert | F1 Operations-Resource verbindet sich mit Plugin-Triggers |

Track C T1-T23 läuft dann parallel zu Phase 1.6+1.7, mit den Patterns als Dependencies.

## 7. Was bewusst NICHT für D2V gebraucht wird

- **Multi-Tenancy** (Phase 0.7): D2V ist Single-Tenant. Schema-Vorbereitung schadet nicht, aber kein D2V-Driver.
- **AI-Schema-Engine** (Phase 3): nicht D2V-relevant — das Schema steht.
- **Codegen-Production-Binary** (Phase 4): nicht D2V-relevant.
- **WASM-Plugins** (Phase 2): D2V-Logik ist hartcodiert in Rust-Domain-Crate sinnvoller; Plugin-Schicht erst dann, wenn Drittpartei-Erweiterungen erlaubt werden sollen.
- **Webhooks, eIDAS, PDF/A, WORM-Storage** (1.7.10+): nicht im D2V-Alt-System, kein Migrations-Druck.

## 8. Risiken

- **Scope-Explosion**: 7 neue UI-Pattern-Specs (U1–U7) plus 2 Operations-Specs (F1, F2). Mitigation: jede ist klein-mittel; Reihenfolge in §5 sorgt dafür, dass U1+U5 als erstes Sichtbares früh fertig sind und Track C anschieben können.
- **Track-C-Items, die Patterns voraussetzen**: T5/T6 brauchen U2 (Save-Report) und U6 (Long-Running-Action). T7 braucht U3 (Report-View). Reihenfolge in §5 berücksichtigt das.
- **Patterns sind nicht D2V-exklusiv**: U1–U7 + F1+F2 sind generische ERP-Patterns. Falls in der Zwischenzeit ein anderer User-Case (nicht D2V) aufkommt, könnten Prioritäten verschieben.

## 9. Decisions

1. **Diese Spec ist keine Bau-Spec** — sie identifiziert Lücken und schlägt 9 Folge-Specs vor (U1–U7, F1, F2). Jede dieser Folge-Specs wird im Track-Modell als Track-D (UI-Patterns) eigenständig spezifiziert.
2. **U1 (Referenz-Picker)** + **U5 (App-Context)** sind die zwei Items mit höchstem Hebel und niedrigster Größe — Empfehlung als nächste Folge-Specs nach Track-B-B1.
3. **U2/U3/U4/U6/F1** sind die "tiefen" Patterns, die D2V-Funktions-Ports (T5/T6/T7/T16/T18) erst wirklich umsetzbar machen.
4. **Phase 1.7-MVP unverändert** — die existierende ERP-Bausteine-Reihenfolge (1.7.1 → 1.7.4 → 1.7.9) deckt den Buchhaltungs-Kern.
5. **G-Plans (G1–G7) bleiben uncommittet** bis zur jeweiligen Trigger-Phase — G1 (Soft-Delete) und G7 (IntEnum) als nächste nach B1.

## 10. Referenzen

- `docs/superpowers/specs/2026-05-19-d2v-data-port-design.md` — Track A, T1–T23 Backlog.
- `docs/superpowers/specs/2026-05-19-dblicious-source-architecture-design.md` — Track B (in flight).
- `docs/superpowers/specs/2026-05-20-d2v-target-schema-design.md` — Ziel-Schema D2V-Neubau.
- `docs/superpowers/specs/2026-05-20-dblicious-schema-language-gaps.md` — G1–G7 Schema-Sprache.
- `docs/superpowers/plans/2026-05-20-g{1..7}-*.md` — uncommitted Pläne.
- `ROADMAP.md` Phase 0.6 / 0.7 / 1 / 1.5 / 1.7 / 2 / 3 / 4 — Roadmap-Kontext.
- Memory [[d2v-domain-glossary]] — User-Klarstellungen zu D2V-Begriffen.
- Memory [[d2v-on-dblicious-migration]] — Drei-Track-Struktur.
- `C:\Users\jz\source\D2V2019\D2V2019\UI\` — Quelle der UI-Pattern-Inventur (§2.4).
- `C:\Users\jz\source\D2V2019\D2V2019\Model\` — Domain-Modell-Inventur (§2.1).
- `C:\Users\jz\source\D2V2019\D2V2019\Lib\` — Lib-Inventur (§2.2).
- `C:\Users\jz\source\D2V2019\D2VPdf\` — PDF-Modul (§2.3).
