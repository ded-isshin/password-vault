use std::{future::Future, time::Duration};

use axum::http::HeaderMap;
use subtle::ConstantTimeEq;

pub(crate) const TRAFFIC_CLASS_HEADER: &str = "x-password-vault-traffic-class";
pub(crate) const SYNTHETIC_TRAFFIC_TOKEN_HEADER: &str = "x-password-vault-synthetic-token";
const TRAFFIC_CLASS_SYNTHETIC: &str = "synthetic";
const TRAFFIC_CLASS_USER: &str = "user";
const TRAFFIC_CLASS_UNKNOWN: &str = "unknown";

tokio::task_local! {
    static REQUEST_TRAFFIC_CLASS: &'static str;
}

pub(crate) fn traffic_class_from_headers(
    headers: &HeaderMap,
    synthetic_traffic_token: Option<&str>,
) -> &'static str {
    let requested_class = headers
        .get(TRAFFIC_CLASS_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim);
    if requested_class != Some(TRAFFIC_CLASS_SYNTHETIC) {
        return TRAFFIC_CLASS_USER;
    }

    let Some(expected_token) = synthetic_traffic_token else {
        return TRAFFIC_CLASS_USER;
    };
    let Some(provided_token) = headers
        .get(SYNTHETIC_TRAFFIC_TOKEN_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
    else {
        return TRAFFIC_CLASS_USER;
    };

    if constant_time_token_eq(provided_token, expected_token) {
        TRAFFIC_CLASS_SYNTHETIC
    } else {
        TRAFFIC_CLASS_USER
    }
}

fn constant_time_token_eq(provided: &str, expected: &str) -> bool {
    let provided = provided.as_bytes();
    let expected = expected.as_bytes();
    provided.len() == expected.len() && provided.ct_eq(expected).into()
}

pub(crate) async fn scope_request_traffic_class<F>(
    traffic_class: &'static str,
    future: F,
) -> F::Output
where
    F: Future,
{
    REQUEST_TRAFFIC_CLASS.scope(traffic_class, future).await
}

pub(crate) fn current_traffic_class() -> &'static str {
    REQUEST_TRAFFIC_CLASS
        .try_with(|traffic_class| *traffic_class)
        .unwrap_or(TRAFFIC_CLASS_UNKNOWN)
}

pub(crate) fn record_build_info() {
    metrics::gauge!(
        "password_vault_build_info",
        "version" => env!("CARGO_PKG_VERSION"),
        "revision" => option_env!("PASSWORD_VAULT_BUILD_REVISION").unwrap_or("unknown"),
    )
    .set(1.0);
}

pub(crate) fn registration_event(event: &'static str, outcome: &'static str) {
    metrics::counter!(
        "password_vault_registration_events_total",
        "event" => event,
        "outcome" => outcome,
        "traffic_class" => current_traffic_class(),
    )
    .increment(1);
}

pub(crate) fn account_created(outcome: &'static str) {
    metrics::counter!(
        "password_vault_accounts_created_total",
        "outcome" => outcome,
        "traffic_class" => current_traffic_class(),
    )
    .increment(1);
}

pub(crate) fn login_start(outcome: &'static str) {
    metrics::counter!(
        "password_vault_login_starts_total",
        "outcome" => outcome,
        "traffic_class" => current_traffic_class(),
    )
    .increment(1);
}

pub(crate) fn login_attempt(outcome: &'static str, failure_class: &'static str) {
    metrics::counter!(
        "password_vault_login_attempts_total",
        "outcome" => outcome,
        "failure_class" => failure_class,
        "traffic_class" => current_traffic_class(),
    )
    .increment(1);
}

pub(crate) fn rate_limited_request(policy: &'static str, flow: &'static str) {
    metrics::counter!(
        "password_vault_rate_limited_requests_total",
        "policy" => policy,
        "flow" => flow,
        "traffic_class" => current_traffic_class(),
    )
    .increment(1);
}

pub(crate) fn session_event(event: &'static str, outcome: &'static str) {
    metrics::counter!(
        "password_vault_session_events_total",
        "event" => event,
        "outcome" => outcome,
        "traffic_class" => current_traffic_class(),
    )
    .increment(1);
}

pub(crate) fn db_pool_connections(max: u32, size: u32, idle: usize) {
    let idle = u32::try_from(idle).unwrap_or(u32::MAX).min(size);
    let used = size.saturating_sub(idle);
    metrics::gauge!("password_vault_db_pool_connections", "state" => "max").set(f64::from(max));
    metrics::gauge!("password_vault_db_pool_connections", "state" => "idle").set(f64::from(idle));
    metrics::gauge!("password_vault_db_pool_connections", "state" => "used").set(f64::from(used));
}

pub(crate) fn db_pool_wait_duration(
    operation: &'static str,
    outcome: &'static str,
    duration: Duration,
) {
    metrics::histogram!(
        "password_vault_db_pool_wait_duration_seconds",
        "operation" => operation,
        "outcome" => outcome,
    )
    .record(duration.as_secs_f64());
}

pub(crate) fn db_query_duration(
    operation: &'static str,
    outcome: &'static str,
    duration: Duration,
) {
    metrics::histogram!(
        "password_vault_db_query_duration_seconds",
        "operation" => operation,
        "outcome" => outcome,
    )
    .record(duration.as_secs_f64());
}

pub(crate) fn db_error(operation: &'static str, error_class: &'static str) {
    metrics::counter!(
        "password_vault_db_errors_total",
        "operation" => operation,
        "error_class" => error_class,
    )
    .increment(1);
}

pub(crate) fn mfa_event(event: &'static str, outcome: &'static str) {
    metrics::counter!(
        "password_vault_mfa_events_total",
        "event" => event,
        "outcome" => outcome,
        "traffic_class" => current_traffic_class(),
    )
    .increment(1);
}

pub(crate) fn sync_request(outcome: &'static str, page: &'static str) {
    metrics::counter!(
        "password_vault_sync_requests_total",
        "outcome" => outcome,
        "page" => page,
        "traffic_class" => current_traffic_class(),
    )
    .increment(1);
}

pub(crate) fn vault_item_change(operation: &'static str, outcome: &'static str) {
    metrics::counter!(
        "password_vault_vault_item_changes_total",
        "operation" => operation,
        "outcome" => outcome,
        "traffic_class" => current_traffic_class(),
    )
    .increment(1);
}
