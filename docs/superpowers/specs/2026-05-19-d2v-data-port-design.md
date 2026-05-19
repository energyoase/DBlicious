# Design: D2V2019 Data Port — `examples/d2v/`

Date: 2026-05-19
Status: Draft — awaiting user review

## 1. Problem

D2V2019 is a UWP/EF Core 2.2/SQLite desktop bookkeeping tool (DATEV ↔ StarMoney). Its long-term direction is to migrate onto **DBlicious** (Rust + Leptos + WASM + axum + async-graphql + SeaORM). The full migration is far too large for a single spec.

This spec covers the **first, narrowest step**: make the existing D2V2019 SQLite database (`docs/production db/d2v.db`, ~4 MB) visible and editable through DBlicious's generic CRUD UI — **without porting any D2V domain functionality**.

The user is also planning a **broader DBlicious database-layer redesign** (introspection, schema improvements, generally better support for external SQLite sources). That work is **out of scope here** and goes into its own planning track ("Track B" below). This spec only *consumes* what Track B will deliver, as a contract.

## 2. Two-Track Setup

| Track | Scope | Owner |
|---|---|---|
| **A — This spec: `examples/d2v` Data Port** | Pure DBlicious userdata under `examples/d2v/`. Read/write the existing `d2v.db` via the generic table. No D2V logic. | This spec → Plan |
| **B — DBlicious DB-Layer Evolution** | Schema introspection of foreign SQLite DBs, composite-key support, generic FK resolution, DB-design improvements. | **Separate future spec(s).** Only the *interface contract* is referenced here. |
| **C — D2V Domain Functionality** | Account-entry generation, imports/exports, reconciliation, Jahresabschluss, GoBD, etc. | **Separate future specs**, one per feature. Backlog in §10. |

Track A is **specifiable now** but **not buildable** until Track B delivers the minimum DB-introspection contract in §3.

## 3. Contract to Track B (what `examples/d2v` needs from DBlicious)

Track A treats this as a black box. Track B will design and implement it.

**Required capabilities** (in order of necessity):

1. **Read an existing SQLite database without imposing DBlicious's own schema.** Today the server runs `Schema::create_table_from_entity` for the built-in `entities`/`users`/etc. tables at boot. With an external DB, the server must *not* try to create those tables on top of a foreign schema — the loader needs a mode where the DBlicious internal tables either live in a separate attached schema (`ATTACH DATABASE`) or are entirely absent for read-only-foreign-DB sessions.
2. **Schema introspection**: enumerate tables, columns (name, declared SQL type), primary keys (single or composite), foreign keys (single-column).
3. **Composite primary keys**: at least the four cases in `d2v.db`:
   - `StarMoneyAccount(BankCode, Code)`
   - `DatevAccountEntry(EntryId, AccountNr, OffsetAccountNr)`
   - `DatevCalculationValue(CalculationId, Year, Method)`
   - `SuSaEntry(AccountNr, Year)`
4. **Generic FK resolution** for the `FieldType::Reference`/`Collection` formatters (today these expect a single-column FK + lookup by id).
5. **Read-only flag per entity type**: setting `settings.read_only = true` makes the generic editor refuse Update/Delete (Create is also off). Needed for `DatevEntryChangeTracking` and the EXPERIMENTAL tables in §6.
6. **Column name mapping**: D2V's column names are PascalCase (`PrimaryAccountNr`, `BookingDate`). DBlicious's wire format is camelCase. The loader needs a per-column rename map (`columns.json` already has a `key`; it should accept an optional `dbColumn` override that defaults to the same as `key`).

**Explicitly not required for Track A** (so Track B can defer these):

- Append-only enforcement (Phase 1.7.4 GoBD)
- Period locks (Phase 1.7.3)
- Number sequences (Phase 1.7.1)
- Schema migration of `d2v.db` to a new layout
- Multi-DB-source loading (one foreign DB at a time is fine)

If Track B chooses not to deliver capability 1 (foreign-DB mode) and instead requires `d2v.db` to be migrated into a DBlicious-native shape, this spec adapts trivially — the entity definitions stay the same, only the `dbColumn` renames disappear.

## 4. `examples/d2v/` Layout

```
examples/d2v/
├── config.toml
├── navigation.json
├── translatables/
│   ├── languages.json
│   └── entries/
│       └── de.json
├── security/
│   ├── users.json
│   └── groups.json
└── entities/
    ├── company/
    ├── datev_account/
    ├── datev_entry/
    ├── datev_account_entry/
    ├── datev_entry_group/
    ├── datev_entry_stack/
    ├── datev_entry_change_tracking/
    ├── datev_calculation/
    ├── datev_calculation_entry/
    ├── datev_calculation_value/
    ├── star_money_bank/
    ├── star_money_account/
    ├── star_money_booking_text/
    ├── star_money_entry/
    ├── star_money_credit_card/
    ├── star_money_credit_card_entry/
    └── susa_entry/
        ├── columns.json
        ├── editor.json
        └── settings.json
```

**Note**: no `seed.json` anywhere. Data comes from `d2v.db`.

## 5. `config.toml`

Source-Konfiguration via `examples/d2v/sources.toml` (Track-B-Form, siehe [`2026-05-19-dblicious-source-architecture-design.md`](./2026-05-19-dblicious-source-architecture-design.md)):

```toml
# Default managed-sqlite-Source für DBlicious-eigene Tabellen
# (Auth, Audit, Plugin, Translatables, Builder-Designs).
[sources.local]
kind = "managed-sqlite"
url  = "${DBLICIOUS_DATABASE_URL:-sqlite://./dblicious-d2v.db}"

# Fremde D2V-DB als read/write-Source. Pfad wird per Env überschrieben.
[sources.d2v_legacy]
kind = "foreign-sqlite"
url  = "${D2V_LEGACY_URL:-sqlite:///pfad/zu/d2v.db}"
```

`examples/d2v/config.toml` bleibt schmal:

```toml
[locale]
default = "de"

[ui]
title = "D2V 2019 — Daten-Port"
```

**DB-Datei-Konvention** (siehe auch §12): the `d2v.db` file is **not committed** to the DBlicious repo and **not bundled** with `examples/d2v/`. Each developer places a copy at a path of their choice and points `DBLICIOUS_DATABASE_URL` at it. `examples/d2v/` ships only the metadata (columns, navigation, translatables, security). A `README.md` under `examples/d2v/` documents this.

## 6. Entity Definitions

For each entity below: DB table name, primary key (single/composite), main columns, FK relationships, status flag, and whether the editor is enabled.

Status legend:
- **ACTIVE** — editable
- **EVAL** — read-only, kept visible for inspection
- **EXPERIMENTAL** — read-only, in a "Sandbox" navigation branch, kept visible for inspection

### 6.1 Kontakt / Cross-Ref

| Entity | DB-Table | PK | Status | Editor |
|---|---|---|---|---|
| `company` | `Companies` | `Id` (int auto) | EXPERIMENTAL | read-only |

Sinn (User-Klärung 2026-05-19): Kreuztabelle, um identische Firmen (Kreditoren/Debitoren) zwischen DATEV und StarMoney zu erkennen. Nie funktional fertiggestellt.

Spalten (Auszug): `Id`, `Name`, `DatevAccountNr` (FK → `DatevAccount.Number`, optional, 1:1).

### 6.2 DATEV-Kontenplan

| Entity | DB-Table | PK | Status | Editor |
|---|---|---|---|---|
| `datev_account` | `DatevAccounts` | `Number` (int, verified at port time) | ACTIVE | enabled |

Wichtige Spalten: `Number`, `Name`, `EnglishName`, `TaxRate`, `AutomaticTaxAccountNr` (self-ref), `AutomaticReverseTaxAccountNr` (self-ref), Flags für Debitor/Kreditor/Automatic.

### 6.3 DATEV-Buchungen

| Entity | DB-Table | PK | Status | Editor |
|---|---|---|---|---|
| `datev_entry` | `DatevEntries` | `Id` (int auto) | ACTIVE | enabled |
| `datev_account_entry` | `DatevAccountEntries` | composite `(EntryId, AccountNr, OffsetAccountNr)` | ACTIVE | enabled but visually marked "derived" — these are produced by `GenerateAccountEntries` in D2V; in the data port, edits are technically possible but discouraged until the function port (T5 in backlog) lands |
| `datev_entry_group` | `DatevEntryGroups` | `Id` | **EVAL** | read-only |
| `datev_entry_stack` | `DatevEntryStacks` | `Id` | ACTIVE | enabled |
| `datev_entry_change_tracking` | `DatevEntryChangeTrackings` | `TrackingId` | EVAL | read-only |

**Note on Stack** (User-Klärung 2026-05-19): `DatevEntryStack` ist der Import-/Session-Marker — Buchungen lassen sich darüber nach Edit-/Import-Zeitpunkt filtern (z.B. EB-Werte vs. Endjahresbuchungen).

**Note on Group** (User-Klärung 2026-05-19): Zweck von `DatevEntryGroup` ist unklar — der User weiß es selbst nicht. Code-Befund: `DatevEntryGroup.cs` enthält **nur `Id` plus Backreference auf `DatevEntry.Group`**, keinerlei eigene Felder (kein Name, Datum, Beschreibung). Es gibt auch kein zugehöriges UI-Verzeichnis. Stub-Modell, vermutlich nie zu Ende gebracht; Tabelle ist möglicherweise leer in Production. Im Datenport read-only sichtbar zur Inspektion. **TODO** (Backlog T9b): klären, ob die Tabelle gefüllt ist und wenn ja, was die Zeilen bedeuten; ansonsten Kandidat zum Löschen.

**Note on ChangeTracking**: "programmiert, nie debugged" — listed read-only so the user can inspect production data and decide whether the mechanism actually produced useful records.

`datev_entry` columns (key fields): `Id`, `Date`, `Description`, `Value`, `ValueType` (enum SOLL/HABEN), `Key`, `PrimaryAccountNr` (FK → `DatevAccount.Number`), `SecondaryAccountNr` (FK), `Inverted`, `Published`, `Discount`, `DocumentInfo`, `ExternalId`, `GroupId` (FK), `StackId` (FK), `StarMoneyEntryId` (FK, nullable), `StarMoneyCreditCardEntryId` (FK, nullable).

`datev_account_entry` columns: `EntryId` (FK), `AccountNr` (FK), `OffsetAccountNr` (FK), `Factor`, `IsDiscountRelated`, derived `DebitValue`/`CreditValue` (computed in C#; in the data port shown as plain columns since DBlicious has no computed-column feature yet — **TODO** for Track C).

### 6.4 DATEV-Auswertungen

| Entity | DB-Table | PK | Status | Editor |
|---|---|---|---|---|
| `datev_calculation` | `DatevCalculations` | `Id` | ACTIVE | enabled |
| `datev_calculation_entry` | `DatevCalculationEntries` | `Id` | ACTIVE | enabled |
| `datev_calculation_value` | `DatevCalculationValues` | composite `(CalculationId, Year, Method)` | ACTIVE | enabled |

`Method` is a small enum; in the data port shown as text until §11 (post-port cleanup).

### 6.5 StarMoney-Stammdaten

| Entity | DB-Table | PK | Status | Editor |
|---|---|---|---|---|
| `star_money_bank` | `StarMoneyBanks` | `Code` (verified at port time) | ACTIVE | enabled |
| `star_money_account` | `StarMoneyAccounts` | composite `(BankCode, Code)` | ACTIVE | enabled |
| `star_money_booking_text` | `StarMoneyBookingTexts` | `Key` (verified at port time) | **LEGACY-IMPORT** read-only | read-only (defaults schlecht, kein frischer Import mehr — Aufräumen ist eigenes Backlog-Item T14) |

`star_money_account` is hierarchical via `ParentBankCode`/`ParentCode` (composite self-ref FK). Also has 1:1 optional `DatevAccountNr` linking to `DatevAccount`.

### 6.6 StarMoney-Buchungen

| Entity | DB-Table | PK | Status | Editor |
|---|---|---|---|---|
| `star_money_entry` | `StarMoneyEntries` | `Id` | ACTIVE | enabled |

`star_money_entry` has 14 numbered `Description1`..`Description14` columns, plus split-fields (`SplitValue`, `SplitTaxRate`, …) and FX-fields (`OriginValue`, `OriginCurrency`, `ForeignFee`, …). In the data port these are listed flat (one column per DB-column); presentation cleanup is post-port.

**Note** (User-Klärung 2026-05-19): native StarMoney imports are obsolete — banks deliver CSV in changing layouts now, hand-imported via custom parsers. The DB-table is still actively populated, just not from StarMoney.

### 6.7 Kreditkarten

| Entity | DB-Table | PK | Status | Editor |
|---|---|---|---|---|
| `star_money_credit_card` | `StarMoneyCreditCards` | `Id` | EXPERIMENTAL | read-only, Sandbox-Branch |
| `star_money_credit_card_entry` | `StarMoneyCreditCardEntries` | `Id` | EXPERIMENTAL | read-only, Sandbox-Branch |

### 6.8 SuSa

| Entity | DB-Table | PK | Status | Editor |
|---|---|---|---|---|
| `susa_entry` | `SuSaEntries` | composite `(AccountNr, Year)` | ACTIVE | enabled |

Loose coupling to `DatevAccount` via `AccountNr` (no formal FK in the EF model; in DBlicious shown as Reference if Track B's FK resolver supports it for this case).

## 7. `navigation.json` (sketch)

```jsonc
[
  { "labelKey": "nav.datev",       "children": [
    { "labelKey": "nav.datev_accounts",        "route": "/entities/datev_account" },
    { "labelKey": "nav.datev_entries",         "route": "/entities/datev_entry" },
    { "labelKey": "nav.datev_account_entries", "route": "/entities/datev_account_entry" },
    { "labelKey": "nav.datev_entry_groups",    "route": "/entities/datev_entry_group" },
    { "labelKey": "nav.datev_entry_stacks",    "route": "/entities/datev_entry_stack" },
    { "labelKey": "nav.datev_change_tracking", "route": "/entities/datev_entry_change_tracking" }
  ]},
  { "labelKey": "nav.calc",        "children": [
    { "labelKey": "nav.datev_calculations",        "route": "/entities/datev_calculation" },
    { "labelKey": "nav.datev_calculation_entries", "route": "/entities/datev_calculation_entry" },
    { "labelKey": "nav.datev_calculation_values",  "route": "/entities/datev_calculation_value" }
  ]},
  { "labelKey": "nav.starmoney",   "children": [
    { "labelKey": "nav.sm_banks",          "route": "/entities/star_money_bank" },
    { "labelKey": "nav.sm_accounts",       "route": "/entities/star_money_account" },
    { "labelKey": "nav.sm_booking_texts",  "route": "/entities/star_money_booking_text" },
    { "labelKey": "nav.sm_entries",        "route": "/entities/star_money_entry" }
  ]},
  { "labelKey": "nav.susa",        "children": [
    { "labelKey": "nav.susa_entries", "route": "/entities/susa_entry" }
  ]},
  { "labelKey": "nav.contacts",    "children": [
    { "labelKey": "nav.companies", "route": "/entities/company" }
  ]},
  { "labelKey": "nav.sandbox",     "children": [
    { "labelKey": "nav.sm_credit_cards",         "route": "/entities/star_money_credit_card" },
    { "labelKey": "nav.sm_credit_card_entries",  "route": "/entities/star_money_credit_card_entry" }
  ]}
]
```

The "Sandbox" branch makes the EXPERIMENTAL status visually explicit.

## 8. `translatables/`

`languages.json`: `de` (Primary), `en` (carried over from the base shop example, sparse).

`entries/de.json`: one entry per `labelKey` used in the navigation and per `column.labelKey`. Auto-generated stubs to start, then hand-polished. Initial DE-only is fine; EN can lag.

## 9. `security/`

Minimal:
- `users.json`: one user `bookkeeper@local`.
- `groups.json`: one group `bookkeepers`, with full CRUD permission on all ACTIVE entities, read-only on EVAL/LEGACY-IMPORT/EXPERIMENTAL.

When Phase 0.7.2–0.7.4 (server-side permission enforcement) lands, this gets a real `permissions.json` matching the Status column in §6. Until then, the existing client-side `AuthContext` legacy-fallback covers it.

## 10. Out-of-Scope — D2V Functionality Backlog

These were enumerated and commented during the brainstorm. Each gets its own spec when picked up. Not planned here.

| # | Funktion | Quelle in D2V | User-Anmerkung (2026-05-19) |
|---|---|---|---|
| T1 | Companies cross-ref DATEV↔StarMoney | `UI/CompanyControls/` | Konzept war Kreditor/Debitor-Matching; nie funktional |
| T2 | DATEV-Konten Liste/Editor/Import/Export | `UI/DatevAccountControls/`, `Lib/DatevAccountImportExport.cs` | Funktional OK, Verbesserung: config-getriebenes Import-Mapping |
| T3 | DATEV-Buchungen Liste/Editor/Filter/Suche | `UI/DatevEntryControls/{List,Editor,Filter,SearchList}.xaml` | Funktional OK |
| T4 | DATEV-Buchungen Import/Export | `Lib/DatevEntryImportExport.cs` | Funktional OK, Verbesserung: config statt hardcoded |
| **T5** | **`GenerateAccountEntries`** (USt-Auto-Split, Hospitality, Skonto, Debitor/Kreditor-Summen) | `Model/DatevEntry.cs:163-440` | **Extrem wichtig**, muss immer mit DATEV abgeglichen werden |
| **T6** | DATEV-Account-Entries als computed Sub-Buchungen | `UI/DatevAccountEntryControls/` | **Extrem wichtig**, gekoppelt an T5 |
| T7 | DATEV-Calculations | `UI/DatevCalculationControls/` | Funktional OK |
| T8 | DATEV-Entry-Change-Tracking | `Model/DatevEntryChangeTracking.cs` | Programmiert, nie debugged; Datenport macht Inspektion möglich |
| T9a | DATEV-Entry-Stacks als Import-/Session-Marker | `Model/DatevEntryStack.cs` + `D2VContext.ApplySessionStack` | Filterzweck (z.B. EB vs. Endjahresbuchungen) |
| T9b | DATEV-Entry-Groups — Stand klären | `Model/DatevEntryGroup.cs` | Stub-Modell ohne Felder, vermutlich nie ausgebaut. Im Datenport read-only inspizieren, dann entscheiden: Inhalt geben oder Tabelle entfernen |
| T10 | Invert/Generalumkehr | `DatevEntry.GetInverted()` | Auch ohne GoBD-Lock nützlich; nicht zwingend gekoppelt |
| T11 | StarMoney-Banken/Konten | `UI/StarMoneyBank/AccountControls/` | Funktional OK |
| T12 | StarMoney-Buchungen Liste/Editor | `UI/StarMoneyEntryControls/` | Funktional OK |
| T13 | StarMoney-Buchungen Import | `Lib/StarMoneyEntryImportExport.cs` | Heute CSV von Banken (kein StarMoney mehr); config-getriebenes Layout-Mapping nötig |
| T14 | StarMoney-BookingTexts Katalog | `UI/StarMoneyBookingTextControls/` | Problematisch — schlechte Defaults; Aufräumen oder ersetzen |
| T15 | StarMoney-CreditCard | Model | Nie funktional |
| T16 | StarMoney↔DATEV-Abgleich | `UI/StarMoneyToDatevCheckControls/`, `Model/DatevToStarMoneySearch*` | Nie funktional |
| T17 | SuSa | `UI/SuSaControls/`, `Model/SuSa*` | Funktional OK |
| **T18** | Jahresabschluss Export + PDF | `UI/JA/`, `D2VPdf/`, `Lib/JahresabschlussHelper.cs` | **Design 1:1 übertragen** |
| T19 | Voucher (Excel-Export pro Buchung — **nicht PDF**) | `DatevEntry.GetVoucherData` | Korrektur ggü. erstem Read: Excel, nicht PDF |
| T20 | YearSelector | `UI/YearSelector.xaml` | Aktiv genutzt; Mandant war nie ein Konzept |
| T21 | DescriptionSplitter (Heuristik aus StarMoney-Beschreibungen) | `Lib/DescriptionSplitter.cs` | Nie richtig gut genutzt |
| T22 | IBAN-Helper | `Lib/IbanHelper.cs` | Utility |
| T23 | CSV-View (generischer XLSX-Loader) | `Lib/CSVView.cs` | Auf viele Weisen ersetzbar |

Open clarifications (kein Backlog-Item, sondern Klärungs-Notiz):
- **DatevEntryGroup**: Zweck unbekannt; Modell ohne eigene Felder; Stand der Tabelle (leer / Beispielzeilen / produktiv genutzt) muss am Datenport evaluiert werden.
- **DatevEntryChangeTracking-Stand**: zu evaluieren, sobald Datenport läuft.

## 11. Post-Port Cleanup (still in Track A, after the data port works)

These are small visual/UX improvements within the data port — not new functionality.

- Enum-FieldType statt Text für `DatevEntry.ValueType`, `DatevCalculationValue.Method`, `DatevAccountEntry.Factor`-Sonderwerte (sobald Trade B's introspection den deklarierten SQL-Typ ausreicht — sonst hardcoded in `columns.json`).
- 14-Description-Felder in `star_money_entry` als zusammengefasste Spalte (Concat in der List-View, getrennt im Editor) — falls die generische Tabelle das ohne Phase-1.5-Komponenten kann; sonst Backlog.
- DE-Labels für alle Spalten polieren.

## 12. Risks

- **Track B might not be ready**: this spec doesn't unblock until DBlicious can read a foreign SQLite as a CRUD source. Mitigation: write the entity definitions now anyway — they're useful as input to Track B's design and require no DBlicious code.
- **D2V column naming**: PascalCase vs DBlicious camelCase. Solved by `dbColumn` override per column (contract item 3.6). If Track B decides on global rename instead, the override field is unused.
- **Editor for composite-PK rows**: DBlicious's editor today assumes a single-id route. Composite PKs need URL-form like `/entities/star_money_account/12345/4711` or a query-param form. Counts as an open design question for Track B.
- **`d2v.db` is the production DB**: data is real, possibly sensitive. The spec assumes a *copy* lives at the path in §5; never point at the original. Add this as a README note in `examples/d2v/`.

## 13. Decisions

1. **Two tracks**: A (this) covers data port; B (separate) covers DB-layer evolution. Function ports are Track C, one spec each.
2. **No domain logic in Track A**: explicitly out of scope, even tempting items like `GenerateAccountEntries`.
3. **`d2v.db` stays as-is**: no migration to a DBlicious-native schema. If Track B requires that, this spec adapts later.
4. **Read-only for non-ACTIVE entities**: don't enable the editor for EVAL/LEGACY-IMPORT/EXPERIMENTAL.
5. **Sandbox-Branch in Nav** for EXPERIMENTAL entities, so the status is visible to the user.
6. **`Companies` stays in**: even though EXPERIMENTAL, the data is there and the user wants to inspect it.

## 14. Acceptance

The data port is "done" when:

- `cargo run -p server -- --data-dir ./examples/d2v` (against a copy of `d2v.db`) boots without error.
- The client at `:8080` shows the navigation in §7.
- All 17 entity lists load real rows from `d2v.db`.
- ACTIVE entities are editable; an edit round-trips back into the DB and is visible after page reload.
- EVAL / LEGACY-IMPORT / EXPERIMENTAL entities show data but reject Update/Delete.
- A composite-PK row (e.g. `star_money_account`) opens its detail editor correctly.
