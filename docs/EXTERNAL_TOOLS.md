# EXTERNAL_TOOLS — Externe Bibliotheken und Werkzeuge

Mapping der geplanten Features (siehe `ROADMAP.md`) auf existierende Crates und externe Services, **um nicht das Rad neu zu erfinden**. Stand: Mai 2026.

## Lesehinweise

- **Status-Legenda**:
  - ✅ *Production-ready* — stabile API, gepflegt, in produktivem Einsatz beobachtbar
  - 🟡 *Usable mit Vorbehalten* — funktioniert, aber API-Drift oder Lücken
  - 🔴 *Experimentell/alpha* — vor Einsatz Code-Review noetig
  - 🌐 *Service statt Rust-Lib* — kein reifer Rust-Port verfuegbar, externe Komponente integrieren
- **Philosophie**: Rust-Crates bevorzugen, wo sie produktionsreif sind; bei Rust-Lücken (z.B. SOAP, e-Invoicing-XML-Validatoren) lieber ein externes Service/CLI integrieren als unreif selbst bauen.
- **Lizenzen**: dblicious-Code ist MIT/Apache-2.0. Crates mit GPL/AGPL sind nicht als Library brauchbar, koennen aber als Service-Sidecar laufen.
- **Pflege-Hinweis**: Dieses Dokument **veraltet schnell** — pro Phase-Implementation den jeweiligen Abschnitt verifizieren.

---

## Phase 0.7 — Auth- & Permission-Modell

| Capability | Crate / Tool | Status | Fit | Hinweise |
|---|---|---|---|---|
| Permission-Modell shared types | `serde`, custom | ✅ | A | DIY in `shared/src/auth.rs` ist richtig — keine fertige Crate trifft das spezifische Modell |
| Permission-Resolver | DIY | ✅ | A | Casbin-rs als Alternative, aber Glob/RBAC/Vererbung sind hand-rollable |
| Audit-Log Hash-Chain | `sha2` + DIY | ✅ | A | Fuer SoX-Pflicht (tamper-evident) reicht eine SHA-256-Chain mit Vorgaenger-Hash |
| LDAP/AD (fuer 0.7 noch nicht aktiv, aber sobald relevant) | [`ldap3`](https://crates.io/crates/ldap3) | ✅ | B | Tokio-basiert, produktionsreif, async API |

Casbin-rs ([`casbin`](https://crates.io/crates/casbin)) ist als generische Policy-Engine eine Option, wenn man auf ein etabliertes Modell aufsetzen will (RBAC/ABAC mit Casbin-DSL). Trade-Off: weniger Maßschneider-Kontrolle als DIY, dafuer ausgereiftes Modell.

---

## Phase 1.7 — ERP-Plattform-Bausteine

### A — Buchhaltung & Finanzen

| Capability | Tool | Status | Hinweise |
|---|---|---|---|
| Gapless Number-Sequence | DIY mit `sqlx`/SeaORM-Transaction | ✅ | Kein passender Crate — Concurrency-Garantie ist DB-spezifisch. PostgreSQL: `SELECT ... FOR UPDATE`; SQLite: BEGIN IMMEDIATE |
| FX-Raten — Datenfeed | [ECB Daily Reference Rates](https://www.ecb.europa.eu/stats/policy_and_exchange_rates/euro_reference_exchange_rates/html/index.en.html), [frankfurter.app](https://frankfurter.dev/) | ✅ | ECB ist gratis, Open Data, kein API-Key noetig. Frankfurter wrapped es nutzerfreundlich. |
| FX-Conversion-Helper | DIY | ✅ | Mit `rust_decimal` fuer Banker-Rounding |
| Money-Type | [`rust_decimal`](https://crates.io/crates/rust_decimal) | ✅ | Exakte Dezimal-Arithmetik. NICHT `f64` fuer Geld nutzen. |
| Period-Locks | DIY in Resolver | ✅ | Tabelle + Resolver-Check, kein fertiger Baustein |
| GoBD-Append-Only | DIY | ✅ | Setting auf Entity-Typ + Resolver-Block. Audit-Hash-Chain absichern. |

### B — Workflow & Prozesse

| Capability | Tool | Status | Hinweise |
|---|---|---|---|
| State-Machine | [`statig`](https://crates.io/crates/statig), [`rust-fsm`](https://crates.io/crates/rust-fsm) | 🟡 | Beide gepflegt; `statig` ist HSM-faehig. Fuer Persistenz + Permissions DIY drumherum bauen |
| Approval-Workflow | DIY auf State-Machine | ✅ | Kein Crate macht Multi-Stage-Approvals plus dblicious-Permissions |
| Background-Jobs | [`apalis`](https://github.com/geofmureithi/apalis) | ✅ | Backends fuer Redis/SQLite/Postgres/MySQL/AMQP; Tokio-nativ; monitoring + retries integriert |
| Cron-Scheduler | [`tokio-cron-scheduler`](https://crates.io/crates/tokio-cron-scheduler) | ✅ | Speziell fuer cron-Style; persistierbar ueber Postgres/Nats. Kombinieren mit apalis fuer Event-Triggers |

**Empfehlung Jobs**: `apalis` als Haupt-Job-Engine (kann Event + Cron + Persistenz), `tokio-cron-scheduler` ist die einfachere Alternative wenn man nur cron braucht.

### C — Dokumente & Kommunikation

| Capability | Tool | Status | Hinweise |
|---|---|---|---|
| PDF-Generierung | [Typst](https://typst.app/) via [`typst-pdf`](https://crates.io/crates/typst-pdf), [`papermake`](https://crates.io/crates/papermake) | ✅ | Typst 0.14 (2025) hat Accessibility built-in. Single ~40MB-Binary, containerfreundlich. Massiv schneller als LaTeX. |
| PDF Templates fuer Rechnungen | [`typst-business-templates`](https://crates.io/crates/typst-business-templates) | 🟡 | Spezifisch fuer Rechnungen/Angebote/Vertraege als CLI + Rust-Lib. Embedded Templates, Fonts, i18n |
| PDF-Manipulation (Sign, Merge) | [`lopdf`](https://crates.io/crates/lopdf), [`pdf-writer`](https://crates.io/crates/pdf-writer) | ✅ | Fuer Post-Processing (Anhaengen, Signieren) |
| Email-Versand | [`lettre`](https://crates.io/crates/lettre) | ✅ | De-facto-Standard; SMTP, sendmail, file transport; async-tokio |
| File-Storage-Abstraction | [`object_store`](https://crates.io/crates/object_store) (Apache Arrow) ODER [`opendal`](https://crates.io/crates/opendal) | ✅ | object_store: minimaler, Apache-getragen, S3/Azure/GCP/local; OpenDAL: viel mehr Backends (50+), aber groesser. Fuer dblicious vermutlich `object_store`. |
| File-Storage Backend S3 | [`aws-sdk-s3`](https://crates.io/crates/aws-sdk-s3) | ✅ | Direkt, wenn nur S3 noetig |
| Digitale Signatur (X.509, eIDAS) | [`rcgen`](https://crates.io/crates/rcgen) (Key-Erzeugung) + [`ring`](https://crates.io/crates/ring) / [`rustls`](https://crates.io/crates/rustls) + [`xmlsec`](https://crates.io/crates/xmlsec) fuer XAdES | 🟡 | Rust-Eco hat keine fertige XAdES-/CAdES-Library. Fuer XRechnung-Signing: externer Service oder Java-CLI (Open eSignature Validator / DSS) |
| OCR | [`leptess`](https://crates.io/crates/leptess) (Tesseract-Binding) | 🟡 | Funktioniert, aber Binding zu C-Lib. Alternative: externer Service (AWS Textract, Azure Document Intelligence) |

**eIDAS-Signing-Empfehlung**: fuer rechtskonforme XAdES/PAdES heute eher [DSS](https://github.com/esig/dss) als Java-Sidecar einbinden statt selbst bauen. Pure-Rust-Implementierung ist nicht da. 🌐

### D — Reporting & Search

| Capability | Tool | Status | Hinweise |
|---|---|---|---|
| Aggregation-Queries | nativ SQL via SeaORM, optional DuckDB | ✅ | SeaORM/SQLite reicht fuer einfache GROUP BY. Fuer OLAP siehe Phase 6 |
| Embedded OLAP | [`duckdb-rs`](https://crates.io/crates/duckdb) | ✅ | DuckDB als embedded analytical DB, kann SQLite-Files lesen. "SQLite fuer Analytics". Streaming-Engine, spill-to-disk |
| DataFrame-Manipulation | [`polars`](https://crates.io/crates/polars) | ✅ | Rust-native columnar DataFrame. Komplementaer zu DuckDB: Polars fuer In-Memory-Transformationen, DuckDB fuer SQL-Joins |
| Volltextsuche | SQLite FTS5 nativ ODER [`tantivy`](https://crates.io/crates/tantivy) | ✅ | FTS5 reicht fuer dblicious-Skala; Tantivy als skalierbarer Lucene-Pendant ab Phase 6 |
| Hosted Search (managed) | [Meilisearch](https://www.meilisearch.com/) als Sidecar | ✅🌐 | Wenn man eine externe Search-Engine bevorzugt — pure-Rust geschrieben |
| CSV | [`csv`](https://crates.io/crates/csv) | ✅ | Standard |
| Excel Read | [`calamine`](https://crates.io/crates/calamine) | ✅ | Read-only xlsx/xls/ods |
| Excel Write | [`rust_xlsxwriter`](https://crates.io/crates/rust_xlsxwriter) | ✅ | Modern, MIT, gepflegt |

### E — Compliance & Recht

| Capability | Tool | Status | Hinweise |
|---|---|---|---|
| DSGVO-Tooling | DIY | ✅ | Subject-Export = GraphQL-Aggregation + ZIP-Schreiber ([`zip`](https://crates.io/crates/zip)) |
| Encryption-at-Rest (DB-Level) | [`rusqlite`](https://crates.io/crates/rusqlite) mit SQLCipher-Feature ODER [`libsql`](https://crates.io/crates/libsql) | 🟡 | SQLCipher fuer Rust funktioniert, braucht aber C-Library. Build-Komplexitaet. |
| Field-Level-Encryption | [`aes-gcm`](https://crates.io/crates/aes-gcm), [`chacha20poly1305`](https://crates.io/crates/chacha20poly1305) | ✅ | RustCrypto-Standard |
| Key-Management | [`zeroize`](https://crates.io/crates/zeroize), externes KMS (Vault, AWS KMS) | ✅🌐 | Keys nicht in DB persistieren — externes KMS oder env-vars |
| PDF/A Conversion | [Ghostscript](https://www.ghostscript.com/) als Sidecar | 🌐 | Pure-Rust kann es nicht; Ghostscript ist Goldstandard fuer PDF/A-2b/3b-Konvertierung |
| WORM Storage | S3 Object Lock, MinIO Object Lock | ✅🌐 | Cloud-Feature, kein Rust-Code noetig — Konfiguration im Backend |

### F — Integration

| Capability | Tool | Status | Hinweise |
|---|---|---|---|
| Webhooks-Sender | DIY mit [`reqwest`](https://crates.io/crates/reqwest) + [`hmac`](https://crates.io/crates/hmac) | ✅ | Signing mit HMAC-SHA-256, Retry-Logik selbst |
| Hierarchien — recursive Queries | SeaORM custom raw SQL (CTE) | ✅ | `WITH RECURSIVE` ist Standard-SQL |
| OpenAPI-Generierung | [`utoipa`](https://crates.io/crates/utoipa) ODER [`aide`](https://crates.io/crates/aide) | ✅ | Beide gepflegt; utoipa nutzt Derive-Macros (kompakt), aide ist axum-spezifisch (integriert besser) |
| REST-Adapter | axum directly | ✅ | Schon im Stack |

---

## Phase 2 — WASM-Plugin-Sandbox

| Capability | Tool | Status | Hinweise |
|---|---|---|---|
| WASM Plugin Host | [`extism`](https://extism.org/) | ✅ | Goldstandard fuer Manifest-basierte Plugin-Sandboxes mit Multi-Language-PDKs (Rust/TS/Python/Go/Haskell) |
| Manifest-Parsing | [`toml`](https://crates.io/crates/toml) + serde | ✅ | Standard |
| HTTP-Host-Function Glob-Check | [`globset`](https://crates.io/crates/globset) | ✅ | Selbe Crate wie fuer Permission-Wildcards |
| Plugin-Signing (Public-Key) | [`ed25519-dalek`](https://crates.io/crates/ed25519-dalek) | ✅ | RustCrypto-Standard |
| Plugin-Versioning (SemVer) | [`semver`](https://crates.io/crates/semver) | ✅ | Standard, fuer Compatibility-Range-Check |

---

## Phase 3 — AI-Schema-Engine & Migrationen

| Capability | Tool | Status | Hinweise |
|---|---|---|---|
| LLM-Client Anthropic | [`anthropic`](https://crates.io/crates/anthropic) ODER [`async-anthropic`](https://crates.io/crates/async-anthropic) | 🟡 | Mehrere Optionen — Anthropic SDK noch wenig "official Rust". Prompt-Caching beachten |
| LLM-Client OpenAI | [`async-openai`](https://crates.io/crates/async-openai) | ✅ | Aktiv gepflegt |
| LLM-Client multi-provider | [`rig`](https://crates.io/crates/rig) | 🟡 | Provider-Abstraktion, junger Code |
| Schema-Diff | [`rusty_schema_diff`](https://crates.io/crates/rusty-schema-diff) | 🔴 | Kleines Crate, nicht aktiv gepflegt — eventuell DIY |
| JSON-Schema-Validation | [`jsonschema`](https://crates.io/crates/jsonschema) | ✅ | Fuer MigrationProposal-Validierung |
| SQLite-Backup (Snapshot) | nativer `VACUUM INTO` oder [`rusqlite::backup`](https://docs.rs/rusqlite/latest/rusqlite/backup/index.html) | ✅ | API-call, kein extra Crate |

---

## Phase 4 — Codegen

| Capability | Tool | Status | Hinweise |
|---|---|---|---|
| Token-Stream / AST | [`quote`](https://crates.io/crates/quote), [`syn`](https://crates.io/crates/syn) | ✅ | Standard fuer Macro/Codegen |
| Code-Formatter | [`prettyplease`](https://crates.io/crates/prettyplease) | ✅ | Drop-in fuer rustfmt-Output |
| Cargo-Workspace-Scaffold | [`cargo-scaffold`](https://crates.io/crates/cargo-scaffold) | 🟡 | Alternativ Eigenbau ueber Handlebars-Templates |
| WASI-NN Runtime | [WasmEdge](https://wasmedge.org/) als Plugin-Engine-Alternative | 🌐 | Fuer ML-Inference im Plugin ab Phase 4. Wechsel weg von wasmtime/Extism noetig |

---

## Phase 5 — Enterprise Identity & Compliance

| Capability | Tool | Status | Hinweise |
|---|---|---|---|
| SAML 2.0 | [`samael`](https://github.com/njaremko/samael) v0.0.20 (2026-03) | ✅ | Aktiv gepflegt, "xmlsec"-Feature fuer Signing-Validierung |
| OAuth 2.0 Client | [`oauth2`](https://crates.io/crates/oauth2) | ✅ | Standard |
| OIDC Client | [`openidconnect`](https://crates.io/crates/openidconnect) | ✅ | OIDC-RP Standard, weit verbreitet |
| OIDC Provider/Server | [Keycloak](https://www.keycloak.org/) oder [Authelia](https://www.authelia.com/) als Sidecar | ✅🌐 | Eigener Rust-OIDC-Server existiert nicht produktiv. Externes IdP nutzen |
| SCIM 2.0 Server | [`scim-server`](https://github.com/pukeko37/scim-server) (pukeko37) | 🟡 | Modern, type-safe, ETag, multi-tenant. API noch im Wachstum (~v0.4 in 2025) |
| SCIM 2.0 Protokoll-Typen | [`scim_v2`](https://docs.rs/scim_v2) ODER [`scim_proto`](https://lib.rs/crates/scim_proto) (kanidm) | ✅ | Nur Typen, kein Server-Skelett |
| TOTP | [`totp-rs`](https://crates.io/crates/totp-rs) | ✅ | RFC 6238, mit Steam/Google-Compat-Modi |
| WebAuthn/FIDO2 (Server) | [`webauthn-rs`](https://github.com/kanidm/webauthn-rs) (kanidm) | ✅ | Security-Audit von SUSE bestanden, der De-facto-Standard |
| Passkey-Library | [`passkey-rs`](https://github.com/1Password/passkey-rs) (1Password) | ✅ | Modernerer Stack inkl. CTAP2, von 1Password gepflegt |
| LDAP-Client | [`ldap3`](https://crates.io/crates/ldap3) | ✅ | Tokio-async, weit eingesetzt |
| Risk-based Auth (Geo-IP + Heuristik) | DIY mit [`maxminddb`](https://crates.io/crates/maxminddb) | ✅ | MaxMind GeoIP-Datenbanken, Heuristik selbst |
| Compliance-Framework-Datenmodell (SoX, ISO 27001) | [OpenSCAP](https://www.open-scap.org/) Definitionen | 🌐 | Compliance-Controls als Datenstandard, keine Rust-Lib |

**Empfehlung Identity-Federation**: in einem ersten Schritt OIDC + LDAP integrieren (deckt 80 % der Enterprise-IdPs ab: Okta, Azure AD, Authelia, Keycloak). SAML kommt fuer Legacy-Enterprise dazu, dann via `samael`.

---

## Phase 6 — Skalierung & High Availability

| Capability | Tool | Status | Hinweise |
|---|---|---|---|
| PostgreSQL als Production-Backend | SeaORM (vorhanden) + [`sqlx`](https://crates.io/crates/sqlx)-Postgres-Treiber | ✅ | SeaORM unterstuetzt Postgres direkt — Backend-Wahl per Config |
| Distributed Cache | [`redis`](https://crates.io/crates/redis) (Rust-RS) | ✅ | Production-stable, tokio-async |
| In-Memory Column Store | [`duckdb-rs`](https://crates.io/crates/duckdb) | ✅ | Eingebettet, kein extra Server |
| OpenTelemetry | [`opentelemetry`](https://crates.io/crates/opentelemetry) + [`tracing-opentelemetry`](https://crates.io/crates/tracing-opentelemetry) | ✅ | Standard, exports an Tempo/Jaeger/Honeycomb |
| Prometheus-Metrics | [`prometheus`](https://crates.io/crates/prometheus) ODER [`metrics`](https://crates.io/crates/metrics) | ✅ | metrics-Crate als framework-agnostische Facade |
| Health-Check-Pattern | DIY mit axum-Route | ✅ | kein Crate noetig |
| Backup-Tooling Postgres | [`pgbackrest`](https://pgbackrest.org/), [`wal-g`](https://github.com/wal-g/wal-g) | ✅🌐 | Externe CLI-Tools, kein Rust-Code noetig |

---

## Phase 7 — Process Orchestration & EDA

| Capability | Tool | Status | Hinweise |
|---|---|---|---|
| BPMN-Engine (Rust-native) | [`bpm-engine`](https://github.com/Colin4k1024/bpm-engine) | 🟡 | Aktiv entwickelt; BPMN 2.0 XML-Parser + Compiler; in-memory + Postgres-Backend. Reife noch unklar |
| BPMN-Engine (Service) | [Camunda 8 / Zeebe](https://camunda.com/), [Operaton](https://operaton.org/), [CIB seven](https://cibseven.org/) | ✅🌐 | Java-based, BPMN-2.0-konform, REST-API. Operaton + CIB seven sind Camunda-7-Forks (OSS) |
| Workflow-Engine (Code-first) | [Temporal](https://temporal.io/) | ✅🌐 | Nicht BPMN, dafuer code-first; durable execution; Go-based. Rust-SDK in Entwicklung |
| Saga-Pattern | DIY auf State-Machine | ✅ | Kein fertiges Crate — Pattern statt Library |
| Kafka-Client | [`rdkafka`](https://crates.io/crates/rdkafka) | ✅ | Wrapper um librdkafka (C), produktionsreif |
| NATS-Client | [`async-nats`](https://crates.io/crates/async-nats) | ✅ | Pure-Rust, gepflegt |
| Schema-Registry | [Confluent Schema Registry](https://docs.confluent.io/platform/current/schema-registry/index.html) Service + [`schema_registry_converter`](https://crates.io/crates/schema_registry_converter) | ✅🌐 | Rust-Client gegen Confluent-API |
| Avro-Serialisierung | [`apache-avro`](https://crates.io/crates/apache-avro) | ✅ | Standard |
| Process Mining | [PM4Py](https://pm4py.fit.fraunhofer.de/) Python-Lib oder [Celonis](https://www.celonis.com/) | 🌐 | Keine Rust-Implementierung; entweder PM4Py als Sidecar oder Eigenbau-Heuristiken auf Audit-Log |

**Empfehlung Workflow**: fuer Code-First-Workflows ist **Temporal** der etablierte Standard; fuer BPMN-XML-getriebene Geschaeftsprozesse ist **Camunda/Zeebe** oder dessen OSS-Forks der Weg. `bpm-engine` (Rust) waere die spannende Alternative fuer All-Rust-Stack, sollte aber vor Produktivnutzung evaluiert werden.

---

## Phase 8 — Analytics & Embedded AI

| Capability | Tool | Status | Hinweise |
|---|---|---|---|
| Embedded OLAP | [`duckdb-rs`](https://crates.io/crates/duckdb) | ✅ | Wie Phase 6; primaer hier fuer Drill-Down-Cubes |
| Real-Time-MatViews | PostgreSQL nativ ODER [`pg_ivm`](https://github.com/sraoss/pg_ivm) | ✅🌐 | Native MatViews + manual refresh; pg_ivm fuer incremental |
| ML-Inference (ONNX) | [`ort`](https://github.com/pykeio/ort) | ✅ | ONNX Runtime Binding; 3–5x schneller als Python; mature, ueberragend wenn Modelle bereits ONNX-exportiert |
| ML-Inference (PyTorch-style) | [`candle`](https://github.com/huggingface/candle) | ✅ | Von Hugging Face; minimalistisch, kleine Binaries, gut fuer Edge/Serverless |
| ML-Inference (Pure Rust) | [`tract`](https://github.com/sonos/tract) | ✅ | Von Sonos, TF/ONNX-Parser, gut auf ARM (Edge) |
| Document AI (OCR) | [`leptess`](https://crates.io/crates/leptess) (Tesseract-Binding), Cloud (AWS Textract, Azure DI, Google DocumentAI) | 🟡🌐 | Lokale OCR via Tesseract akzeptabel; Cloud-OCR genauer fuer strukturierte Belege |
| Conversational AI / Copilot | LLM-Provider direkt (siehe Phase 3) + RAG-Tools | ✅ | Anthropic/OpenAI SDKs, eigener RAG-Loop |
| Vector-Store fuer RAG | [`qdrant-client`](https://crates.io/crates/qdrant-client), [`lance`](https://github.com/lancedb/lance), [`chroma`](https://github.com/chroma-core/chroma) | ✅🌐 | Qdrant als Service (Rust-geschrieben), Lance als Embedded |
| Self-Service BI Frontend | [Apache Superset](https://superset.apache.org/), [Metabase](https://www.metabase.com/) | 🌐 | Python/Java; via OpenAPI-Layer (Phase 1.7.21) integrierbar |

**Empfehlung ML-Inference**: Start mit `ort` fuer bestehende ONNX-Modelle (schnellster Pfad zu Produktion); `candle` fuer schnelle Iteration. Phase 4.5 WASI-NN-Switch nur, wenn lokale Inference-Anforderung real wird.

---

## Phase 9 — Globalisierung & MDM

| Capability | Tool | Status | Hinweise |
|---|---|---|---|
| Multi-Calendar | [`icu_calendar`](https://crates.io/crates/icu_calendar) (ICU4X) | ✅ | Hijri, Buddhist, Hebrew, Japanisch — Standard-Implementierung von Unicode |
| Multi-Locale-Formatting | [`icu`](https://crates.io/crates/icu) (ICU4X) | ✅ | Plurale, Datum, Zahl, Currency pro Locale |
| Multi-Language UI (bereits Stack) | Project Fluent | ✅ | Existiert, nichts hinzuzufuegen |
| RTL-Sprachen | CSS + Leptos direction-Setting | ✅ | Browser-nativ, kein Crate |
| Tax-Engine — externe APIs | [Avalara](https://www.avalara.com/), [Vertex](https://www.vertexinc.com/), [TaxJar](https://www.taxjar.com/) | 🌐 (kommerziell) | Pro Land + Use-Case lizenzieren |
| Tax-Engine — OSS | [openCRX TaxEngine](https://www.opencrx.org/) | 🌐 | Java-basiert, eingeschraenkt |
| XRechnung / ZUGFeRD / Factur-X | [Mustangproject](https://www.mustangproject.org/) (Java), [horstoeko/zugferd](https://github.com/horstoeko/zugferd) (PHP) | 🌐 | **Kein Rust-Crate verfuegbar.** Mustang als Java-Sidecar oder CLI integrieren. EN-16931-Spec konformes Format. |
| XRechnung Validator | [backoffice-plus/e-invoice-validator](https://github.com/backoffice-plus/e-invoice-validator) | 🌐 | Sidecar-Validator fuer DE |
| MDM Match-Merge | DIY oder kommerzielle Tools (Informatica, Reltio) | 🌐 | Open-Source-Rust hat hier kein Angebot |
| Address-Validation (DE) | externe APIs (Deutsche Post, melissa) | 🌐 (kommerziell) | Kein OSS-Standard |

**Realismus**: e-Invoicing in Rust ist eine **echte Lücke**. Bis 2026 keine fertige Rust-Implementierung gefunden. Empfehlung: Java-Sidecar (Mustang) oder PHP-Sidecar (horstoeko) als Sub-Service einbinden. Phase 3-Plugin-Architektur ist dafuer geeignet.

---

## Phase 10 — Integration Hub, ECM, Mobile

| Capability | Tool | Status | Hinweise |
|---|---|---|---|
| iPaaS-Adapter-Framework | [n8n](https://n8n.io/), [Apache Camel](https://camel.apache.org/) | 🌐 | n8n als low-code-Workflow; Camel als Java-EIP-Library |
| EDIFACT-Parser | [`edifact-parser`](https://crates.io/crates/edifact-parser) | 🔴 | Existiert, kaum gepflegt. Realistisch: Java/Python-Sidecar |
| ANSI X12 | kein Rust | 🌐 | externe Tools |
| UBL XML | [`yaserde`](https://crates.io/crates/yaserde) fuer XML-Roundtrip + Schema | 🟡 | Manuelle Schema-Mappings noetig |
| SOAP-Client/Server | [`yaserde`](https://crates.io/crates/yaserde) + DIY | 🔴 | Rust-SOAP ist schwach. Fuer Legacy-Enterprise besser ein Java/.NET-Sidecar |
| ECM (Records Management) | [Nuxeo](https://www.nuxeo.com/), [Alfresco](https://www.alfresco.com/) | 🌐 | Java-basiert; via REST-API integrierbar |
| DAM | [Pimcore](https://pimcore.com/) | 🌐 | PHP, OSS, breit eingesetzt |
| Mobile Apps | [Tauri](https://tauri.app/) 2.x (Desktop + Mobile) | ✅ | Rust-Backend, Web-Frontend; Leptos-kompatibel |
| Native Mobile (iOS/Android) | Swift / Kotlin | 🌐 | Wenn Tauri nicht reicht, klassische native Stacks |
| Offline-Sync | [Automerge](https://github.com/automerge/automerge) (CRDT) | ✅ | Pure-Rust, in-Browser via WASM, Konfliktfrei. Out-of-Scope laut ROADMAP, aber das Werkzeug existiert |
| WCAG-Test-Tooling | [axe-core](https://github.com/dequelabs/axe-core), [pa11y](https://github.com/pa11y/pa11y) | 🌐 | JS-basierte Test-Tools, in CI integrieren |

---

## Phase 11 — Lifecycle Management & Ecosystem

| Capability | Tool | Status | Hinweise |
|---|---|---|---|
| GitOps Multi-Env | [Argo CD](https://argoproj.github.io/cd/), [Flux CD](https://fluxcd.io/) | 🌐 | Kubernetes-zentriert |
| Migration-Diffs in Pipeline | [`sqlx migrate`](https://github.com/launchbadge/sqlx), eigene Tools | ✅ | Bereits durch Phase 3 abgedeckt |
| Blue/Green / Canary | [Argo Rollouts](https://argoproj.github.io/rollouts/), Service-Mesh (Istio, Linkerd) | 🌐 | Infra-Layer |
| Plugin-Marketplace | DIY + S3 + Signing-Chain | ✅ | Kein fertiges Open-Source-Marketplace-Framework |

---

## Cross-Cutting

### Observability & Logging

| Capability | Tool | Status |
|---|---|---|
| Tracing (structured) | [`tracing`](https://crates.io/crates/tracing) + [`tracing-subscriber`](https://crates.io/crates/tracing-subscriber) | ✅ |
| OpenTelemetry | [`opentelemetry`](https://crates.io/crates/opentelemetry) | ✅ |
| Metrics | [`metrics`](https://crates.io/crates/metrics) facade + Prometheus-Exporter | ✅ |
| Error-Tracking | [Sentry SDK](https://crates.io/crates/sentry) | ✅ |

### Validation

| Capability | Tool | Status |
|---|---|---|
| Derive-Based Validation | [`validator`](https://crates.io/crates/validator) | ✅ |
| Newer-Alternative | [`garde`](https://crates.io/crates/garde) | ✅ |
| IBAN-Check | [`iban_validate`](https://crates.io/crates/iban_validate) | ✅ |
| BIC-Check | [`swift-bic`](https://crates.io/crates/swift-bic) | 🟡 |
| VAT-ID-Check | [VIES](https://ec.europa.eu/taxation_customs/vies/) als externer Service + reqwest | 🌐 |

### Configuration

| Capability | Tool | Status |
|---|---|---|
| Config-Loading | [`figment`](https://crates.io/crates/figment) ODER [`config`](https://crates.io/crates/config) | ✅ |
| Secrets aus ENV | [`secrecy`](https://crates.io/crates/secrecy) | ✅ |
| Vault-Integration | [`vaultrs`](https://crates.io/crates/vaultrs) | ✅ |

### Crypto-Grundbausteine

| Capability | Tool | Status |
|---|---|---|
| Hashing | [`sha2`](https://crates.io/crates/sha2), [`blake3`](https://crates.io/crates/blake3) | ✅ |
| Password-Hashing | [`argon2`](https://crates.io/crates/argon2) | ✅ |
| Symmetric Encryption | [`aes-gcm`](https://crates.io/crates/aes-gcm), [`chacha20poly1305`](https://crates.io/crates/chacha20poly1305) | ✅ |
| Public-Key | [`ed25519-dalek`](https://crates.io/crates/ed25519-dalek), [`rsa`](https://crates.io/crates/rsa) | ✅ |
| TLS (Server) | [`rustls`](https://crates.io/crates/rustls) | ✅ |

---

## Wo das Rust-Ecosystem echte Luecken hat

Diese Bereiche **muessen** wir oder unsere User mit Nicht-Rust-Sidecars loesen, oder mit deutlich erhoehtem Eigenaufwand:

1. **E-Invoicing (XRechnung, ZUGFeRD, Peppol)**: keine Rust-Implementierung. Java (Mustang) oder PHP (horstoeko) als Sidecar.
2. **SOAP** (legacy Enterprise): Rust-SOAP-Crates sind unreif. Fuer SOAP-Heavy-Integration .NET- oder Java-Bridge.
3. **EDI** (X12, EDIFACT): wie SOAP — nicht in Rust gut abgebildet.
4. **Process Mining**: PM4Py (Python) oder kommerzielle Tools.
5. **eIDAS Signing (XAdES/PAdES/CAdES)**: kein Rust-Crate; DSS (Java) als Sidecar.
6. **PDF/A-Konvertierung**: Ghostscript als externer Process; pure-Rust nicht da.
7. **Native Mobile-Apps**: Tauri 2.x deckt vieles, fuer komplette Plattform-Features (Push, Background-Sync) klassischer Native-Stack.
8. **Compliance-Frameworks (SoX, ISO 27001, HIPAA)**: keine Library mit fertigem Controls-Katalog; Daten-Modell bauen oder kommerzielle GRC-Tools nutzen.
9. **Master Data Management (Match/Merge)**: kein OSS-Tooling auf Enterprise-Niveau.
10. **Tax-Engines (Multi-Country)**: kommerziell (Avalara, Vertex).

Strategie: **dblicious-Plattform bleibt Rust-pur; bei diesen Luecken wird die Plugin-Architektur (Phase 2) genutzt, um Java/Python/PHP-Sidecars als WASM-isolierte oder HTTP-isolierte Services anzubinden.** Das Manifest deklariert die externen Abhaengigkeiten, der Audit-Log erfasst sie.

---

## Pflege

Dieses Dokument wird pro Phase-Implementation aktualisiert. Wenn ein Crate-Status sich aendert (z.B. abandonment, Major-Version), bitte:

1. Status-Marker im Header anpassen
2. Falls Migration noetig, ADR schreiben (`docs/adr/`)
3. ROADMAP-Bezug pruefen — eventuell Phase-Risiken anpassen

Bei groesseren Aenderungen am Tech-Stack: `CLAUDE.md` und `VISION.md` synchron halten.
