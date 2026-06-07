use uuid::Uuid;

use super::encoding::encode_base64url;

pub struct LoginAuthMessage<'a> {
    pub challenge_id: Uuid,
    pub auth_protocol: &'a str,
    pub login_handle_normalized: &'a str,
    pub client_nonce: &'a [u8],
    pub server_nonce: &'a [u8],
    pub client_final_without_proof: &'a [u8],
}

pub fn login_auth_message(input: LoginAuthMessage<'_>) -> Vec<u8> {
    let mut output = Vec::new();
    output.extend_from_slice(b"password-vault/login-auth-message/v1\n");
    push_field(&mut output, "challenge_id", &input.challenge_id.to_string());
    push_field(&mut output, "auth_protocol", input.auth_protocol);
    push_field(
        &mut output,
        "login_handle_normalized",
        input.login_handle_normalized,
    );
    push_field(
        &mut output,
        "client_nonce",
        &encode_base64url(input.client_nonce),
    );
    push_field(
        &mut output,
        "server_nonce",
        &encode_base64url(input.server_nonce),
    );
    push_field(
        &mut output,
        "client_final_without_proof",
        &encode_base64url(input.client_final_without_proof),
    );
    output
}

fn push_field(output: &mut Vec<u8>, name: &str, value: &str) {
    output.extend_from_slice(name.as_bytes());
    output.extend_from_slice(b"=");
    output.extend_from_slice(value.len().to_string().as_bytes());
    output.extend_from_slice(b":");
    output.extend_from_slice(value.as_bytes());
    output.extend_from_slice(b"\n");
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::{LoginAuthMessage, login_auth_message};

    #[test]
    fn login_auth_message_is_stable_and_length_prefixed() {
        let message = login_auth_message(LoginAuthMessage {
            challenge_id: Uuid::parse_str("00000000-0000-4000-8000-000000000020")
                .expect("test UUID parses"),
            auth_protocol: "derived-auth-v1",
            login_handle_normalized: "user@example.com",
            client_nonce: &[0x11; 32],
            server_nonce: &[0x22; 32],
            client_final_without_proof: b"c=biws,r=nonce",
        });

        assert_eq!(
            String::from_utf8(message).expect("message is UTF-8"),
            concat!(
                "password-vault/login-auth-message/v1\n",
                "challenge_id=36:00000000-0000-4000-8000-000000000020\n",
                "auth_protocol=15:derived-auth-v1\n",
                "login_handle_normalized=16:user@example.com\n",
                "client_nonce=43:ERERERERERERERERERERERERERERERERERERERERERE\n",
                "server_nonce=43:IiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiI\n",
                "client_final_without_proof=19:Yz1iaXdzLHI9bm9uY2U\n",
            )
        );
    }
}
