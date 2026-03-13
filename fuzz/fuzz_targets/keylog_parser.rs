#![no_main]

use libfuzzer_sys::fuzz_target;
use prb_pcap::tls::keylog::TlsKeyLog;

fuzz_target!(|data: &[u8]| {
    // Convert fuzz input to string (UTF-8)
    if let Ok(s) = std::str::from_utf8(data) {
        let mut keylog = TlsKeyLog::new();

        // Fuzz individual line parsing
        let _ = keylog.parse_line(s);

        // Also fuzz multi-line parsing (simulate file content)
        for line in s.lines() {
            let _ = keylog.parse_line(line);
        }
    }

    // Fuzz DSB key merging (can handle raw bytes)
    let mut keylog = TlsKeyLog::new();
    let _ = keylog.merge_dsb_keys(data);
});
