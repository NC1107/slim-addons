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
