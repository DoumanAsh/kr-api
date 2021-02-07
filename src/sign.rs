use ring::digest::SHA256;
use ring::hmac::HMAC_SHA512;
use data_encoding::BASE64;

//Signature components:
// - Path;
// - SHA256 of (nonce, POST payload);
// - BASE64 decoded private key;
//
// It must be signed using hmac sha512 and encoded as base64
pub fn generate(path: &str, payload: &[u8], nonce: u64, api_key: &[u8]) -> String {
    let mut input = nonce.to_string().into_bytes();
    input.extend_from_slice(payload);
    let digest = ring::digest::digest(&SHA256, &input);

    input.clear();
    input.extend_from_slice(b"/0/");
    input.extend_from_slice(path.as_bytes());
    input.extend_from_slice(digest.as_ref());

    let key = ring::hmac::Key::new(HMAC_SHA512, api_key);
    let result = ring::hmac::sign(&key, &input);
    BASE64.encode(result.as_ref())
}
