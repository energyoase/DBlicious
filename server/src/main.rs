//! Mock-GraphQL-Server fuer den Leptos-Client.
//!
//! Stellt zwei HTTP-Endpunkte bereit:
//!   - `GET  /`        — GraphiQL-Oberflaeche zum interaktiven Testen
//!   - `POST /graphql` — eigentlicher GraphQL-Endpunkt
//!
//! Inhalte (Navigation, Entity-Metadaten, Seed-Daten, User/Gruppen,
//! Translatables) liegen **nicht** im Server-Code, sondern werden beim
//! Start aus dem `--data-dir`-Verzeichnis geladen. Ohne `--data-dir`
//! verweigert der Server den Start. Im Repo liegt `examples/shop/` als
//! Referenz-Beispiel.

use std::path::PathBuf;

use async_graphql::http::GraphiQLSource;
use async_graphql_axum::{GraphQLRequest, GraphQLResponse, GraphQLSubscription};
use axum::{
    extract::State,
    http::HeaderMap,
    response::{Html, IntoResponse},
    routing::{get, post},
    Router,
};
use clap::Parser;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use server::{auth, build_schema, db, example, AppSchema, AuthContext};

#[derive(Parser, Debug)]
#[command(
    name = "dblicious-server",
    about = "GraphQL-Server fuer dblicious",
    version
)]
struct Args {
    /// Pfad zum Beispiel-/Daten-Ordner. Muss konfig/Seed-Daten enthalten
    /// (vgl. `examples/shop/`).
    #[arg(long, value_name = "DIR")]
    data_dir: PathBuf,

    /// Bind-Adresse (`host:port`) — ueberschreibt config.toml.
    #[arg(long)]
    bind: Option<String>,
}

async fn graphiql() -> impl IntoResponse {
    Html(
        GraphiQLSource::build()
            .endpoint("/graphql")
            .subscription_endpoint("/graphql/ws")
            .finish(),
    )
}

async fn graphql_handler(
    State(schema): State<AppSchema>,
    headers: HeaderMap,
    req: GraphQLRequest,
) -> GraphQLResponse {
    let raw = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());
    let token = raw
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|s| s.trim().to_string());
    let user = match raw {
        Some(s) => auth::user_for_bearer(Some(s)).await,
        None => None,
    };
    let request = req.into_inner().data(AuthContext { user, token });
    schema.execute(request).await.into()
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "server=info,tower_http=info".into()),
        )
        .init();

    let args = Args::parse();

    // Beispiel-/Daten-Ordner laden. Schlaegt der Load fehl, bricht der
    // Server *vor* dem DB-Init ab — kein Halbzustand.
    let set = match example::load(&args.data_dir) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(
                "Daten-Ordner '{}' konnte nicht geladen werden: {e:#}",
                args.data_dir.display()
            );
            std::process::exit(2);
        }
    };
    let bind = args.bind.clone().unwrap_or_else(|| set.config.bind.clone());
    let example_name = set.config.name.clone();
    let entity_types: Vec<String> = set.entity_types().map(str::to_string).collect();
    let sources = set.sources.clone();
    example::install(set);
    tracing::info!(
        "Beispiel '{example_name}' aus '{}' geladen ({} Entity-Typen: {})",
        args.data_dir.display(),
        entity_types.len(),
        entity_types.join(", ")
    );

    // SeaORM-Pool initialisieren. Ohne `DBLICIOUS_DATABASE_URL` laeuft
    // alles gegen `sqlite::memory:` — komfortabel fuer Dev, aber pro
    // Server-Neustart sind alle Daten weg.
    if let Err(e) = db::init().await {
        tracing::error!("DB-Init fehlgeschlagen: {e}");
        std::process::exit(1);
    }

    // Phase 0.6: SourceRegistry aus sources.toml booten — sonst routet
    // jeder CRUD-Resolver auf einen leeren Slot und liefert silent leere
    // Pages. fresh_test_setup macht das, main hat es vorher vergessen.
    if let Err(e) = server::source::boot_registry(&sources).await {
        tracing::error!("Source-Registry-Boot fehlgeschlagen: {e}");
        std::process::exit(1);
    }
    tracing::info!(
        "SourceRegistry geladen ({} Source(s): {})",
        sources.len(),
        sources.keys().cloned().collect::<Vec<_>>().join(", ")
    );
    let _ = server::data::rehydrate_db_schema().await;

    // Phase 1.7.7: Cron-Loop spawnen, sobald die DB steht. Intervall 30s
    // ist konservativ — Cron-Granularitaet ist 1 Minute, also reicht das
    // doppelt so haeufige Polling, um jeden Slot ohne Drift zu treffen.
    // Handle leakt absichtlich (Loop laeuft, bis der Prozess endet).
    let _scheduler_handle =
        server::jobs::start_scheduler_loop(server::db::conn(), std::time::Duration::from_secs(30));
    tracing::info!("Job-Scheduler-Loop gestartet (Intervall 30s)");

    let schema: AppSchema = build_schema();

    // CORS bewusst offen — nur fuer lokale Entwicklung gedacht.
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/", get(graphiql))
        .route("/graphql", post(graphql_handler))
        // Phase 1.6: WebSocket-Subscriptions (entityDesignUpdated).
        .route_service("/graphql/ws", GraphQLSubscription::new(schema.clone()))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(schema);

    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .unwrap_or_else(|e| {
            tracing::error!("Bind auf '{bind}' fehlgeschlagen: {e}");
            std::process::exit(1);
        });
    tracing::info!("GraphQL-Server laeuft auf http://{bind}  (GraphiQL unter /)");
    axum::serve(listener, app).await.expect("Server-Fehler");
}
