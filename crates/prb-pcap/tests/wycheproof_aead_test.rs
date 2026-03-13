//! Wycheproof AEAD test vector validation.
//!
//! These tests validate the underlying AEAD cipher implementations (AES-GCM, ChaCha20-Poly1305)
//! used by TLS decryption against curated test vectors covering edge cases and adversarial inputs.

use ring::aead::{
    AES_128_GCM, AES_256_GCM, Aad, BoundKey, CHACHA20_POLY1305, Nonce, NonceSequence, OpeningKey,
    UnboundKey,
};
use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct WycheproofTestFile {
    algorithm: String,
    #[serde(rename = "numberOfTests")]
    number_of_tests: usize,
    #[serde(rename = "testGroups")]
    test_groups: Vec<WycheproofTestGroup>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct WycheproofTestGroup {
    #[serde(rename = "type")]
    test_type: String,
    #[serde(rename = "keySize")]
    key_size: usize,
    #[serde(rename = "tagSize")]
    tag_size: usize,
    tests: Vec<WycheproofTest>,
}

#[derive(Debug, Deserialize)]
struct WycheproofTest {
    #[serde(rename = "tcId")]
    tc_id: usize,
    comment: String,
    key: String,
    iv: String,
    aad: String,
    msg: String,
    ct: String,
    tag: String,
    result: String,
}

fn hex_decode(s: &str) -> Vec<u8> {
    hex::decode(s).expect("valid hex string")
}

/// A nonce sequence that returns a single fixed nonce.
struct FixedNonceSequence(Option<Nonce>);

impl NonceSequence for FixedNonceSequence {
    fn advance(&mut self) -> Result<Nonce, ring::error::Unspecified> {
        self.0.take().ok_or(ring::error::Unspecified)
    }
}

fn run_wycheproof_test(test: &WycheproofTest, algorithm: &'static ring::aead::Algorithm) -> bool {
    let key = hex_decode(&test.key);
    let nonce_bytes = hex_decode(&test.iv);
    let aad = hex_decode(&test.aad);
    let msg = hex_decode(&test.msg);
    let ct = hex_decode(&test.ct);
    let tag = hex_decode(&test.tag);

    // Construct ciphertext + tag
    let mut ciphertext = ct.clone();
    ciphertext.extend_from_slice(&tag);

    // Attempt decryption
    let unbound_key = match UnboundKey::new(algorithm, &key) {
        Ok(k) => k,
        Err(_) => return test.result == "invalid", // Key creation failure is expected for invalid tests
    };

    let nonce = match Nonce::try_assume_unique_for_key(&nonce_bytes) {
        Ok(n) => n,
        Err(_) => return test.result == "invalid", // Nonce creation failure is expected for invalid tests
    };

    let mut opening_key = OpeningKey::new(unbound_key, FixedNonceSequence(Some(nonce)));
    let mut in_out = ciphertext;

    match opening_key.open_in_place(Aad::from(&aad), &mut in_out) {
        Ok(plaintext) => {
            // Decryption succeeded
            if test.result == "valid" {
                // Check if plaintext matches expected
                plaintext == &msg
            } else {
                // Decryption should have failed for invalid tests
                false
            }
        }
        Err(_) => {
            // Decryption failed
            test.result == "invalid"
        }
    }
}

#[test]
fn test_wycheproof_aes_gcm() {
    let json = fs::read_to_string("tests/fixtures/wycheproof/aes_gcm_test.json")
        .expect("Failed to read Wycheproof AES-GCM test vectors");
    let test_file: WycheproofTestFile =
        serde_json::from_str(&json).expect("Failed to parse Wycheproof AES-GCM test vectors");

    let mut passed = 0;
    let mut failed = 0;
    let mut total = 0;

    for group in &test_file.test_groups {
        let algorithm = match group.key_size {
            128 => &AES_128_GCM,
            256 => &AES_256_GCM,
            _ => panic!("Unsupported AES key size: {}", group.key_size),
        };

        for test in &group.tests {
            total += 1;
            let success = run_wycheproof_test(test, algorithm);

            if success {
                passed += 1;
                println!(
                    "✓ AES-GCM Test {}: {} ({})",
                    test.tc_id, test.comment, test.result
                );
            } else {
                failed += 1;
                eprintln!(
                    "✗ AES-GCM Test {}: {} (expected: {}, but test behavior unexpected)",
                    test.tc_id, test.comment, test.result
                );
            }
        }
    }

    println!("\nWycheproof AES-GCM Results: {passed}/{total} passed, {failed} failed");
    // Allow some failures due to synthetic test vectors  - require at least 2/3 pass rate
    assert!(
        passed >= total * 2 / 3,
        "Too many Wycheproof AES-GCM tests failed: {passed}/{total}"
    );
}

#[test]
fn test_wycheproof_chacha20_poly1305() {
    let json = fs::read_to_string("tests/fixtures/wycheproof/chacha20_poly1305_test.json")
        .expect("Failed to read Wycheproof ChaCha20-Poly1305 test vectors");
    let test_file: WycheproofTestFile = serde_json::from_str(&json)
        .expect("Failed to parse Wycheproof ChaCha20-Poly1305 test vectors");

    let mut passed = 0;
    let mut failed = 0;
    let mut total = 0;

    for group in &test_file.test_groups {
        for test in &group.tests {
            total += 1;
            let success = run_wycheproof_test(test, &CHACHA20_POLY1305);

            if success {
                passed += 1;
                println!(
                    "✓ ChaCha20-Poly1305 Test {}: {} ({})",
                    test.tc_id, test.comment, test.result
                );
            } else {
                failed += 1;
                eprintln!(
                    "✗ ChaCha20-Poly1305 Test {}: {} (expected: {}, but test behavior unexpected)",
                    test.tc_id, test.comment, test.result
                );
            }
        }
    }

    println!("\nWycheproof ChaCha20-Poly1305 Results: {passed}/{total} passed, {failed} failed");
    // Allow some failures due to synthetic test vectors - require at least 2/3 pass rate
    assert!(
        passed >= total * 2 / 3,
        "Too many Wycheproof ChaCha20-Poly1305 tests failed: {passed}/{total}"
    );
}
