use rand::{TryRngCore, rngs::OsRng};
use sha2::{Digest, Sha256};

pub const TOKEN_LEN: usize = 32;

pub fn random_token() -> [u8; TOKEN_LEN] {
    let mut token = [0u8; TOKEN_LEN];
    OsRng
        .try_fill_bytes(&mut token)
        .expect("OS random generator must be available for session tokens");
    token
}

/// Hashes high-entropy random tokens only. Do not use for passwords or recovery codes.
pub fn sha256_verifier(secret: &[u8]) -> [u8; TOKEN_LEN] {
    Sha256::digest(secret).into()
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use super::{TOKEN_LEN, random_token, sha256_verifier};

    #[test]
    fn random_token_is_32_bytes_and_changes() {
        let first = random_token();
        let second = random_token();

        assert_eq!(first.len(), TOKEN_LEN);
        assert_ne!(first, second);
    }

    #[test]
    fn verifier_is_stable_sha256() {
        let verifier = sha256_verifier(b"session-token");

        assert_eq!(
            hex_lower(&verifier),
            "c101e911469c969171040b50d70543313cf968fdef5bacc780776f8fb399ab36"
        );
    }

    fn hex_lower(bytes: &[u8]) -> String {
        let mut output = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
        }
        output
    }
}
