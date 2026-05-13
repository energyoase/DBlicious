# ADR-0006: ERP-Plattform-Bausteine als eigene Phase 1.7 (nicht in Phase 2)

- **Status**: Accepted
- **Datum**: 2026-05-13
- **Beteiligt**: solo

## Kontext

Beim Abgleich der bisherigen Roadmap gegen die Anforderungen einer realen ERP-Anwendung (Rechnungsstellung in DE/EU, Belegverwaltung, Workflows, Reporting, Compliance) fielen ~21 Plattform-Features auf, die in keiner Phase vorgesehen waren:

- Gapless Number-Sequences (gesetzliche Pflicht, GoBD §146 AO)
- Period-Locks und GoBD-Unveraenderbarkeit
- File-Storage, PDF-Generierung, Email-Versand, digitale Signaturen
- State-Machine-Engine und Approval-Workflows
- Background-Job-Scheduler
- Aggregation-Queries, Volltextsuche, Bulk-Import/Export
- DSGVO-Tooling, Encryption-at-Rest, Long-Term-Archive
- Webhooks, Hierarchien, OpenAPI/REST-Adapter

Die Frage: gehoeren diese Features in **Plugins** (Phase 2) oder in die **Plattform**?

## Entscheidung

Phase 1.7 "ERP-Plattform-Bausteine" wird **vor Phase 2** eingezogen und enthaelt diese 21 Features als Plattform-Schicht. Plugins (Phase 2) konsumieren sie als Host-Functions.

Begruendung pro Feature-Klasse:

- **Gapless-Sequences** (1.7.1): kann nicht als Plugin abgebildet werden — braucht Framework-Transaction-Boundary und Concurrency-Lock. Ein Plugin haette keinen Zugriff auf die GraphQL-Mutation-Transaction.
- **GoBD-Append-Only** (1.7.4): muss vom CRUD-Resolver enforced werden, nicht vom Plugin (Plugin koennte umgangen werden).
- **File-Storage** (1.7.8): braucht serverseitigen Pluggable-Backend (S3/FS), Plugin haette nur `host.http.fetch`.
- **Period-Locks** (1.7.3): Framework-Resolver muss pruefen, bevor Plugin-Trigger feuert.
- **State-Machine** (1.7.5): Permission-Gates auf Transitionen sind Phase-0.7-Konzept; Plugin koennte umgangen werden.
- **Job-Scheduler** (1.7.7): Plattform-Komponente; Plugin haette keinen eigenen Event-Loop.
- **Aggregations** (1.7.12) + **FTS** (1.7.13): DB-Layer-Features, brauchen direkten SQL-Zugriff.
- **DSGVO-Tooling** (1.7.16): Subject-Daten-Export ueber alle Tabellen — braucht Framework-Sicht.

Andere Features (PDF-Generierung, Email-Versand) **koennten** als Plugin abgebildet werden — werden aber als Plattform-Service implementiert, damit (a) ein ERP-Bauer sie nicht doppelt schreibt, (b) eine konsistente Audit-Spur entsteht und (c) Plugins sie als Host-Function aufrufen koennen.

## Alternativen

- **Alles als Plugin in Phase 2**. Verworfen: die transaktionssicheren Features (Sequences, Period-Locks, GoBD-Append-Only) sind im Plugin-Modell nicht abbildbar. Plugin-Autoren wuerden Workarounds bauen, die unter Last brechen.
- **Verteilung der Features in bestehende Phasen** (0.7, 1, 2, 3). Verworfen: das ERP-Narrativ geht verloren; einzelne Phasen explodieren in der Groesse; Dependency-Reihenfolge wird unklar.
- **Nur kritische Features (1.7.1, 1.7.4, 1.7.8, 1.7.9, 1.7.10, 1.7.16) in Phase 1.7, Rest als Backlog**. Verworfen nach Diskussion: User will einen zusammenhaengenden Block — die kritischen Features sind als MVP-Pfad in Phase 1.7 markiert, der Rest bleibt sichtbar in derselben Phase.

## Konsequenzen

**Positive**

- ERP-Bauer hat eine klare Plattform-Schicht: Sequences, GoBD, File-Storage, PDF, Email, … aus einer Hand.
- Plugins werden duenner, weil sie nicht die Plattform-Basics nachbauen muessen.
- Eine Demo-ERP-Anwendung wird nach 1.7-MVP realistisch.
- Compliance (DE/EU-Recht) ist klar im Scope und nicht "Plugin-Autor-Problem".

**Negative**

- Phase 1.7 ist gross: 21 Arbeitspakete in sechs Sub-Phasen A–F. Mitigation: MVP-Pfad (A + C + 1.7.16) klar markiert; Rest schrittweise.
- Plugin-Autoren bekommen mehr Host-Functions zum Lernen — aber das ist besser als 21 unterschiedliche Plugin-Workarounds.
- Phase 2 (Plugins) ist nun spaeter im Zeitstrahl. Vertretbar, weil Plugins ohne 1.7 fragmentaer geblieben waeren.

**Architektonische Folgen**

- Architektur-Vertrag (Plugin) wird in Phase 2 um die 1.7-Host-Functions ergaenzt: `host.fx.convert`, `host.storage.*`, `host.email.send`, `host.sequence.next`, …
- Die Job-Scheduler-Mechanik (1.7.7) ueberlappt mit Plugin-async-Triggers (`afterSave async`). Klare Abgrenzung in den Verträgen: Scheduler = zeit-getriggert (cron), Plugin-Triggers = event-getriggert (CRUD).
- VISION-Phasenplan und ROADMAP-Erfolgskriterien werden um Phase 1.7 ergaenzt.

## Referenzen

- `ROADMAP.md` Phase 1.7 "ERP-Plattform-Bausteine"
- `ROADMAP.md` Risk-Register R-18 bis R-20
- `VISION.md` Phasenplan-Tabelle, Phase-1.7-Zeile
- [ADR-0001](./0001-codegen-not-runtime.md) — Codegen statt Runtime-Engine (1.7-Features sind Plattform-Bausteine, kein Runtime-Konfig)
- [ADR-0003](./0003-server-as-authority.md) — Server-as-Authority (1.7-Features sind serverseitige Pflicht, nicht Client-Side)
