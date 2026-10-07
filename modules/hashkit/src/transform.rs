//! Dispatches a hashkit command (`sha256`, `base64`, `hex`) to its transform.
//! Each transform encodes the input string; none of them can fail on their
//! own input, so the only error case is an unrecognized command name.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use sha2::{Digest, Sha256};

pub fn apply(command: &str, input: &str) -> Result<String, String> {
    match command {
        "sha256" => Ok(sha256_hex(input)),
        "base64" => Ok(STANDARD.encode(input.as_bytes())),
        "hex" => Ok(to_hex(input.as_bytes())),
        other => Err(format!("unknown command: {other}")),
    }
}

fn sha256_hex(input: &str) -> String {
    let digest = Sha256::digest(input.as_bytes());
    to_hex(&digest)
}

fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::apply;

    #[test]
    fn sha256_of_abc_is_the_published_digest() {
        assert_eq!(
            apply("sha256", "abc").unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn sha256_of_nothing_is_the_empty_digest() {
        assert_eq!(
            apply("sha256", "").unwrap(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn base64_pads_and_encodes_utf8_bytes() {
        assert_eq!(apply("base64", "hi").unwrap(), "aGk=");
        assert_eq!(apply("base64", "\u{e9}").unwrap(), "w6k=");
    }

    #[test]
    fn hex_is_lowercase_two_digits_per_byte() {
        assert_eq!(apply("hex", "hi").unwrap(), "6869");
        assert_eq!(apply("hex", "\n\u{7f}").unwrap(), "0a7f");
    }

    #[test]
    fn an_unknown_command_is_an_error() {
        assert_eq!(apply("md5", "x").unwrap_err(), "unknown command: md5");
    }
}
