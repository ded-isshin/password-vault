use hmac::{Hmac, Mac};
use sha1::Sha1;
use sha2::{Sha256, Sha512};
use subtle::ConstantTimeEq;

pub const MIN_SEED_BYTES: usize = 20;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TotpAlgorithm {
    Sha1,
    Sha256,
    Sha512,
}

impl TotpAlgorithm {
    pub fn as_uri_value(self) -> &'static str {
        match self {
            Self::Sha1 => "SHA1",
            Self::Sha256 => "SHA256",
            Self::Sha512 => "SHA512",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TotpProfile {
    pub algorithm: TotpAlgorithm,
    pub digits: u32,
    pub period_seconds: u64,
}

impl TotpProfile {
    pub const fn google_authenticator_default() -> Self {
        Self {
            algorithm: TotpAlgorithm::Sha1,
            digits: 6,
            period_seconds: 30,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum TotpError {
    InvalidDigits,
    InvalidPeriod,
    InvalidCode,
    InvalidSeedLength { minimum: usize, actual: usize },
}

impl std::fmt::Display for TotpError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidDigits => write!(formatter, "TOTP digits must be 6 or 8"),
            Self::InvalidPeriod => write!(formatter, "TOTP period must be greater than 0"),
            Self::InvalidCode => write!(formatter, "TOTP code has invalid shape"),
            Self::InvalidSeedLength { minimum, actual } => write!(
                formatter,
                "TOTP seed must be at least {minimum} bytes; got {actual}"
            ),
        }
    }
}

impl std::error::Error for TotpError {}

pub fn generate(
    seed: &[u8],
    unix_time_seconds: u64,
    profile: TotpProfile,
) -> Result<String, TotpError> {
    validate_profile(profile)?;
    validate_seed(seed)?;
    let counter = time_step(unix_time_seconds, profile.period_seconds)?;
    let value = hotp(seed, counter, profile.algorithm, profile.digits)?;
    Ok(format_code(value, profile.digits))
}

pub fn verify(
    seed: &[u8],
    unix_time_seconds: u64,
    code: &str,
    profile: TotpProfile,
    last_accepted_step: Option<u64>,
) -> Result<Option<u64>, TotpError> {
    validate_profile(profile)?;
    validate_seed(seed)?;
    validate_code(code, profile.digits)?;

    let current = time_step(unix_time_seconds, profile.period_seconds)?;
    let candidates = [
        current.checked_sub(1),
        Some(current),
        current.checked_add(1),
    ];

    for candidate in candidates.into_iter().flatten() {
        if last_accepted_step.is_some_and(|last| candidate <= last) {
            continue;
        }

        let expected = format_code(
            hotp(seed, candidate, profile.algorithm, profile.digits)?,
            profile.digits,
        );

        if expected.as_bytes().ct_eq(code.as_bytes()).into() {
            return Ok(Some(candidate));
        }
    }

    Ok(None)
}

pub fn time_step(unix_time_seconds: u64, period_seconds: u64) -> Result<u64, TotpError> {
    if period_seconds == 0 {
        return Err(TotpError::InvalidPeriod);
    }
    Ok(unix_time_seconds / period_seconds)
}

pub fn base32_no_padding(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

    // Five-bit groups can straddle at most two adjacent input bytes.
    let mut output = String::with_capacity((bytes.len() * 8).div_ceil(5));
    let mut buffer = 0u16;
    let mut bits = 0u8;

    for byte in bytes {
        buffer = (buffer << 8) | u16::from(*byte);
        bits += 8;

        while bits >= 5 {
            let index = ((buffer >> (bits - 5)) & 0b11111) as usize;
            output.push(ALPHABET[index] as char);
            bits -= 5;
        }
    }

    if bits > 0 {
        let index = ((buffer << (5 - bits)) & 0b11111) as usize;
        output.push(ALPHABET[index] as char);
    }

    output
}

pub fn provisioning_uri(
    issuer: &str,
    account_label: &str,
    seed: &[u8],
    profile: TotpProfile,
) -> Result<String, TotpError> {
    validate_profile(profile)?;
    validate_seed(seed)?;

    let issuer = percent_encode(issuer);
    let account_label = percent_encode(account_label);
    let secret = base32_no_padding(seed);

    Ok(format!(
        "otpauth://totp/{issuer}:{account_label}?secret={secret}&issuer={issuer}&algorithm={algorithm}&digits={digits}&period={period}",
        algorithm = profile.algorithm.as_uri_value(),
        digits = profile.digits,
        period = profile.period_seconds,
    ))
}

fn hotp(
    seed: &[u8],
    counter: u64,
    algorithm: TotpAlgorithm,
    digits: u32,
) -> Result<u32, TotpError> {
    validate_digits(digits)?;
    let counter = counter.to_be_bytes();
    let digest = match algorithm {
        TotpAlgorithm::Sha1 => hmac_digest::<Hmac<Sha1>>(seed, &counter),
        TotpAlgorithm::Sha256 => hmac_digest::<Hmac<Sha256>>(seed, &counter),
        TotpAlgorithm::Sha512 => hmac_digest::<Hmac<Sha512>>(seed, &counter),
    };
    let offset = usize::from(digest[digest.len() - 1] & 0x0f);
    let binary = (u32::from(digest[offset] & 0x7f) << 24)
        | (u32::from(digest[offset + 1]) << 16)
        | (u32::from(digest[offset + 2]) << 8)
        | u32::from(digest[offset + 3]);

    Ok(binary % 10u32.pow(digits))
}

fn hmac_digest<M>(key: &[u8], data: &[u8]) -> Vec<u8>
where
    M: Mac + hmac::digest::KeyInit,
{
    let mut mac =
        <M as hmac::digest::KeyInit>::new_from_slice(key).expect("HMAC accepts keys of any length");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn format_code(value: u32, digits: u32) -> String {
    format!("{value:0width$}", width = digits as usize)
}

fn validate_profile(profile: TotpProfile) -> Result<(), TotpError> {
    validate_digits(profile.digits)?;
    if profile.period_seconds == 0 {
        return Err(TotpError::InvalidPeriod);
    }
    Ok(())
}

fn validate_seed(seed: &[u8]) -> Result<(), TotpError> {
    if seed.len() < MIN_SEED_BYTES {
        return Err(TotpError::InvalidSeedLength {
            minimum: MIN_SEED_BYTES,
            actual: seed.len(),
        });
    }
    Ok(())
}

fn validate_digits(digits: u32) -> Result<(), TotpError> {
    match digits {
        6 | 8 => Ok(()),
        _ => Err(TotpError::InvalidDigits),
    }
}

fn validate_code(code: &str, digits: u32) -> Result<(), TotpError> {
    if code.len() == digits as usize && code.as_bytes().iter().all(u8::is_ascii_digit) {
        Ok(())
    } else {
        Err(TotpError::InvalidCode)
    }
}

fn percent_encode(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                output.push(byte as char);
            }
            _ => {
                output.push('%');
                output.push_str(&format!("{byte:02X}"));
            }
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{
        MIN_SEED_BYTES, TotpAlgorithm, TotpError, TotpProfile, base32_no_padding, generate,
        provisioning_uri, verify,
    };

    #[test]
    fn matches_rfc6238_appendix_b_vectors() {
        let cases = [
            (59, "94287082", "46119246", "90693936"),
            (1_111_111_109, "07081804", "68084774", "25091201"),
            (1_111_111_111, "14050471", "67062674", "99943326"),
            (1_234_567_890, "89005924", "91819424", "93441116"),
            (2_000_000_000, "69279037", "90698825", "38618901"),
            (20_000_000_000, "65353130", "77737706", "47863826"),
        ];
        let sha1_seed = b"12345678901234567890";
        let sha256_seed = b"12345678901234567890123456789012";
        let sha512_seed = b"1234567890123456789012345678901234567890123456789012345678901234";

        for (time, sha1, sha256, sha512) in cases {
            assert_eq!(
                generate(
                    sha1_seed,
                    time,
                    TotpProfile {
                        algorithm: TotpAlgorithm::Sha1,
                        digits: 8,
                        period_seconds: 30,
                    },
                )
                .expect("SHA1 vector generates"),
                sha1
            );
            assert_eq!(
                generate(
                    sha256_seed,
                    time,
                    TotpProfile {
                        algorithm: TotpAlgorithm::Sha256,
                        digits: 8,
                        period_seconds: 30,
                    },
                )
                .expect("SHA256 vector generates"),
                sha256
            );
            assert_eq!(
                generate(
                    sha512_seed,
                    time,
                    TotpProfile {
                        algorithm: TotpAlgorithm::Sha512,
                        digits: 8,
                        period_seconds: 30,
                    },
                )
                .expect("SHA512 vector generates"),
                sha512
            );
        }
    }

    #[test]
    fn verifies_adjacent_steps_and_rejects_replay() {
        let seed = b"12345678901234567890";
        let profile = TotpProfile::google_authenticator_default();
        let current_time = 1_234_567_890;
        let previous_code = generate(seed, current_time - 30, profile).expect("code generates");
        let current_code = generate(seed, current_time, profile).expect("code generates");
        let next_code = generate(seed, current_time + 30, profile).expect("code generates");

        assert_eq!(
            verify(seed, current_time, &previous_code, profile, None).expect("verify succeeds"),
            Some(41_152_262)
        );
        assert_eq!(
            verify(seed, current_time, &current_code, profile, None).expect("verify succeeds"),
            Some(41_152_263)
        );
        assert_eq!(
            verify(seed, current_time, &next_code, profile, None).expect("verify succeeds"),
            Some(41_152_264)
        );
        assert_eq!(
            verify(seed, current_time, &current_code, profile, Some(41_152_263))
                .expect("verify succeeds"),
            None
        );
    }

    #[test]
    fn rejects_codes_outside_adjacent_window() {
        let seed = b"12345678901234567890";
        let profile = TotpProfile::google_authenticator_default();
        let current_time = 1_234_567_890;
        let old_code = generate(seed, current_time - 60, profile).expect("code generates");
        let future_code = generate(seed, current_time + 60, profile).expect("code generates");

        assert_eq!(
            verify(seed, current_time, &old_code, profile, None).expect("verify succeeds"),
            None
        );
        assert_eq!(
            verify(seed, current_time, &future_code, profile, None).expect("verify succeeds"),
            None
        );
    }

    #[test]
    fn accepting_future_step_burns_current_and_previous_steps() {
        let seed = b"12345678901234567890";
        let profile = TotpProfile::google_authenticator_default();
        let current_time = 1_234_567_890;
        let previous_code = generate(seed, current_time - 30, profile).expect("code generates");
        let current_code = generate(seed, current_time, profile).expect("code generates");
        let next_code = generate(seed, current_time + 30, profile).expect("code generates");

        let accepted_step = verify(seed, current_time, &next_code, profile, None)
            .expect("verify succeeds")
            .expect("next step is in the accepted window");

        assert_eq!(
            verify(
                seed,
                current_time,
                &current_code,
                profile,
                Some(accepted_step),
            )
            .expect("verify succeeds"),
            None
        );
        assert_eq!(
            verify(
                seed,
                current_time,
                &previous_code,
                profile,
                Some(accepted_step),
            )
            .expect("verify succeeds"),
            None
        );
    }

    #[test]
    fn rejects_invalid_code_shape() {
        assert_eq!(
            verify(
                b"12345678901234567890",
                59,
                "12-456",
                TotpProfile::google_authenticator_default(),
                None,
            )
            .expect_err("invalid shape fails"),
            TotpError::InvalidCode
        );
    }

    #[test]
    fn rejects_short_seed() {
        assert_eq!(
            generate(b"short", 59, TotpProfile::google_authenticator_default())
                .expect_err("short seed fails"),
            TotpError::InvalidSeedLength {
                minimum: MIN_SEED_BYTES,
                actual: 5,
            }
        );
        assert_eq!(
            verify(
                b"short",
                59,
                "123456",
                TotpProfile::google_authenticator_default(),
                None,
            )
            .expect_err("short seed fails"),
            TotpError::InvalidSeedLength {
                minimum: MIN_SEED_BYTES,
                actual: 5,
            }
        );
        assert_eq!(
            provisioning_uri(
                "Password Vault",
                "user@example.com",
                b"short",
                TotpProfile::google_authenticator_default(),
            )
            .expect_err("short seed fails"),
            TotpError::InvalidSeedLength {
                minimum: MIN_SEED_BYTES,
                actual: 5,
            }
        );
    }

    #[test]
    fn base32_and_uri_match_google_authenticator_shape() {
        let seed = b"12345678901234567890";
        let secret = base32_no_padding(seed);
        assert_eq!(secret, "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ");

        let uri = provisioning_uri(
            "Password Vault",
            "user@example.com",
            seed,
            TotpProfile::google_authenticator_default(),
        )
        .expect("URI builds");

        assert_eq!(
            uri,
            "otpauth://totp/Password%20Vault:user%40example.com?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ&issuer=Password%20Vault&algorithm=SHA1&digits=6&period=30"
        );
    }
}
