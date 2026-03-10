# TLS Decryption

PRB can decrypt TLS-encrypted traffic to decode the application-layer protocols inside. This works with any protocol PRB supports (gRPC, ZMTP, DDS) when the traffic is wrapped in TLS.

## How It Works

PRB supports two methods of TLS decryption:

1. **SSLKEYLOGFILE** -- A text file containing per-session TLS master secrets, written by the application at runtime. This is the same format used by Wireshark.
2. **DSB (Decryption Secrets Block)** -- TLS keys embedded directly in a pcapng file. Some capture tools (e.g., Wireshark, tshark) can embed keys at capture time.

Both methods provide the ephemeral keys needed to decrypt TLS 1.2 and TLS 1.3 traffic without requiring the server's private key.

## SSLKEYLOGFILE Setup

Set the `SSLKEYLOGFILE` environment variable before starting your application. The application will write TLS session keys to this file as connections are established.

### Go

```bash
export SSLKEYLOGFILE=/tmp/sslkeys.log
```

In your Go code, configure the TLS transport to write keys:

```go
keylogFile, _ := os.OpenFile(os.Getenv("SSLKEYLOGFILE"), os.O_WRONLY|os.O_CREATE|os.O_APPEND, 0600)
tlsConfig := &tls.Config{
    KeyLogWriter: keylogFile,
}
```

For gRPC clients:

```go
creds := credentials.NewTLS(&tls.Config{KeyLogWriter: keylogFile})
conn, _ := grpc.Dial(addr, grpc.WithTransportCredentials(creds))
```

### Python

```bash
export SSLKEYLOGFILE=/tmp/sslkeys.log
```

Python's `ssl` module (3.8+) and libraries like `requests` and `grpcio` respect this variable automatically. No code changes needed.

For explicit control:

```python
import ssl
ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
ctx.keylog_filename = "/tmp/sslkeys.log"
```

### Node.js

```bash
export SSLKEYLOGFILE=/tmp/sslkeys.log
```

Node.js (12+) respects this variable natively for all TLS connections.

### Java

Java does not natively support SSLKEYLOGFILE. Use one of these approaches:

**jSSLKeyLog agent** (recommended):

```bash
java -javaagent:jSSLKeyLog.jar==/tmp/sslkeys.log -jar your-app.jar
```

Download from [github.com/nicnacnic/jSSLKeyLog](https://github.com/nicnacnic/jSSLKeyLog) or similar projects.

**javax.net.debug** (manual extraction):

```bash
java -Djavax.net.debug=ssl,keygen -jar your-app.jar 2> ssl-debug.log
```

This requires post-processing the debug log to extract keys.

### Rust

```bash
export SSLKEYLOGFILE=/tmp/sslkeys.log
```

With `rustls`:

```rust
use std::sync::Arc;
use rustls::{ClientConfig, KeyLogFile};

let config = ClientConfig::builder()
    .with_safe_defaults()
    .with_root_certificates(root_store)
    .with_no_client_auth();
config.key_log = Arc::new(KeyLogFile::new());
```

With `openssl` crate, configure the SSL context callback.

### C / C++

With OpenSSL:

```c
SSL_CTX_set_keylog_callback(ctx, keylog_callback);

void keylog_callback(const SSL *ssl, const char *line) {
    FILE *f = fopen(getenv("SSLKEYLOGFILE"), "a");
    fprintf(f, "%s\n", line);
    fclose(f);
}
```

With BoringSSL, the same API applies. With GnuTLS, use `gnutls_session_set_keylog_function()`.

## Using Keys with PRB

### During ingest

```bash
prb ingest capture.pcap --tls-keylog /tmp/sslkeys.log
```

### During live capture

```bash
sudo prb capture -i eth0 --tls-keylog /tmp/sslkeys.log --tui
```

### With pcapng DSB

If your pcapng file already contains embedded decryption secrets (DSB), PRB detects and uses them automatically -- no `--tls-keylog` flag needed:

```bash
prb ingest capture-with-keys.pcapng
```

## SSLKEYLOGFILE Format

The file format is one key per line:

```
CLIENT_RANDOM <client_random_hex> <master_secret_hex>
CLIENT_HANDSHAKE_TRAFFIC_SECRET <client_random_hex> <secret_hex>
SERVER_HANDSHAKE_TRAFFIC_SECRET <client_random_hex> <secret_hex>
CLIENT_TRAFFIC_SECRET_0 <client_random_hex> <secret_hex>
SERVER_TRAFFIC_SECRET_0 <client_random_hex> <secret_hex>
```

- `CLIENT_RANDOM` lines are for TLS 1.2
- `*_TRAFFIC_SECRET_*` lines are for TLS 1.3

Lines starting with `#` are comments. Blank lines are ignored.

## Troubleshooting

### No decrypted events

1. **Verify the keylog file has content**: `wc -l /tmp/sslkeys.log`
2. **Check timing**: the keylog must be generated during the same TLS sessions as the capture. Keys generated before or after the capture will not match.
3. **Check TLS version support**: PRB supports TLS 1.2 and 1.3. Older versions are not supported.
4. **Verify the capture contains the TLS handshake**: decryption requires seeing the ClientHello to match `CLIENT_RANDOM` values. If you started capturing after the handshake, the keys cannot be matched.

### Partial decryption

- Some connections may use session resumption (TLS session tickets). Ensure the keylog file covers the original handshake.
- For TLS 1.3 0-RTT (early data), additional key lines may be needed.

### Performance

TLS decryption adds overhead to the pipeline. For very large captures, consider:
- Using `--jobs` to increase parallel workers
- Pre-filtering with BPF at capture time to reduce traffic volume
- Decrypting only specific connections by combining with `--protocol` or query filters
