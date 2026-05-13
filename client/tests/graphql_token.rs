//! WASM-Client-Tests-Skeleton.
//!
//! Diese Tests laufen via `wasm-pack test --headless --firefox` (oder
//! `--chrome`). Sie pruefen *nicht* die Leptos-Reaktivitaet selbst, sondern
//! die statischen Helper-Pfade — z.B. den GraphQL-Token-Store.
//!
//! Setup-Hinweis: `cargo test -p client` allein triggert die WASM-Tests
//! nicht — dafuer wird `wasm-pack` benoetigt. Die Tests bleiben hier
//! kompilierfaehig, damit der CI-Setup spaeter "nur" `wasm-pack test`
//! drauf werfen muss.

#![cfg(target_arch = "wasm32")]

use client::graphql::{get_auth_token, set_auth_token};
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn auth_token_store_round_trips() {
    set_auth_token(Some("tok-123".into()));
    assert_eq!(get_auth_token().as_deref(), Some("tok-123"));
    set_auth_token(None);
    assert!(get_auth_token().is_none());
}
