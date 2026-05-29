# Q0013 — Minimale livable d2v2019-Teilmenge (gestaffelte DoD)

Date: 2026-05-29
Status: Design — non-interaktiv erstellt (Sub-Agent), awaiting review
Typ: **Definition-of-Done-Design, kein Bauauftrag.**

## 0. Frage & Auftrag

> Welche **genaue Teilmenge** der *funktionierenden* d2v2019-Features in
> `examples/d2v/` ist heute „livable" (nutzbar) — und was ist die gestaffelte
> Definition-of-Done dafür? Explizit OHNE die nie-funktionalen/Stub-Features.

Die Staffelung ist **bereits vom User entschieden** (2026-05-29) und wird hier
nicht neu verhandelt, sondern ausgestaltet:

- **Stufe 1 = read/display/validate** — größtenteils HEUTE machbar.
- **Stufe 2 = schreibend/aggregierend** — später.

Dieses Dokument **distilliert** die Script-First-Gap-Analyse
(`2026-05-24-d2v-script-first-gap-analysis.md`, im Folgenden „die Analyse") in
eine konkrete, gestaffelte DoD. Es leitet die Lücken NICHT neu her — die
Analyse-Abschnitte §4 (Feature-Matrix), §5 (Lücken-Cluster), §6 (script-first-
Kernantwort) und §7 (priorisierte Folge-Lücken) sind die maßgebliche Basis.

**Schwester-Item Q0012** (Example → eigenständiges Projekt / Packaging) ist
ein **eigenes** Item — Distribution/Versionierung wird hier NICHT designt, nur
referenziert. Q0013 ist die **Feature-Teilmenge**, nicht die Distribution.

---

## 1. Verifizierter Ausgangsstand (Repo-Spot-Check 2026-05-29)

Gegen den aktuellen Code/Daten-Stand bestätigt (nicht nur aus der Analyse
übernommen):

| Behauptung der Analyse | Spot-Check-Ergebnis | Beleg |
|---|---|---|
| 17 Entities im Port | ✅ bestätigt | `examples/d2v/entities/` (17 Verzeichnisse) |
| Loader lädt Provider-Scripts aus dem data-dir | ✅ bestätigt | `server/src/example/loader.rs::load_scripts` (Z. 190ff): scannt `scripts/*.rhai` + sucht `<stem>.manifest.{json,toml}` |
| Write-Grenze hält (Script kann nicht schreiben) | ✅ bestätigt | `WriteEntity` ist erst ab `ScriptTier::Developer` (`shared/src/script/capability.rs` Z. 67ff); Provider-Scripts sind Reader/Author; Analyse §3: `db.patch` = `ServerOnlyFunction` |
| ValueType SOLL/HABEN bereits script-first verdrahtet | ✅ **schon live** | `datev_entry/columns.json`: `valueType` ist `directionalEnum` mit `"formatterId": "script:d2v_value_type_label"`; Script + Manifest existieren in `scripts/` |
| FK-Picker fehlt → Konto nur als ID | ✅ bestätigt | `datev_entry/columns.json`: `primaryAccountNr`/`secondaryAccountNr` = `{"kind":"integer"}` (reine ID-Zahl, kein Picker, kein display_template) |
| `DatevAccountType` = Bitflags fehlt → Text-Workaround | ✅ bestätigt | `datev_account/columns.json`: `accountType` = `{"kind":"text"}` |
| `DirectionalEnum` ist Framework-Primitive, Saldo-Vorzeichen erst in Aggregation | ✅ bestätigt | `shared/src/lib.rs`: `DirectionalEnum { values, amount_field }`, Kommentar „Vorzeichen-Anwendung (Saldo) ist Aggregation (Welle 2), hier nur das Modell" |

### 1a. Eine Abweichung, die das Design beeinflusst: **kein IBAN-Feld im Port**

Die Analyse (§4 Zeilen 5/12, §6) nennt einen IBAN-Modulo-97-Validator als
„heute script-first machbar". Der Spot-Check zeigt aber: **keine der 17
Entities hat heute ein IBAN-Feld.** `star_money_account/columns.json` führt
`bankCode`, `code`, `defaultName`, `companyId`, `datevAccountNr`; ein expliziter
IBAN-String existiert weder dort noch in `star_money_entry` (74 Spalten,
durchgesehen — keine IBAN).

Das ist **kein Widerspruch zum Code**, sondern eine Daten-/Config-Lücke: der
Validator-**Algorithmus** ist script-first machbar (reine `ComputeOnly`-
Arithmetik), aber es fehlt eine **Spalte zum Anbinden**. Konsequenz fürs
Design (siehe §3 Pilot P2 und Decisions D4): Der IBAN-Validator ist ein
gültiger Stufe-1-Pilot, aber sein produktiver Einsatz ist auf das Hinzufügen
einer IBAN-Spalte (kleine Config-Aufgabe) ODER auf eine aus `bankCode`+`code`
synthetisierte BBAN-Prüfung angewiesen. Das wird **explizit als Annahme
markiert**, nicht überspielt.

---

## 2. Stufe-1-DoD — „livable jetzt" (read/display/validate)

### 2.1 Entity-Teilmenge (die „livable" zählt)

Reine **Lese-/Anzeige-/Filter-/Validier**-Nutzung über den generischen
`EntityTable` + die existierenden `columns.json`/`settings.json`. Eingeschlossen
sind die fertigen, nicht ausgeklammerten Entities (Ausschlussliste §5):

| Entity | Stufe-1-Nutzung | Status heute |
|---|---|---|
| **datev_account** | Kontenrahmen anzeigen + filtern (Nummer/Name/Typ/Steuersatz) | ✅ listbar; `accountType` als Text (Bitflags-Workaround akzeptiert) |
| **datev_entry** | Buchungen anzeigen + filtern; SOLL/HABEN formatiert; Soll/Haben-Plausibilität validieren | ✅ listbar; SOLL/HABEN-Formatter **live**; Konto nur als ID (U1-gated, s.u.) |
| **datev_entry_stack** | Import-/Edit-Stapel anzeigen; als Filter-Marker über Buchungen | ✅ listbar; Stack-Filter als Script-Pilot (P3) |
| **star_money_bank** | Bank-Stammdaten anzeigen + filtern | ✅ listbar |
| **star_money_account** | Bank-Konto-Stammdaten anzeigen + filtern; (IBAN-Validierung: siehe §1a/§3 P2) | ✅ listbar; kein IBAN-Feld heute |
| **star_money_booking_text** | Buchungstext-Stammdaten anzeigen | ✅ listbar |
| **star_money_entry** | Bank-Buchungen anzeigen + filtern (40+ Spalten flach, Description-Format) | ✅ listbar; Bank/BookingText nur als Code (U1-gated) |
| **susa_entry** | Summen-/Salden-Zeilen **anzeigen** (als gespeicherte Tabelle, read-only) | ✅ listbar als flache Tabelle; **echte** SuSa-Auswertung (Aggregation/Report-View) = Stufe 2 |
| **datev_account_entry** | abgeleitete Sub-Buchungen **anzeigen** (sofern Daten im Port vorhanden) | anzeigbar; **Erzeugen/Persistieren** = Stufe 2 |
| **datev_calculation** / **_entry** / **_value** | Calculation-Stammdaten/Werte **flach anzeigen** | flach listbar; **Baum-View + Aggregation** = Stufe 2 |

**Nicht in Stufe 1, aber auch nicht ausgeklammert** (gehören in Stufe 2,
weil sie schreiben/aggregieren/eine fehlende Control brauchen): SuSa-/
Calculation-**Auswertung** (nicht nur flache Anzeige), GenerateAccountEntries-
Erzeugung, Invert/Storno, jeder Import.

### 2.2 Akzeptanzkriterien „livable" (Stufe 1) — präzise

Ein Nutzer kann **end-to-end** das Folgende ohne neuen Framework-Code tun
(Server mit `--data-dir ./examples/d2v` + Client laufen):

1. **Navigieren & Listen:** Über die Navigation jede der §2.1-Entities öffnen
   und ihre Zeilen im `EntityTable` sehen (Paginierung über `defaultPageSize`).
2. **Sortieren & Filtern:** Pro Spalte mit `sortable`/`filterable=true`
   sortieren/filtern (UI ist verdrahtet; Server-seitige Anwendung — soweit
   schon vorhanden — bzw. der dokumentierte `DataSource`-Pfad).
3. **SOLL/HABEN lesbar:** In `datev_entry` zeigt `valueType` das lokalisierte
   Label (SOLL/HABEN) statt der rohen Zahl — **bereits erfüllt** durch
   `script:d2v_value_type_label`.
4. **Buchungs-Plausibilität:** Eine Validator-Regel (Soll==Haben-Balance bzw.
   Pflichtfeld-Checks) meldet read-time einen Fehler — als Script-Validator
   (Pilot P1, s. §3).
5. **Read-time-Vorschau kleiner Mengen:** Ein Computed-Script liefert je Zeile
   einen abgeleiteten Kleinwert (z.B. Saldo-Vorschau pro Buchung über die
   `directionalEnum`-Vorzeichen) für **kleine** Datenmengen — explizit ohne
   Anspruch auf produktive Aggregation (das ist Stufe 2).
6. **Stack-Fokus:** Buchungen lassen sich auf einen `datev_entry_stack`
   einschränken (Filter-Script P3).

> **„Livable" heißt also:** Stammdaten + Buchungen **durchsehen, sortieren,
> filtern, formatiert lesen und read-time plausibilisieren** — ein
> Buchhalter:in kann den Bestand inspizieren. Es heißt NICHT: neue Buchungen
> anlegen, Sub-Buchungen erzeugen, stornieren, importieren oder produktive
> Auswertungen rechnen (alles Stufe 2).

### 2.3 Der EINE harte Blocker für Stufe 1: **U1 FK-Referenz-Picker**

Die Analyse (§5.1, §7 Punkt 1) benennt **U1** als den einzigen harten Stufe-1-
Blocker. Konkret, am Port verifiziert:

| | OHNE U1 (heute) | MIT U1 |
|---|---|---|
| **datev_entry → Konto** | `primaryAccountNr`/`secondaryAccountNr` zeigen die **rohe Konto-ID-Zahl**; keine Auswahl, kein Name | Picker zeigt Konto-Name + -Nummer (`display_template`), Auswahl per Suche |
| **star_money_entry → Bank/BookingText** | `bankCode`/`bookingTextKey` als **Roh-Code** | Picker zeigt Bank-/Buchungstext-Name |

**Wichtig für die DoD-Abgrenzung:** Stufe 1 ist **bewusst so geschnitten, dass
U1 NICHT erforderlich ist** — die livable-Teilmenge ist *anzeigen/filtern*, und
Konto-Referenzen werden in Stufe 1 als **ID/Code-Text** akzeptiert. U1
verbessert die Anzeige (Name statt ID) und ist **Voraussetzung für jede
Buchungs-Form** (Editieren/Anlegen) — das fällt aber ohnehin in Stufe 2
(schreibend). U1 wird hier deshalb als **„Stufe-1-Komfort / Stufe-2-Pflicht"**
geführt: ohne U1 ist Stufe 1 *livable mit ID-Anzeige*; U1 ist der erste
Baustein, der Stufe 2 entsperrt. **U1 selbst wird in diesem Item NICHT gebaut**
(eigenes Item, vgl. Analyse §7.1).

---

## 3. Heute-machbare Script-Piloten (`examples/d2v/scripts/`)

Kandidaten-Deliverables, die **mit null neuem Framework-Code** anlegbar sind —
nach dem Muster des bereits geladenen `d2v_value_type_label.rhai` (Provider-
Slot `formatter`, Tier `reader`, Capabilities `computeOnly`+`readI18n`,
Manifest neben der `.rhai`-Datei). Alle bleiben innerhalb der Write-Grenze
(Read/Compute/Format/Validate/Filter).

| Pilot | Slot | Bindung | Capabilities | Status / Voraussetzung |
|---|---|---|---|---|
| **(vorhanden) d2v_value_type_label** | Formatter | `datev_entry.valueType.formatterId` | computeOnly, readI18n | ✅ existiert bereits — das **Referenzmuster** |
| **P1 — Soll/Haben-Balance-Validator** | Validator | `datev_entry` (settings → validatorId) | computeOnly (+ ggf. readI18n für Meldung) | heute anlegbar; reine Arithmetik über `value`/`valueType` |
| **P2 — IBAN-Modulo-97-Validator** | Validator | **gated**: braucht ein IBAN-Feld (s. §1a) ODER prüft synthetisierte BBAN aus `bankCode`+`code` | computeOnly | Algorithmus heute machbar; **Bindung erst nach IBAN-Spalte/Annahme A2** |
| **P3 — DatevEntryStack-Filter** | Filter | `datev_entry.stackId` bzw. Listen-Filter | computeOnly | heute anlegbar; einfacher Gleichheits-/Mengen-Filter auf `stackId` |
| **P4 (optional) — Konto-Nummer-Formatter** | Formatter | `datev_account.number` / `datev_entry.primaryAccountNr` | computeOnly | heute anlegbar; reine Darstellung (z.B. Zero-Pad / SKR-Range-Label read-time) |
| **P5 (optional) — Description-Aufbereitung StarMoneyEntry** | Formatter | `star_money_entry.description1..14` Zusammenführung | computeOnly | heute anlegbar; String-Concat read-time |

**Empfohlener Mindest-Pilot-Satz für die Stufe-1-DoD:** P1 + P3 (beide ohne
jede Vorbedingung) plus der bereits vorhandene ValueType-Formatter. P2 nur,
wenn Annahme A2 (IBAN-Spalte) bestätigt wird; P4/P5 sind Komfort.

> Hinweis: Diese Piloten sind **Kandidaten-Deliverables** für ein späteres
> Bau-Item, kein Bauauftrag in Q0013. Q0013 liefert die DoD und die Liste; das
> tatsächliche Anlegen der `.rhai`+Manifest-Dateien ist ein kleiner
> Folgeschritt (kein neuer Framework-Code, daher niedrigschwellig).

---

## 4. Stufe-2-Inventar (später) — priorisierter Backlog, KEIN Build-Order

Die schreibenden/aggregierenden Features mit ihrer jeweiligen Lücke aus
Analyse §5/§7. Sortiert nach Hebelwirkung (= Analyse §7). **Dies ist ein
Backlog zum Einordnen, nicht zum jetzt-Bauen** — jedes U#/F#/1.7.x ist ein
eigenes Item.

| Stufe-2-Feature | Was es braucht (Lücke) | Analyse-Ref | Größe |
|---|---|---|---|
| **Konto-/Bank-Auswahl in Formularen** (Voraussetzung für alles Schreibende) | **U1 FK-Referenz-Picker** | §5.1, §7.1 | M |
| **Effiziente SuSa-/Calculation-/Saldo-Auswertung** | **Aggregation-Queries (1.7.12)** — statt read-time-N+1-Script | §5.2, §7.2 | M |
| **Auswertungs-Sichten** (DatevAccountView, SuSaView, Calculation-Baum-Layout) | **U3 Report-View** (Cross-Table/Tree/Pivot) | §5.1, §7.3 | L |
| **Globaler Jahres-Filter** | **U5 App-Context / YearSelector** | §5.1, §7.4 | S–M |
| **DatevCalculation-Baum-Darstellung** | **`FieldType::Tree` (1.7.20)** | §5.2, §7.5 | M |
| **GenerateAccountEntries → persistierte Sub-Buchungen** | **U2 Save-Report** (Diff-Vorschau) + **U6/F1** (Persistier-Operation) + WriteEntity-Server-Pfad | §4 #8, §5.1/§5.3, §7.6 | M–L |
| **Invert / Generalumkehr (Storno-Buchung schreiben)** | **U6 Long-Running-Action / F1** + Write-Pfad | §4 #13, §5.3, §7.6 | M |
| **DATEV-/Bank-CSV-Import** | **Bulk-Import (1.7.15)** + Bank-CSV-Mapping | §5.3, §7.7 | M–L |
| **Jahresabschluss-PDF / Voucher-XLSX** (Binär) | Server-Backends 1.7.9/1.7.14 (✅ vorhanden) + Template/Resolver-Anbindung; Script liefert nur Daten read-time | §4 #9/#10, §5.3 | (Folge) |

Niedrig-Prioritäts-Komfort (Analyse §7-Schluss): `FieldType::Bitflags` (G3) für
`DatevAccountType` und `FieldType::Iban` — beide durch IntEnum-Text-Workaround
bzw. Script-Validator provisorisch gedeckt; **nicht** Stufe-1-relevant.

---

## 5. Explizite Ausschlussliste (broken/Stub/obsolet)

Diese Features zählen **nicht** zur livable-Teilmenge — weder Stufe 1 noch
Stufe 2 (laut Glossary & Analyse §0). Je eine Zeile Begründung:

| Ausgeschlossen | Begründung (eine Zeile) |
|---|---|
| **Company** | nie funktional / obsolet — kein nutzbares Verhalten im Alt-System |
| **DatevEntryGroup** | Stub — Gruppierungs-Mechanik nie fertiggestellt |
| **DatevEntryChangeTracking** | Stub — Änderungs-Tracking nie funktional |
| **StarMoney↔DATEV-Reconciliation** | nie funktional — Abgleich-Logik nicht produktiv |
| **StarMoneyCreditCard** (+ _entry) | obsolet — Kreditkarten-Zweig nicht genutzt |
| **DescriptionSplitter** | Stub — Beschreibungs-Splitter nie fertig |
| **StarMoney-Import** | obsolet — Importer veraltet (Bank-CSV stattdessen, = Bulk-Import 1.7.15, Stufe 2) |

> Hinweis: Die **Daten** mancher ausgeschlossener Tabellen sind im Port als
> Verzeichnis vorhanden (`company`, `datev_entry_group`,
> `datev_entry_change_tracking`, `star_money_credit_card`,
> `star_money_credit_card_entry`) — sie bleiben **read-only / unbeworben** und
> sind NICHT Teil der livable-DoD.

---

## 6. Decisions

- **D1 — Staffelung wie vom User vorgegeben.** Stufe 1 = read/display/validate;
  Stufe 2 = schreibend/aggregierend. Nicht neu verhandelt; dieses Dokument
  schärft nur den Schnitt.
- **D2 — Stufe-1-Schnitt ist U1-frei.** Die livable-Teilmenge ist bewusst so
  geschnitten, dass sie OHNE FK-Picker funktioniert (Konto/Bank als ID/Code-
  Text). U1 ist „Komfort für Stufe 1 / Pflicht für Stufe 2" und der erste
  Stufe-2-Baustein — wird in Q0013 nicht gebaut.
- **D3 — Write-Grenze ist die Trennlinie zwischen Stufe 1 und Stufe 2.**
  Read/Compute/Format/Validate/Filter/UI-Emit = Stufe 1 (script-first im
  example dir, verifiziert möglich); jede Mutation = Stufe 2 (Server).
- **D4 — IBAN-Validator ist Pilot, aber bindungs-gated.** Algorithmus heute
  script-first machbar; produktiver Einsatz braucht eine IBAN-Spalte (heute
  nicht im Port) oder eine BBAN-Synthese aus `bankCode`+`code`. Als Annahme
  markiert (A2), nicht als erfüllt angenommen.
- **D5 — Mindest-Pilot-Satz = P1 (Balance-Validator) + P3 (Stack-Filter) +
  vorhandener ValueType-Formatter.** P2/P4/P5 sind bedingt/Komfort.
- **D6 — SuSa/Calculation in Stufe 1 nur als flache Anzeige.** Echte
  Auswertung (Summen, Baum) ist aggregations-/Report-View-abhängig → Stufe 2.
- **D7 — Abgrenzung zu Q0012.** Q0013 = Feature-Teilmenge/DoD; Q0012 =
  Packaging/Distribution. Keine Distribution hier.
- **D8 — Bookkeeping-Stdlib (Analyse §4b/Decision 7) ist hier NICHT designt.**
  Out of scope laut Auftrag; nur als 4-Schichten-Kontext referenziert.

## 7. Offene Fragen / Annahmen

(Non-interaktiv erstellt — folgende Punkte wären normalerweise Rückfragen;
hier als begründete Best-Guesses festgehalten.)

- **A1 — Server-seitige Sort/Filter-Anwendung.** Annahme: Für die Stufe-1-DoD
  reicht es, dass die Sort/Filter-**UI** verdrahtet ist und der dokumentierte
  `DataSource`-Pfad (Remote/Local) die Anwendung trägt; ob der Server die Args
  heute schon vollständig anwendet, ist für „livable = durchsehen/filtern"
  unkritisch (CLAUDE.md vermerkt: Server ignoriert Sort/Filter-Args teils).
  **Falls** produktives Filtern über große Mengen gefordert ist, rückt das
  Richtung Stufe-2-Aggregation. *Best-Guess: Stufe-1-DoD = UI-verdrahtetes
  Filtern/Sortieren auf der geladenen Seite, kein Anspruch auf Server-Pushdown.*
- **A2 — IBAN-Feld.** Annahme: Im Port existiert heute **kein** IBAN-Feld
  (verifiziert, §1a). Best-Guess: Der IBAN-Validator-Pilot (P2) ist optional
  und wird erst scharf, wenn (a) eine IBAN-Spalte in `star_money_account`
  ergänzt wird (kleine Config-Aufgabe) oder (b) der Validator eine aus
  `bankCode`+`code` synthetisierte deutsche BBAN prüft. **Nicht** als Stufe-1-
  Pflicht gewertet.
- **A3 — Validator-Slot-Verdrahtung in settings.json.** Annahme: Validator-
  Scripts binden analog zum Formatter über eine `validatorId`/Provider-Lookup-
  Mechanik (Phase-1.5-Resolution-Kette der Analyse). Spot-Check zeigt den
  Formatter-Pfad (`formatterId`) live; der Validator-Pfad wird als analog
  angenommen. **Falls** die settings-Schlüssel anders heißen, ist das eine
  reine Pilot-Detail-Frage, kein DoD-Blocker.
- **A4 — „kleine Mengen" für read-time Computed.** Keine harte Zahl vorgegeben.
  Best-Guess: read-time-Computed-Vorschau ist für die *aktuell geladene Seite*
  (`defaultPageSize`, z.B. 100 Zeilen) gedacht; alles, was über die ganze
  Buchungsmenge summiert, ist Stufe-2-Aggregation (1.7.12). Die genaue Grenze
  ist eine Performance-Frage, die der Aggregations-Folge-Spec gehört.
- **A5 — datev_account_entry-Daten im Port.** Annahme: Sofern abgeleitete Sub-
  Buchungen bereits im Quell-`d2v.db` liegen, sind sie in Stufe 1 anzeigbar;
  ihre **Erzeugung** bleibt Stufe 2 (GenerateAccountEntries). Ob der Port-
  Datensatz solche Zeilen enthält, hängt an der jeweiligen `d2v.db`-Kopie
  (nicht im Repo) — daher als Annahme geführt.

## 8. Referenzen

- `docs/superpowers/specs/2026-05-24-d2v-script-first-gap-analysis.md` — **primär** (§4 Matrix, §5 Lücken, §6 Kernantwort, §7 Folge-Lücken, §4b 4-Schichten).
- `docs/superpowers/specs/2026-05-20-d2v-on-dblicious-gap-analysis.md` — ältere Gesamt-Analyse (U1–U7, F1/F2, T1–T23).
- `docs/queue/Q0012-d2v-example-zu-eigenstaendigem-projekt.md` — Schwester-Item Packaging/Distribution (abgegrenzt, hier nicht designt).
- `examples/d2v/entities/*/{columns,settings,editor,binding}.json` — Daten-Port-Layout (Spot-Check-Quelle).
- `examples/d2v/scripts/d2v_value_type_label.{rhai,manifest.json}` — Referenzmuster für Provider-Scripts.
- `server/src/example/loader.rs::load_scripts` — verifizierter Script-Lade-Pfad.
- `shared/src/script/{capability,model}.rs` — Provider-Slots + Capabilities + Write-Grenze (`WriteEntity` ab Developer-Tier).
- `shared/src/lib.rs` — `FieldType::DirectionalEnum` (SOLL/HABEN-Primitive).
