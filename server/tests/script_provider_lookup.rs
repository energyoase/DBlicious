//! Server-Pendant zum Client-Test `client/tests/script_provider_lookup.rs`.
//! Pin: identische Fixture (Reader-Tier Formatter, source `40 + 2`) muss in
//! Client und Server denselben Display-String liefern (Spec §8 Symmetrie).

use server::script::provider_lookup::{
    lookup_provider_with_script, parse_script_id, script_value_to_display, FallbackReason,
    LookupResult, SCRIPT_PREFIX,
};

use shared::script::engine::ScriptCtx;
use shared::script::model::{ProviderSlot, Script, ScriptKind, ScriptState};
use shared::script::testing::MockHostApi;
use shared::script::{CapabilityToken, ScriptId, ScriptManifest, ScriptTier};

fn formatter_script(id: &str, source: &str, state: ScriptState) -> Script {
    Script {
        id: ScriptId(id.into()),
        kind: ScriptKind::Provider {
            slot: ProviderSlot::Formatter,
        },
        manifest: ScriptManifest {
            manifest_version: 1,
            tier: ScriptTier::Reader,
            capabilities: vec![CapabilityToken::ComputeOnly],
            ..Default::default()
        },
        source: source.into(),
        version: 1,
        state,
        last_error: None,
        created_by: "u-1".into(),
        created_at: "2026-05-23T00:00:00Z".into(),
        updated_at: "2026-05-23T00:00:00Z".into(),
    }
}

#[test]
fn server_script_prefix_constant_matches_client() {
    assert_eq!(SCRIPT_PREFIX, "script:");
}

#[test]
fn server_parse_script_id_strips_prefix() {
    assert_eq!(parse_script_id("script:abc"), Some(ScriptId("abc".into())));
    assert!(parse_script_id("xyz").is_none());
}

#[test]
fn server_active_formatter_evaluates_to_same_value_as_client() {
    // Diese Fixture spiegelt `client/tests/script_provider_lookup.rs::
    // provider_lookup_returns_value_for_active_formatter`. Symmetrie-Pin:
    // gleicher source + gleicher Slot + gleiches Manifest → gleicher
    // Display-String.
    let s = formatter_script("discount-tier", "40 + 2", ScriptState::Active);
    let host = MockHostApi::new();
    let res = lookup_provider_with_script(
        "script:discount-tier",
        ProviderSlot::Formatter,
        Some(&s),
        &host,
        ScriptCtx::default(),
    );
    match res {
        LookupResult::Ok { value } => {
            assert_eq!(script_value_to_display(&value), "42");
        }
        other => panic!("expected Ok, got {other:?}"),
    }
}

#[test]
fn server_missing_script_falls_back() {
    let host = MockHostApi::new();
    let res = lookup_provider_with_script(
        "script:does-not-exist",
        ProviderSlot::Formatter,
        None,
        &host,
        ScriptCtx::default(),
    );
    match res {
        LookupResult::Fallback {
            reason: FallbackReason::Missing { script_id },
        } => assert_eq!(script_id, "does-not-exist"),
        other => panic!("expected Fallback::Missing, got {other:?}"),
    }
}
