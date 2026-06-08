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
    )
    .increment(1);
}

pub(crate) fn account_created(outcome: &'static str) {
    metrics::counter!(
        "password_vault_accounts_created_total",
        "outcome" => outcome,
    )
    .increment(1);
}

pub(crate) fn login_start(outcome: &'static str) {
    metrics::counter!(
        "password_vault_login_starts_total",
        "outcome" => outcome,
    )
    .increment(1);
}

pub(crate) fn login_attempt(outcome: &'static str, failure_class: &'static str) {
    metrics::counter!(
        "password_vault_login_attempts_total",
        "outcome" => outcome,
        "failure_class" => failure_class,
    )
    .increment(1);
}

pub(crate) fn session_event(event: &'static str, outcome: &'static str) {
    metrics::counter!(
        "password_vault_session_events_total",
        "event" => event,
        "outcome" => outcome,
    )
    .increment(1);
}

pub(crate) fn mfa_event(event: &'static str, outcome: &'static str) {
    metrics::counter!(
        "password_vault_mfa_events_total",
        "event" => event,
        "outcome" => outcome,
    )
    .increment(1);
}

pub(crate) fn sync_request(outcome: &'static str, page: &'static str) {
    metrics::counter!(
        "password_vault_sync_requests_total",
        "outcome" => outcome,
        "page" => page,
    )
    .increment(1);
}

pub(crate) fn vault_item_change(operation: &'static str, outcome: &'static str) {
    metrics::counter!(
        "password_vault_vault_item_changes_total",
        "operation" => operation,
        "outcome" => outcome,
    )
    .increment(1);
}
