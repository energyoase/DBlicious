//! GraphQL-Client und Query-Definitionen.
//!
//! Bewusst ohne Code-Generator (z.B. `graphql_client` oder `cynic`):
//! die Mock-Domaene ist klein, und so bleibt die Build-Pipeline auf
//! `cargo`/`trunk` beschraenkt. Bei wachsender Komplexitaet kann hier
//! durch einen Generator ersetzt werden, ohne die aufrufenden
//! Komponenten zu beruehren.

pub mod queries;

use std::sync::{Mutex, OnceLock};

use gloo_net::http::Request;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use thiserror::Error;

const ENDPOINT: &str = "/graphql";

/// Aktives Bearer-Token. Wird beim erfolgreichen Login gesetzt und bei jedem
/// Request an den Server gehaengt. Persistenz uebernimmt der `AuthContext`
/// (siehe `client/src/auth.rs`).
fn current_token() -> &'static Mutex<Option<String>> {
    static T: OnceLock<Mutex<Option<String>>> = OnceLock::new();
    T.get_or_init(|| Mutex::new(None))
}

pub fn set_auth_token(token: Option<String>) {
    *current_token().lock().unwrap() = token;
}

pub fn get_auth_token() -> Option<String> {
    current_token().lock().unwrap().clone()
}

#[derive(Debug, Clone, Error)]
pub enum GqlError {
    #[error("Netzwerkfehler: {0}")]
    Network(String),
    #[error("Server lieferte Fehler: {0}")]
    GraphQl(String),
    #[error("Antwort konnte nicht deserialisiert werden: {0}")]
    Decode(String),
}

#[derive(Serialize)]
struct GqlRequest<'a, V: Serialize> {
    query: &'a str,
    variables: V,
}

#[derive(Deserialize)]
struct GqlResponse<T> {
    data: Option<T>,
    errors: Option<Vec<GqlErrorPayload>>,
}

#[derive(Deserialize, Debug)]
struct GqlErrorPayload {
    message: String,
}

/// Fuehrt eine GraphQL-Operation aus.
pub async fn execute<V, T>(query: &str, variables: V) -> Result<T, GqlError>
where
    V: Serialize,
    T: DeserializeOwned,
{
    let body = GqlRequest { query, variables };
    let mut req = Request::post(ENDPOINT).header("Content-Type", "application/json");
    if let Some(token) = get_auth_token() {
        req = req.header("Authorization", &format!("Bearer {token}"));
    }
    let response = req
        .json(&body)
        .map_err(|e| GqlError::Network(e.to_string()))?
        .send()
        .await
        .map_err(|e| GqlError::Network(e.to_string()))?;

    let parsed: GqlResponse<T> = response
        .json()
        .await
        .map_err(|e| GqlError::Decode(e.to_string()))?;

    if let Some(errors) = parsed.errors {
        if !errors.is_empty() {
            return Err(GqlError::GraphQl(
                errors
                    .into_iter()
                    .map(|e| e.message)
                    .collect::<Vec<_>>()
                    .join("; "),
            ));
        }
    }

    parsed
        .data
        .ok_or_else(|| GqlError::GraphQl("Antwort enthaelt keine `data`".into()))
}
