//! Integration tests for TLS decryption module.

use prb_pcap::TlsStreamProcessor;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_keylog_parse() {
    use prb_pcap::tls::keylog::TlsKeyLog;

    let mut keylog = TlsKeyLog::new();

    let line = "CLIENT_RANDOM 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef \
                aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    keylog.parse_line(line).unwrap();
    assert_eq!(keylog.len(), 1);

    let client_random =
        hex::decode("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef").unwrap();
    let keys = keylog.lookup(&client_random);
    assert!(keys.is_some());
    assert!(keys.unwrap()[0].is_tls12());

    // All 5 TLS 1.3 labels
    let mut keylog2 = TlsKeyLog::new();
    let cr = "aa".repeat(32);
    let secret = "bb".repeat(32);
    keylog2
        .parse_line(&format!("CLIENT_HANDSHAKE_TRAFFIC_SECRET {cr} {secret}"))
        .unwrap();
    keylog2
        .parse_line(&format!("SERVER_HANDSHAKE_TRAFFIC_SECRET {cr} {secret}"))
        .unwrap();
    keylog2
        .parse_line(&format!("CLIENT_TRAFFIC_SECRET_0 {cr} {secret}"))
        .unwrap();
    keylog2
        .parse_line(&format!("SERVER_TRAFFIC_SECRET_0 {cr} {secret}"))
        .unwrap();
    assert_eq!(keylog2.len(), 1);
    let materials = keylog2.lookup(&hex::decode(&cr).unwrap()).unwrap();
    assert_eq!(materials.len(), 4);
}

#[test]
fn test_keylog_merge_dsb() {
    use prb_pcap::tls::keylog::TlsKeyLog;

    let mut keylog = TlsKeyLog::new();
    let dsb_data = b"CLIENT_RANDOM 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef \
        aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n";
    keylog.merge_dsb_keys(dsb_data).unwrap();
    assert_eq!(keylog.len(), 1);
}

#[test]
fn test_keylog_from_file() {
    use prb_pcap::tls::keylog::TlsKeyLog;

    let mut tmpfile = NamedTempFile::new().unwrap();
    writeln!(tmpfile, "# TLS Key Log File").unwrap();
    writeln!(
        tmpfile,
        "CLIENT_RANDOM 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    ).unwrap();
    writeln!(
        tmpfile,
        "CLIENT_TRAFFIC_SECRET_0 fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210 bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
    ).unwrap();
    let keylog = TlsKeyLog::from_file(tmpfile.path()).unwrap();
    assert_eq!(keylog.len(), 2);
}

#[test]
fn test_tls12_key_derivation() {
    use prb_pcap::tls::kdf::derive_tls12_keys;

    let master_secret = vec![0x01; 48];
    let client_random = vec![0x02; 32];
    let server_random = vec![0x03; 32];
    let keys = derive_tls12_keys(&master_secret, &client_random, &server_random, 16, 4).unwrap();
    assert_eq!(keys.client_write_key.len(), 16);
    assert_eq!(keys.server_write_key.len(), 16);
    assert_eq!(keys.client_write_iv.len(), 4);
    assert_eq!(keys.server_write_iv.len(), 4);

    let keys2 = derive_tls12_keys(&master_secret, &client_random, &server_random, 16, 4).unwrap();
    assert_eq!(keys.client_write_key, keys2.client_write_key);
}

#[test]
fn test_tls13_key_derivation() {
    use prb_pcap::tls::kdf::derive_tls13_keys;

    let traffic_secret = vec![0x01; 32];
    let keys = derive_tls13_keys(&traffic_secret, 16, false).unwrap();
    assert_eq!(keys.key.len(), 16);
    assert_eq!(keys.iv.len(), 12);

    let keys2 = derive_tls13_keys(&traffic_secret, 16, false).unwrap();
    assert_eq!(keys.key, keys2.key);
    assert_eq!(keys.iv, keys2.iv);
}

/// Helper: seal with an AEAD algorithm using ring.
fn aead_seal(
    algo: &'static ring::aead::Algorithm,
    key: &[u8],
    nonce: &[u8],
    aad: &[u8],
    plaintext: &[u8],
) -> Vec<u8> {
    use ring::aead::{Aad, BoundKey, Nonce, NonceSequence, SealingKey, UnboundKey};

    struct OneNonce(Option<Nonce>);
    impl NonceSequence for OneNonce {
        fn advance(&mut self) -> Result<Nonce, ring::error::Unspecified> {
            self.0.take().ok_or(ring::error::Unspecified)
        }
    }

    let unbound = UnboundKey::new(algo, key).unwrap();
    let nonce_val = Nonce::try_assume_unique_for_key(nonce).unwrap();
    let mut sealing_key = SealingKey::new(unbound, OneNonce(Some(nonce_val)));
    let mut in_out = plaintext.to_vec();
    sealing_key
        .seal_in_place_append_tag(Aad::from(aad), &mut in_out)
        .unwrap();
    in_out
}

#[test]
fn test_aes128gcm_decrypt_synthetic() {
    use prb_pcap::tls::decrypt::TlsDecryptor;
    use prb_pcap::tls::keylog::KeyMaterial;
    use prb_pcap::tls::session::SessionInfo;
    use tls_parser::TlsVersion;

    let traffic_secret = vec![0xaa; 32];
    let session = SessionInfo {
        client_random: vec![0u8; 32],
        server_random: vec![0u8; 32],
        cipher_suite_id: 0x1301,
        version: TlsVersion::Tls13,
    };
    let decryptor = TlsDecryptor::new(
        &session,
        &[KeyMaterial::ClientTrafficSecret0(traffic_secret.clone())],
    )
    .unwrap();

    let plaintext = b"Hello TLS 1.3 AES-128-GCM";
    let keys = prb_pcap::tls::kdf::derive_tls13_keys(&traffic_secret, 16, false).unwrap();
    let nonce = keys.iv.clone(); // seq=0 XOR IV = IV

    // TLS 1.3 AAD: content_type + version + ciphertext_length
    let ct_len = plaintext.len() + 16; // plaintext + 16-byte tag
    let aad = vec![0x17, 0x03, 0x03, (ct_len >> 8) as u8, ct_len as u8];
    let ciphertext = aead_seal(&ring::aead::AES_128_GCM, &keys.key, &nonce, &aad, plaintext);

    let result = decryptor
        .decrypt_aead(
            &ciphertext,
            0,
            0x17,
            0x0303,
            prb_pcap::tcp::StreamDirection::ClientToServer,
        )
        .unwrap();
    assert_eq!(result, plaintext);
}

#[test]
fn test_aes256gcm_decrypt_synthetic() {
    use prb_pcap::tls::decrypt::TlsDecryptor;
    use prb_pcap::tls::keylog::KeyMaterial;
    use prb_pcap::tls::session::SessionInfo;
    use tls_parser::TlsVersion;

    let traffic_secret = vec![0xbb; 48];
    let session = SessionInfo {
        client_random: vec![0u8; 32],
        server_random: vec![0u8; 32],
        cipher_suite_id: 0x1302,
        version: TlsVersion::Tls13,
    };
    let decryptor = TlsDecryptor::new(
        &session,
        &[KeyMaterial::ClientTrafficSecret0(traffic_secret.clone())],
    )
    .unwrap();

    let plaintext = b"Hello TLS 1.3 AES-256-GCM";
    let keys = prb_pcap::tls::kdf::derive_tls13_keys(&traffic_secret, 32, true).unwrap();
    let nonce = keys.iv.clone();

    let ct_len = plaintext.len() + 16;
    let aad = vec![0x17, 0x03, 0x03, (ct_len >> 8) as u8, ct_len as u8];
    let ciphertext = aead_seal(&ring::aead::AES_256_GCM, &keys.key, &nonce, &aad, plaintext);

    let result = decryptor
        .decrypt_aead(
            &ciphertext,
            0,
            0x17,
            0x0303,
            prb_pcap::tcp::StreamDirection::ClientToServer,
        )
        .unwrap();
    assert_eq!(result, plaintext);
}

#[test]
fn test_chacha20poly1305_decrypt_synthetic() {
    use prb_pcap::tls::decrypt::TlsDecryptor;
    use prb_pcap::tls::keylog::KeyMaterial;
    use prb_pcap::tls::session::SessionInfo;
    use tls_parser::TlsVersion;

    let traffic_secret = vec![0xcc; 32];
    let session = SessionInfo {
        client_random: vec![0u8; 32],
        server_random: vec![0u8; 32],
        cipher_suite_id: 0x1303,
        version: TlsVersion::Tls13,
    };
    let decryptor = TlsDecryptor::new(
        &session,
        &[KeyMaterial::ClientTrafficSecret0(traffic_secret.clone())],
    )
    .unwrap();

    let plaintext = b"Hello TLS 1.3 ChaCha20-Poly1305";
    let keys = prb_pcap::tls::kdf::derive_tls13_keys(&traffic_secret, 32, false).unwrap();
    let nonce = keys.iv.clone();

    let ct_len = plaintext.len() + 16;
    let aad = vec![0x17, 0x03, 0x03, (ct_len >> 8) as u8, ct_len as u8];
    let ciphertext = aead_seal(
        &ring::aead::CHACHA20_POLY1305,
        &keys.key,
        &nonce,
        &aad,
        plaintext,
    );

    let result = decryptor
        .decrypt_aead(
            &ciphertext,
            0,
            0x17,
            0x0303,
            prb_pcap::tcp::StreamDirection::ClientToServer,
        )
        .unwrap();
    assert_eq!(result, plaintext);
}

#[test]
fn test_session_identification() {
    use prb_pcap::tls::session::TlsSession;

    let client_random: Vec<u8> = (0..32).collect();
    let server_random: Vec<u8> = (32..64).collect();

    // ClientHello
    let mut ch_body = Vec::new();
    ch_body.extend_from_slice(&[0x03, 0x03]); // TLS 1.2
    ch_body.extend_from_slice(&client_random);
    ch_body.push(0x00); // session_id length
    ch_body.extend_from_slice(&[0x00, 0x02, 0x13, 0x01]); // cipher suites
    ch_body.push(0x01);
    ch_body.push(0x00); // compression

    let mut ch_hs = vec![0x01, 0x00];
    ch_hs.extend_from_slice(&(ch_body.len() as u16).to_be_bytes());
    ch_hs.extend_from_slice(&ch_body);
    let mut ch_rec = vec![0x16, 0x03, 0x01];
    ch_rec.extend_from_slice(&(ch_hs.len() as u16).to_be_bytes());
    ch_rec.extend_from_slice(&ch_hs);

    // ServerHello
    let mut sh_body = Vec::new();
    sh_body.extend_from_slice(&[0x03, 0x03]);
    sh_body.extend_from_slice(&server_random);
    sh_body.push(0x00); // session_id length
    sh_body.extend_from_slice(&[0x13, 0x01]); // cipher suite
    sh_body.push(0x00); // compression

    let mut sh_hs = vec![0x02, 0x00];
    sh_hs.extend_from_slice(&(sh_body.len() as u16).to_be_bytes());
    sh_hs.extend_from_slice(&sh_body);
    let mut sh_rec = vec![0x16, 0x03, 0x03];
    sh_rec.extend_from_slice(&(sh_hs.len() as u16).to_be_bytes());
    sh_rec.extend_from_slice(&sh_hs);

    let mut stream = ch_rec;
    stream.extend_from_slice(&sh_rec);

    let session = TlsSession::from_stream(&stream).unwrap();
    assert_eq!(session.client_random, client_random);
    assert_eq!(session.server_random, server_random);
    assert_eq!(session.cipher_suite_id, 0x1301);
}

#[test]
fn test_no_key_passthrough() {
    use prb_pcap::tcp::{ReassembledStream, StreamDirection};
    use std::net::IpAddr;

    let processor = TlsStreamProcessor::new();
    let stream = ReassembledStream {
        src_ip: IpAddr::from([127, 0, 0, 1]),
        src_port: 12345,
        dst_ip: IpAddr::from([127, 0, 0, 1]),
        dst_port: 443,
        direction: StreamDirection::ClientToServer,
        data: vec![0x17, 0x03, 0x03, 0x00, 0x10],
        is_complete: false,
        missing_ranges: vec![],
        timestamp_us: 1000000,
    };
    let decrypted = processor.decrypt_stream(stream).unwrap();
    assert!(decrypted.encrypted);
}

#[test]
fn test_end_to_end_tls12_synthetic() {
    use prb_pcap::tls::decrypt::TlsDecryptor;
    use prb_pcap::tls::keylog::KeyMaterial;
    use prb_pcap::tls::session::SessionInfo;
    use tls_parser::TlsVersion;

    let client_random = vec![0x11; 32];
    let server_random = vec![0x22; 32];
    let master_secret = vec![0x33; 48];

    let session = SessionInfo {
        client_random: client_random.clone(),
        server_random: server_random.clone(),
        cipher_suite_id: 0xC02F, // TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256
        version: TlsVersion::Tls12,
    };
    let decryptor = TlsDecryptor::new(
        &session,
        &[KeyMaterial::MasterSecret(master_secret.clone())],
    )
    .unwrap();

    let keys = prb_pcap::tls::kdf::derive_tls12_keys(
        &master_secret,
        &client_random,
        &server_random,
        16,
        4,
    )
    .unwrap();

    let plaintext = b"TLS 1.2 test payload";
    let explicit_nonce: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 1];

    // 12-byte nonce: implicit_iv(4) + explicit_nonce(8)
    let mut nonce = keys.client_write_iv.clone();
    nonce.extend_from_slice(&explicit_nonce);

    // TLS 1.2 AAD: seq_num(8) + content_type(1) + version(2) + plaintext_length(2)
    let seq: u64 = 0;
    let mut aad = Vec::with_capacity(13);
    aad.extend_from_slice(&seq.to_be_bytes());
    aad.push(0x17);
    aad.extend_from_slice(&[0x03, 0x03]);
    aad.extend_from_slice(&(plaintext.len() as u16).to_be_bytes());

    let ciphertext_and_tag = aead_seal(
        &ring::aead::AES_128_GCM,
        &keys.client_write_key,
        &nonce,
        &aad,
        plaintext,
    );

    // TLS 1.2 record payload: explicit_nonce(8) + ciphertext + tag
    let mut record_payload = Vec::new();
    record_payload.extend_from_slice(&explicit_nonce);
    record_payload.extend_from_slice(&ciphertext_and_tag);

    // Decrypt via direct API
    let result = decryptor
        .decrypt_aead(
            &record_payload,
            0,
            0x17,
            0x0303,
            prb_pcap::tcp::StreamDirection::ClientToServer,
        )
        .unwrap();
    assert_eq!(result, plaintext);
}

#[test]
fn test_end_to_end_tls13_synthetic() {
    use prb_pcap::tls::decrypt::TlsDecryptor;
    use prb_pcap::tls::keylog::KeyMaterial;
    use prb_pcap::tls::session::SessionInfo;
    use tls_parser::TlsVersion;

    let client_secret = vec![0xaa; 32];
    let server_secret = vec![0xbb; 32];
    let session = SessionInfo {
        client_random: vec![0u8; 32],
        server_random: vec![0u8; 32],
        cipher_suite_id: 0x1301,
        version: TlsVersion::Tls13,
    };
    let decryptor = TlsDecryptor::new(
        &session,
        &[
            KeyMaterial::ClientTrafficSecret0(client_secret.clone()),
            KeyMaterial::ServerTrafficSecret0(server_secret.clone()),
        ],
    )
    .unwrap();

    // Client-to-server direction
    let plaintext_c2s = b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n";
    let keys_c = prb_pcap::tls::kdf::derive_tls13_keys(&client_secret, 16, false).unwrap();
    let nonce_c = keys_c.iv.clone();
    let ct_len = plaintext_c2s.len() + 16;
    let aad_c = vec![0x17, 0x03, 0x03, (ct_len >> 8) as u8, ct_len as u8];
    let ct_c = aead_seal(
        &ring::aead::AES_128_GCM,
        &keys_c.key,
        &nonce_c,
        &aad_c,
        plaintext_c2s,
    );

    let result_c = decryptor
        .decrypt_aead(
            &ct_c,
            0,
            0x17,
            0x0303,
            prb_pcap::tcp::StreamDirection::ClientToServer,
        )
        .unwrap();
    assert_eq!(result_c, plaintext_c2s);

    // Server-to-client direction (uses server keys)
    let plaintext_s2c = b"HTTP/1.1 200 OK\r\n\r\n";
    let keys_s = prb_pcap::tls::kdf::derive_tls13_keys(&server_secret, 16, false).unwrap();
    let nonce_s = keys_s.iv.clone();
    let ct_len_s = plaintext_s2c.len() + 16;
    let aad_s = vec![0x17, 0x03, 0x03, (ct_len_s >> 8) as u8, ct_len_s as u8];
    let ct_s = aead_seal(
        &ring::aead::AES_128_GCM,
        &keys_s.key,
        &nonce_s,
        &aad_s,
        plaintext_s2c,
    );

    let result_s = decryptor
        .decrypt_aead(
            &ct_s,
            0,
            0x17,
            0x0303,
            prb_pcap::tcp::StreamDirection::ServerToClient,
        )
        .unwrap();
    assert_eq!(result_s, plaintext_s2c);
}
