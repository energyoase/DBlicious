---
id: Q0006
created: 2026-05-21T12:35:00Z
status: done
priority: medium
title: "EntityListPage: H1 zeigt entity_type roh; Spalten-Header zeigen rohe i18n-Keys"
spec: null
plan: null
pending_question_id: null
resume_step: null
parent: null
artifacts: []
source: manual
fingerprint: null
requirements: null
assigned_worker: null
type: bug
review:
  status: null
  reviewer: null
  notes_path: null
  requested_at: null
  decided_at: null
security_review:
  required: false
  status: null
  notes_path: null
diagnosis_path: null
design_path: null
linked_issue: null
linked_pr: null
verification_ops: []
tags:
  - ui
  - i18n
  - table
  - usability
---

## Description

Auf `/entities/<entity_type>` (z.B. `/entities/datev_account`) zeigt die Seite
statt eines übersetzten Titels den **rohen `entity_type`-Slug** als H1
(„datev_account" statt erwartet z.B. „DATEV Kontenplan"). Beim genaueren Hinsehen
zeigen auch die **Spalten-Header der Tabelle** rohe Fluent-Keys
(„field.datev_account.number" statt „Nummer").

Beobachtet im d2v-Example mit der lokalen Smoke-Copy.

## Repro

1. `cargo run -p server -- --data-dir ./examples/d2v` (mit
   `D2V_LEGACY_URL=sqlite:///pfad/zu/d2v-smoke-copy.db`)
2. `cd client && trunk serve`
3. http://127.0.0.1:8080/ öffnen, einloggen, in der Nav z.B. „Kontenplan"
   anklicken → Route `/entities/datev_account`.
4. Erwartet: „DATEV Kontenplan" (o.ä.) als H1, „Nummer / Bezeichnung / …" als Spalten.
5. Tatsächlich: H1 = „datev_account", Spalten = „field.datev_account.number" usw.

## Root-Cause (provisorisch, nicht abschließend)

Zwei Symptome, gemeinsamer Mechanismus:

- **H1:** `client/src/routes/mod.rs` rendert an mehreren Stellen
  (Zeilen 144, 154, 183, 198) `<h1>{entity_type.clone()}</h1>` — direkt der
  Slug, ohne i18n-Lookup und ohne Lookup in den Entity-Settings/Navigation.
- **Spalten-Header:** `client/src/components/table/{table_view,view}.rs` ruft
  `t(&column.label_key)` auf. `t()` (`client/src/i18n/mod.rs:251–268`) liest
  ausschließlich aus den per `include_str!` eingebetteten Fluent-Bundles
  (`client/locales/{de,en}/main.ftl`). Fehlt der Key, fällt es auf
  `key.to_string()` zurück — also der rohe Schlüssel.
- Die echten Übersetzungen liegen aber **server-seitig** in
  `examples/d2v/translatables/{entries,values}.json` (z.B.
  `field.datev_account.number → "Nummer"`, `nav.datev_accounts → "Kontenplan"`)
  und sind über die `translatable`-GraphQL-Query abrufbar — werden vom Client
  aber heute **nicht** in den i18n-Lookup einbezogen.

## Erwartetes Verhalten

H1 und Spalten-Header zeigen die übersetzten Werte in der aktuellen Locale.
Quelle für den H1-Titel ist zu entscheiden (Optionen siehe „Open questions").

## Open questions

1. **Titel-Quelle für H1:** (a) Nav-`labelKey` matchen (Route → Nav-Knoten →
   labelKey), (b) neuer Konventions-Key `entity.<type>.title` als Translatable,
   (c) neues optionales `title` in `entity_settings.json` mit Fallback auf den
   Slug?
2. **Server-Translatables in Client-i18n integrieren:** wie?
   (a) Beim App-Start einmal `translatable`-Query, dort gelieferte Werte als
   zusätzliche Fluent-Bundle-Schicht überlagern, ggf. mit Locale-Switch
   reagieren; (b) Per-Key Fetch im `t()`-Fallback (zu chatty);
   (c) Server-Generator macht Fluent-FTL aus Translatables zur Build-Zeit
   (verliert Live-Editing). — Option (a) ist die natürliche Wahl, weil es zum
   bestehenden `revision`-Subskriptions-Mechanismus passt.
3. Wenn (a) gewählt: Wie behandelt man Translatables, die nach Login geladen
   werden (Security-Gating der `translatable`-Query) vs. die statisch
   eingebetteten Keys (`login.title`, `error.validation`, …)?

## Notes

- Vermutlich derselbe Effekt auch im `shop`-Example, dort fallen die Keys nur
  weniger auf, weil Spaltennamen englisch und kürzer sind.
- Der bestehende `t()`-Reaktivitäts-Pfad über `ctx.revision` ist genau das, was
  ein „dynamisches Bundle nach Login" bräuchte — d.h. die Architektur erlaubt
  die Lösung ohne Bruch.
- Vor dem Spec-Lauf bitte mit User klären, welche Variante der Titel-Quelle
  gewünscht ist.
