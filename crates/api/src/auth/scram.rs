use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

type HmacSha256 = Hmac<Sha256>;

pub const PROFILE_ID: &str = "pv-scram-sha-256-v1";
pub const KEY_LEN: usize = 32;
pub const MIN_ITERATIONS: u32 = 4096;
pub const DEFAULT_ITERATIONS: u32 = 150_000;
pub const MAX_ITERATIONS: u32 = 1_000_000;
pub const MIN_SALT_BYTES: usize = 16;
pub const DEFAULT_SALT_BYTES: usize = 32;

#[derive(Clone)]
pub struct ScramVerifier {
    stored_key: [u8; KEY_LEN],
    server_key: [u8; KEY_LEN],
}

impl ScramVerifier {
    /// Returns verifier material that must not be logged or serialized outside encrypted storage.
    pub fn stored_key(&self) -> &[u8; KEY_LEN] {
        &self.stored_key
    }

    /// Returns verifier material that must not be logged or serialized outside encrypted storage.
    pub fn server_key(&self) -> &[u8; KEY_LEN] {
        &self.server_key
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum ScramError {
    IterationsTooLow { minimum: u32, actual: u32 },
    IterationsTooHigh { maximum: u32, actual: u32 },
    SaltTooShort { minimum: usize, actual: usize },
    InvalidProofLength,
}

impl std::fmt::Display for ScramError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IterationsTooLow { minimum, actual } => write!(
                formatter,
                "SCRAM iterations must be at least {minimum}; got {actual}"
            ),
            Self::IterationsTooHigh { maximum, actual } => write!(
                formatter,
                "SCRAM iterations must not exceed {maximum}; got {actual}"
            ),
            Self::SaltTooShort { minimum, actual } => {
                write!(
                    formatter,
                    "SCRAM salt must be at least {minimum} bytes; got {actual}"
                )
            }
            Self::InvalidProofLength => write!(formatter, "SCRAM proof must be 32 bytes"),
        }
    }
}

impl std::error::Error for ScramError {}

pub fn derive_verifier(
    client_auth_secret: &[u8],
    salt: &[u8],
    iterations: u32,
) -> Result<ScramVerifier, ScramError> {
    let salted_password = salted_password(client_auth_secret, salt, iterations)?;
    let client_key = hmac_sha256(&salted_password, b"Client Key");
    let stored_key = sha256(&client_key);
    let server_key = hmac_sha256(&salted_password, b"Server Key");

    Ok(ScramVerifier {
        stored_key,
        server_key,
    })
}

pub fn client_proof(
    client_auth_secret: &[u8],
    salt: &[u8],
    iterations: u32,
    auth_message: &[u8],
) -> Result<[u8; KEY_LEN], ScramError> {
    let salted_password = salted_password(client_auth_secret, salt, iterations)?;
    let client_key = hmac_sha256(&salted_password, b"Client Key");
    let stored_key = sha256(&client_key);
    let client_signature = hmac_sha256(&stored_key, auth_message);

    Ok(xor_keys(&client_key, &client_signature))
}

pub fn server_signature(server_key: &[u8; KEY_LEN], auth_message: &[u8]) -> [u8; KEY_LEN] {
    hmac_sha256(server_key, auth_message)
}

pub fn verify_client_proof(
    stored_key: &[u8; KEY_LEN],
    auth_message: &[u8],
    proof: &[u8],
) -> Result<bool, ScramError> {
    let proof = proof
        .try_into()
        .map_err(|_| ScramError::InvalidProofLength)?;
    let client_signature = hmac_sha256(stored_key, auth_message);
    let client_key = xor_keys(proof, &client_signature);
    let candidate_stored_key = sha256(&client_key);

    Ok(candidate_stored_key.ct_eq(stored_key).into())
}

fn salted_password(
    client_auth_secret: &[u8],
    salt: &[u8],
    iterations: u32,
) -> Result<[u8; KEY_LEN], ScramError> {
    validate_iterations(iterations)?;
    validate_salt(salt)?;

    let mut block_input = Vec::with_capacity(salt.len() + 4);
    block_input.extend_from_slice(salt);
    block_input.extend_from_slice(&1u32.to_be_bytes());

    let mut u = hmac_sha256(client_auth_secret, &block_input);
    let mut output = u;

    for _ in 1..iterations {
        u = hmac_sha256(client_auth_secret, &u);
        for (left, right) in output.iter_mut().zip(u) {
            *left ^= right;
        }
    }

    Ok(output)
}

fn validate_iterations(iterations: u32) -> Result<(), ScramError> {
    if iterations < MIN_ITERATIONS {
        return Err(ScramError::IterationsTooLow {
            minimum: MIN_ITERATIONS,
            actual: iterations,
        });
    }
    if iterations > MAX_ITERATIONS {
        return Err(ScramError::IterationsTooHigh {
            maximum: MAX_ITERATIONS,
            actual: iterations,
        });
    }
    Ok(())
}

fn validate_salt(salt: &[u8]) -> Result<(), ScramError> {
    if salt.len() < MIN_SALT_BYTES {
        return Err(ScramError::SaltTooShort {
            minimum: MIN_SALT_BYTES,
            actual: salt.len(),
        });
    }
    Ok(())
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; KEY_LEN] {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts keys of any length");
    mac.update(data);
    mac.finalize().into_bytes().into()
}

fn sha256(data: &[u8]) -> [u8; KEY_LEN] {
    Sha256::digest(data).into()
}

fn xor_keys(left: &[u8; KEY_LEN], right: &[u8; KEY_LEN]) -> [u8; KEY_LEN] {
    let mut output = [0u8; KEY_LEN];
    for ((out, left), right) in output.iter_mut().zip(left).zip(right) {
        *out = left ^ right;
    }
    output
}

#[cfg(test)]
mod tests {
    use base64::{Engine as _, engine::general_purpose};

    use super::{
        MAX_ITERATIONS, MIN_ITERATIONS, MIN_SALT_BYTES, PROFILE_ID, ScramError, client_proof,
        derive_verifier, server_signature, verify_client_proof,
    };

    #[test]
    fn profile_id_is_stable() {
        assert_eq!(PROFILE_ID, "pv-scram-sha-256-v1");
    }

    #[test]
    fn matches_rfc7677_scram_sha256_example() {
        let password = b"pencil";
        let salt = general_purpose::STANDARD
            .decode("W22ZaJ0SNY7soEsUEjb6gQ==")
            .expect("RFC salt is valid base64");
        let iterations = 4096;
        let auth_message = b"n=user,r=rOprNGfwEbeRWgbNEkqO,r=rOprNGfwEbeRWgbNEkqO%hvYDpWUa2RaTCAfuxFIlj)hNlF$k0,s=W22ZaJ0SNY7soEsUEjb6gQ==,i=4096,c=biws,r=rOprNGfwEbeRWgbNEkqO%hvYDpWUa2RaTCAfuxFIlj)hNlF$k0";

        let verifier = derive_verifier(password, &salt, iterations).expect("verifier derives");
        let proof = client_proof(password, &salt, iterations, auth_message).expect("proof derives");
        let signature = server_signature(verifier.server_key(), auth_message);

        assert_eq!(
            general_purpose::STANDARD.encode(proof),
            "dHzbZapWIk4jUhN+Ute9ytag9zjfMHgsqmmiz7AndVQ="
        );
        assert_eq!(
            general_purpose::STANDARD.encode(signature),
            "6rriTRBi23WpRR/wtup+mMhUZUn/dB5nLTJRsjl95G4="
        );
        assert!(
            verify_client_proof(verifier.stored_key(), auth_message, &proof)
                .expect("proof length is valid")
        );
    }

    #[test]
    fn rejects_wrong_scram_proof() {
        let salt = b"1234567890123456";
        let verifier = derive_verifier(b"secret", salt, 4096).expect("verifier derives");
        let mut proof =
            client_proof(b"secret", salt, 4096, b"auth-message").expect("proof derives");
        proof[0] ^= 0xff;

        assert!(
            !verify_client_proof(verifier.stored_key(), b"auth-message", &proof)
                .expect("proof length is valid")
        );
    }

    #[test]
    fn rejects_iterations_below_minimum() {
        assert!(matches!(
            derive_verifier(b"secret", b"1234567890123456", MIN_ITERATIONS - 1),
            Err(ScramError::IterationsTooLow {
                minimum: MIN_ITERATIONS,
                actual: 4095,
            })
        ));
    }

    #[test]
    fn rejects_iterations_above_maximum() {
        assert!(matches!(
            derive_verifier(b"secret", b"1234567890123456", MAX_ITERATIONS + 1),
            Err(ScramError::IterationsTooHigh {
                maximum: MAX_ITERATIONS,
                actual: 1_000_001,
            })
        ));
    }

    #[test]
    fn rejects_short_salt() {
        assert!(matches!(
            derive_verifier(b"secret", b"short", MIN_ITERATIONS),
            Err(ScramError::SaltTooShort {
                minimum: MIN_SALT_BYTES,
                actual: 5,
            })
        ));
    }
}
