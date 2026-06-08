use std::env;

use sqlx::PgPool;
use time::{Duration, OffsetDateTime};

pub const DEFAULT_SYNTHETIC_CLEANUP_PREFIX: &str = "synthetic";
pub const DEFAULT_SYNTHETIC_CLEANUP_DOMAIN: &str = "loadtest.invalid";
pub const DEFAULT_SYNTHETIC_CLEANUP_MIN_AGE_HOURS: i64 = 24;
pub const DEFAULT_SYNTHETIC_CLEANUP_MAX_DELETE: i64 = 100;

#[derive(Debug, Clone)]
pub struct SyntheticCleanupOptions {
    pub prefix: String,
    pub domain: String,
    pub cutoff: OffsetDateTime,
    pub dry_run: bool,
    pub max_delete: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyntheticCleanupReport {
    pub matched: u64,
    pub deleted: u64,
    pub dry_run: bool,
    pub max_delete: i64,
}

#[derive(Debug)]
pub enum SyntheticCleanupError {
    InvalidConfig(&'static str),
    Database(sqlx::Error),
}

impl SyntheticCleanupOptions {
    pub fn from_env(dry_run: bool) -> Result<Self, SyntheticCleanupError> {
        let prefix = env::var("PV_SYNTHETIC_CLEANUP_PREFIX")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_SYNTHETIC_CLEANUP_PREFIX.to_string())
            .trim()
            .to_lowercase();
        let domain = env::var("PV_SYNTHETIC_CLEANUP_DOMAIN")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_SYNTHETIC_CLEANUP_DOMAIN.to_string())
            .trim()
            .to_lowercase();
        let min_age_hours = parse_i64_env(
            "PV_SYNTHETIC_CLEANUP_MIN_AGE_HOURS",
            DEFAULT_SYNTHETIC_CLEANUP_MIN_AGE_HOURS,
        )?;
        let max_delete = parse_i64_env(
            "PV_SYNTHETIC_CLEANUP_MAX_DELETE",
            DEFAULT_SYNTHETIC_CLEANUP_MAX_DELETE,
        )?;

        Self::new(prefix, domain, min_age_hours, dry_run, max_delete)
    }

    pub fn new(
        prefix: impl Into<String>,
        domain: impl Into<String>,
        min_age_hours: i64,
        dry_run: bool,
        max_delete: i64,
    ) -> Result<Self, SyntheticCleanupError> {
        let prefix = prefix.into();
        let domain = domain.into();
        validate_prefix(&prefix)?;
        validate_domain(&domain)?;
        if min_age_hours < 1 {
            return Err(SyntheticCleanupError::InvalidConfig(
                "synthetic cleanup min age must be at least 1 hour",
            ));
        }
        if max_delete < 1 {
            return Err(SyntheticCleanupError::InvalidConfig(
                "synthetic cleanup max delete must be at least 1",
            ));
        }

        Ok(Self {
            prefix,
            domain,
            cutoff: OffsetDateTime::now_utc() - Duration::hours(min_age_hours),
            dry_run,
            max_delete,
        })
    }

    #[cfg(test)]
    pub fn for_test(
        prefix: impl Into<String>,
        domain: impl Into<String>,
        cutoff: OffsetDateTime,
        dry_run: bool,
        max_delete: i64,
    ) -> Result<Self, SyntheticCleanupError> {
        let prefix = prefix.into();
        let domain = domain.into();
        validate_prefix(&prefix)?;
        validate_domain(&domain)?;
        if max_delete < 1 {
            return Err(SyntheticCleanupError::InvalidConfig(
                "synthetic cleanup max delete must be at least 1",
            ));
        }
        Ok(Self {
            prefix,
            domain,
            cutoff,
            dry_run,
            max_delete,
        })
    }

    fn regex_pattern(&self) -> String {
        format!(
            "^{}-[a-z0-9._-]+@{}$",
            postgres_regex_literal(&self.prefix),
            postgres_regex_literal(&self.domain)
        )
    }
}

pub async fn cleanup_synthetic_accounts(
    pool: &PgPool,
    options: &SyntheticCleanupOptions,
) -> Result<SyntheticCleanupReport, SyntheticCleanupError> {
    let pattern = options.regex_pattern();
    let matched = sqlx::query_scalar::<_, i64>(
        "
        SELECT count(*)::bigint
        FROM accounts
        WHERE login_handle_normalized ~ $1
          AND created_at < $2
        ",
    )
    .bind(&pattern)
    .bind(options.cutoff)
    .fetch_one(pool)
    .await
    .map_err(SyntheticCleanupError::Database)?;

    if options.dry_run {
        return Ok(SyntheticCleanupReport {
            matched: matched as u64,
            deleted: 0,
            dry_run: true,
            max_delete: options.max_delete,
        });
    }

    let deleted = sqlx::query_scalar::<_, i64>(
        "
        WITH candidates AS (
            SELECT id
            FROM accounts
            WHERE login_handle_normalized ~ $1
              AND created_at < $2
            ORDER BY created_at ASC
            LIMIT $3
        ),
        deleted AS (
            DELETE FROM accounts
            WHERE id IN (SELECT id FROM candidates)
            RETURNING id
        )
        SELECT count(*)::bigint FROM deleted
        ",
    )
    .bind(&pattern)
    .bind(options.cutoff)
    .bind(options.max_delete)
    .fetch_one(pool)
    .await
    .map_err(SyntheticCleanupError::Database)?;

    Ok(SyntheticCleanupReport {
        matched: matched as u64,
        deleted: deleted as u64,
        dry_run: false,
        max_delete: options.max_delete,
    })
}

fn postgres_regex_literal(value: &str) -> String {
    value.replace('.', r"\.")
}

fn parse_i64_env(name: &str, default_value: i64) -> Result<i64, SyntheticCleanupError> {
    match env::var(name) {
        Ok(value) if !value.trim().is_empty() => value.trim().parse::<i64>().map_err(|_| {
            SyntheticCleanupError::InvalidConfig("synthetic cleanup env value must be an integer")
        }),
        _ => Ok(default_value),
    }
}

fn validate_prefix(value: &str) -> Result<(), SyntheticCleanupError> {
    if value.is_empty()
        || value.len() > 32
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-')
        })
    {
        return Err(SyntheticCleanupError::InvalidConfig(
            "synthetic cleanup prefix must be a lowercase safe label",
        ));
    }
    Ok(())
}

fn validate_domain(value: &str) -> Result<(), SyntheticCleanupError> {
    if value.is_empty()
        || value.len() > 80
        || !value.ends_with(".invalid")
        || value.starts_with('.')
        || value.contains("..")
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-')
        })
    {
        return Err(SyntheticCleanupError::InvalidConfig(
            "synthetic cleanup domain must be a safe .invalid domain",
        ));
    }
    Ok(())
}

impl std::fmt::Display for SyntheticCleanupError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidConfig(message) => write!(formatter, "{message}"),
            Self::Database(_) => write!(formatter, "synthetic cleanup database operation failed"),
        }
    }
}

impl std::error::Error for SyntheticCleanupError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            Self::InvalidConfig(_) => None,
        }
    }
}
