use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

#[derive(Debug, Eq, PartialEq)]
pub enum Base64UrlError {
    InvalidEncoding,
    InvalidLength { expected: usize, actual: usize },
}

impl std::fmt::Display for Base64UrlError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidEncoding => write!(formatter, "invalid base64url encoding"),
            Self::InvalidLength { expected, actual } => {
                write!(formatter, "expected {expected} bytes; got {actual}")
            }
        }
    }
}

impl std::error::Error for Base64UrlError {}

pub fn encode_base64url(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn decode_base64url(value: &str) -> Result<Vec<u8>, Base64UrlError> {
    URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| Base64UrlError::InvalidEncoding)
}

pub fn decode_base64url_array<const N: usize>(value: &str) -> Result<[u8; N], Base64UrlError> {
    let bytes = decode_base64url(value)?;
    if bytes.len() != N {
        return Err(Base64UrlError::InvalidLength {
            expected: N,
            actual: bytes.len(),
        });
    }

    let mut output = [0u8; N];
    output.copy_from_slice(&bytes);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::{Base64UrlError, decode_base64url_array, encode_base64url};

    #[test]
    fn round_trips_url_safe_without_padding() {
        let encoded = encode_base64url(&[0xfb, 0xff, 0xee]);

        assert_eq!(encoded, "-__u");
        assert_eq!(
            decode_base64url_array::<3>(&encoded).expect("valid base64url decodes"),
            [0xfb, 0xff, 0xee]
        );
    }

    #[test]
    fn rejects_wrong_fixed_length() {
        assert_eq!(
            decode_base64url_array::<4>("-__u").expect_err("length mismatch fails"),
            Base64UrlError::InvalidLength {
                expected: 4,
                actual: 3,
            }
        );
    }

    #[test]
    fn rejects_padding_and_standard_base64_only_chars() {
        assert_eq!(
            decode_base64url_array::<3>("-__u=").expect_err("padding fails"),
            Base64UrlError::InvalidEncoding
        );
        assert_eq!(
            decode_base64url_array::<3>("+//u").expect_err("standard alphabet fails"),
            Base64UrlError::InvalidEncoding
        );
    }
}
