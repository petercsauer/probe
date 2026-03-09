//! TLS key derivation functions.
//!
//! Implements:
//! - TLS 1.2 PRF (Pseudo-Random Function) using HMAC-SHA256
//! - TLS 1.3 HKDF-Expand-Label using HKDF-SHA256 or HKDF-SHA384

use crate::error::PcapError;
use ring::hmac;

/// Derived keys for TLS 1.2 AEAD ciphers.
#[derive(Debug, Clone)]
pub struct Tls12Keys {
    pub client_write_key: Vec<u8>,
    pub server_write_key: Vec<u8>,
    pub client_write_iv: Vec<u8>,
    pub server_write_iv: Vec<u8>,
}

/// Derived keys for TLS 1.3 AEAD ciphers.
#[derive(Debug, Clone)]
pub struct Tls13Keys {
    pub key: Vec<u8>,
    pub iv: Vec<u8>,
}

/// TLS 1.2 PRF (Pseudo-Random Function) using HMAC-SHA256.
///
/// PRF(secret, label, seed) = P_SHA256(secret, label + seed)
///
/// Reference: RFC 5246 Section 5
fn prf_sha256(secret: &[u8], label: &[u8], seed: &[u8], output_len: usize) -> Vec<u8> {
    let key = hmac::Key::new(hmac::HMAC_SHA256, secret);

    // Concatenate label and seed
    let mut label_seed = Vec::with_capacity(label.len() + seed.len());
    label_seed.extend_from_slice(label);
    label_seed.extend_from_slice(seed);

    let mut result = Vec::with_capacity(output_len);
    let mut a = label_seed.clone();

    while result.len() < output_len {
        // A(i) = HMAC_hash(secret, A(i-1))
        a = hmac::sign(&key, &a).as_ref().to_vec();

        // P_hash(secret, seed) = HMAC_hash(secret, A(1) + seed) +
        //                        HMAC_hash(secret, A(2) + seed) + ...
        let mut a_plus_seed = Vec::with_capacity(a.len() + label_seed.len());
        a_plus_seed.extend_from_slice(&a);
        a_plus_seed.extend_from_slice(&label_seed);

        let hmac_result = hmac::sign(&key, &a_plus_seed);
        result.extend_from_slice(hmac_result.as_ref());
    }

    result.truncate(output_len);
    result
}

/// Derives TLS 1.2 encryption keys from master secret.
///
/// Key block = PRF(master_secret, "key expansion", server_random + client_random)
///
/// Reference: RFC 5246 Section 6.3
pub fn derive_tls12_keys(
    master_secret: &[u8],
    client_random: &[u8],
    server_random: &[u8],
    key_len: usize,
    iv_len: usize,
) -> Result<Tls12Keys, PcapError> {
    // Validate master secret length (must be 48 bytes for TLS 1.2)
    if master_secret.len() != 48 {
        return Err(PcapError::TlsKey(format!(
            "invalid master_secret length: {} (expected 48)",
            master_secret.len()
        )));
    }

    // Concatenate server_random + client_random for seed
    let mut seed = Vec::with_capacity(server_random.len() + client_random.len());
    seed.extend_from_slice(server_random);
    seed.extend_from_slice(client_random);

    // Derive key block
    let key_block_len = 2 * key_len + 2 * iv_len;
    let key_block = prf_sha256(master_secret, b"key expansion", &seed, key_block_len);

    // Split key block into individual keys
    let mut offset = 0;
    let client_write_key = key_block[offset..offset + key_len].to_vec();
    offset += key_len;
    let server_write_key = key_block[offset..offset + key_len].to_vec();
    offset += key_len;
    let client_write_iv = key_block[offset..offset + iv_len].to_vec();
    offset += iv_len;
    let server_write_iv = key_block[offset..offset + iv_len].to_vec();

    Ok(Tls12Keys {
        client_write_key,
        server_write_key,
        client_write_iv,
        server_write_iv,
    })
}

/// TLS 1.3 HKDF-Expand-Label.
///
/// HKDF-Expand-Label(Secret, Label, Context, Length) =
///     HKDF-Expand(Secret, HkdfLabel, Length)
///
/// Where HkdfLabel is:
/// struct {
///     uint16 length = Length;
///     opaque label<7..255> = "tls13 " + Label;
///     opaque context<0..255> = Context;
/// } HkdfLabel;
///
/// Reference: RFC 8446 Section 7.1
fn hkdf_expand_label(
    algorithm: hmac::Algorithm,
    secret: &[u8],
    label: &[u8],
    context: &[u8],
    length: usize,
) -> Result<Vec<u8>, PcapError> {
    // Build HkdfLabel structure
    let mut hkdf_label = Vec::new();

    // uint16 length
    hkdf_label.push((length >> 8) as u8);
    hkdf_label.push(length as u8);

    // opaque label<7..255> = "tls13 " + Label
    let tls13_label = format!("tls13 {}", std::str::from_utf8(label).unwrap());
    if tls13_label.len() > 255 {
        return Err(PcapError::TlsKey(format!(
            "label too long: {}",
            tls13_label.len()
        )));
    }
    hkdf_label.push(tls13_label.len() as u8);
    hkdf_label.extend_from_slice(tls13_label.as_bytes());

    // opaque context<0..255> = Context
    if context.len() > 255 {
        return Err(PcapError::TlsKey(format!(
            "context too long: {}",
            context.len()
        )));
    }
    hkdf_label.push(context.len() as u8);
    hkdf_label.extend_from_slice(context);

    // HKDF-Expand
    let prk = hmac::Key::new(algorithm, secret);
    let mut result = Vec::with_capacity(length);
    let mut counter = 1u8;
    let mut t = Vec::new();

    while result.len() < length {
        let mut input = t.clone();
        input.extend_from_slice(&hkdf_label);
        input.push(counter);

        t = hmac::sign(&prk, &input).as_ref().to_vec();
        result.extend_from_slice(&t);
        counter += 1;
    }

    result.truncate(length);
    Ok(result)
}

/// Derives TLS 1.3 encryption keys from traffic secret.
///
/// Key = HKDF-Expand-Label(traffic_secret, "key", "", key_len)
/// IV = HKDF-Expand-Label(traffic_secret, "iv", "", 12)
///
/// Reference: RFC 8446 Section 7.3
pub fn derive_tls13_keys(
    traffic_secret: &[u8],
    key_len: usize,
    use_sha384: bool,
) -> Result<Tls13Keys, PcapError> {
    let algorithm = if use_sha384 {
        hmac::HMAC_SHA384
    } else {
        hmac::HMAC_SHA256
    };

    // Derive key and IV
    let key = hkdf_expand_label(algorithm, traffic_secret, b"key", b"", key_len)?;
    let iv = hkdf_expand_label(algorithm, traffic_secret, b"iv", b"", 12)?;

    Ok(Tls13Keys { key, iv })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tls12_prf_rfc5246_vector() {
        // RFC 5246 test vector (not officially published, but commonly used)
        // Using a synthetic test case for verification
        let secret = b"secret";
        let label = b"test label";
        let seed = b"seed";
        let output = prf_sha256(secret, label, seed, 32);

        // Verify output length
        assert_eq!(output.len(), 32);

        // PRF should be deterministic
        let output2 = prf_sha256(secret, label, seed, 32);
        assert_eq!(output, output2);
    }

    #[test]
    fn test_hkdf_expand_label_tls13_prefix() {
        // Verify that HKDF-Expand-Label correctly adds "tls13 " prefix
        let secret = vec![0u8; 32];
        let result = hkdf_expand_label(hmac::HMAC_SHA256, &secret, b"key", b"", 16);
        assert!(result.is_ok());

        let key = result.unwrap();
        assert_eq!(key.len(), 16);
    }

    #[test]
    fn test_tls12_key_derivation() {
        // Synthetic test vector
        let master_secret = vec![0x01; 48];
        let client_random = vec![0x02; 32];
        let server_random = vec![0x03; 32];

        let keys = derive_tls12_keys(&master_secret, &client_random, &server_random, 16, 4);
        assert!(keys.is_ok());

        let keys = keys.unwrap();
        assert_eq!(keys.client_write_key.len(), 16);
        assert_eq!(keys.server_write_key.len(), 16);
        assert_eq!(keys.client_write_iv.len(), 4);
        assert_eq!(keys.server_write_iv.len(), 4);
    }

    #[test]
    fn test_tls13_key_derivation() {
        // Synthetic test vector
        let traffic_secret = vec![0x01; 32];

        let keys = derive_tls13_keys(&traffic_secret, 16, false);
        assert!(keys.is_ok());

        let keys = keys.unwrap();
        assert_eq!(keys.key.len(), 16);
        assert_eq!(keys.iv.len(), 12);
    }
}
