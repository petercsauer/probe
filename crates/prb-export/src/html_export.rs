use crate::{ExportError, Exporter};
use prb_core::DebugEvent;
use std::io::Write;

pub struct HtmlExporter;

const HTML_TEMPLATE: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Probe Report</title>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body {
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
            background: #1a1a1a;
            color: #e0e0e0;
            padding: 20px;
        }
        .container { max-width: 1400px; margin: 0 auto; }
        .header {
            background: #2a2a2a;
            padding: 20px;
            border-radius: 8px;
            margin-bottom: 20px;
        }
        .header h1 { font-size: 24px; margin-bottom: 10px; }
        .header .meta { color: #888; font-size: 14px; }
        .summary {
            background: #2a2a2a;
            padding: 15px;
            border-radius: 8px;
            margin-bottom: 20px;
            display: flex;
            gap: 30px;
            flex-wrap: wrap;
        }
        .summary-item { display: flex; flex-direction: column; }
        .summary-item .label { color: #888; font-size: 12px; margin-bottom: 5px; }
        .summary-item .value { font-size: 20px; font-weight: bold; }
        .controls {
            background: #2a2a2a;
            padding: 15px;
            border-radius: 8px;
            margin-bottom: 20px;
        }
        .controls input[type="text"] {
            width: 300px;
            padding: 8px 12px;
            background: #1a1a1a;
            border: 1px solid #444;
            border-radius: 4px;
            color: #e0e0e0;
            font-size: 14px;
        }
        .controls input[type="text"]::placeholder { color: #666; }
        table {
            width: 100%;
            border-collapse: collapse;
            background: #2a2a2a;
            border-radius: 8px;
            overflow: hidden;
        }
        thead { background: #333; }
        th {
            padding: 12px;
            text-align: left;
            font-weight: 600;
            font-size: 12px;
            text-transform: uppercase;
            color: #aaa;
            cursor: pointer;
            user-select: none;
        }
        th:hover { background: #3a3a3a; }
        td {
            padding: 12px;
            border-top: 1px solid #333;
            font-size: 13px;
            font-family: "SF Mono", Monaco, monospace;
        }
        tr:hover { background: #333; }
        .expandable { cursor: pointer; }
        .detail-row {
            display: none;
            background: #1a1a1a;
        }
        .detail-row.expanded { display: table-row; }
        .detail-content {
            padding: 20px;
            border-top: 2px solid #444;
        }
        .detail-section {
            margin-bottom: 15px;
        }
        .detail-section h4 {
            color: #888;
            font-size: 11px;
            text-transform: uppercase;
            margin-bottom: 8px;
            letter-spacing: 1px;
        }
        .detail-section .content {
            background: #2a2a2a;
            padding: 10px;
            border-radius: 4px;
            font-family: "SF Mono", Monaco, monospace;
            font-size: 12px;
            overflow-x: auto;
        }
        .badge {
            display: inline-block;
            padding: 2px 8px;
            border-radius: 3px;
            font-size: 11px;
            font-weight: 600;
        }
        .badge.grpc { background: #4a9eff; color: #000; }
        .badge.zmq { background: #ff9f40; color: #000; }
        .badge.dds { background: #9f40ff; color: #fff; }
        .badge.inbound { background: #40ff9f; color: #000; }
        .badge.outbound { background: #ff4040; color: #fff; }
        .badge.unknown { background: #666; color: #fff; }
        .footer {
            text-align: center;
            color: #666;
            font-size: 12px;
            margin-top: 40px;
            padding: 20px;
        }
        pre { margin: 0; white-space: pre-wrap; word-break: break-word; }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>Probe Report</h1>
            <div class="meta" id="meta">Generated: {{TIMESTAMP}}</div>
        </div>

        <div class="summary" id="summary"></div>

        <div class="controls">
            <input type="text" id="search" placeholder="Search events (ID, transport, method, address...)">
        </div>

        <table>
            <thead>
                <tr>
                    <th onclick="sortTable(0)">ID</th>
                    <th onclick="sortTable(1)">Time</th>
                    <th onclick="sortTable(2)">Source</th>
                    <th onclick="sortTable(3)">Dest</th>
                    <th onclick="sortTable(4)">Transport</th>
                    <th onclick="sortTable(5)">Dir</th>
                    <th onclick="sortTable(6)">Size</th>
                    <th onclick="sortTable(7)">Summary</th>
                </tr>
            </thead>
            <tbody id="events"></tbody>
        </table>

        <div class="footer">
            Generated by <strong>Probe v{{VERSION}}</strong> — Universal Message Debugger
        </div>
    </div>

    <script>
        const EVENTS = {{EVENTS_JSON}};

        function formatTimestamp(nanos) {
            const ms = nanos / 1000000;
            const date = new Date(ms);
            return date.toLocaleTimeString() + '.' + (ms % 1000).toFixed(0).padStart(3, '0');
        }

        function formatSize(bytes) {
            if (bytes < 1024) return bytes + ' B';
            if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
            return (bytes / 1024 / 1024).toFixed(1) + ' MB';
        }

        function transportBadge(transport) {
            const cls = transport.toLowerCase().replace(/[^a-z]/g, '');
            return `<span class="badge ${cls}">${transport}</span>`;
        }

        function directionBadge(direction) {
            return `<span class="badge ${direction.toLowerCase()}">${direction}</span>`;
        }

        function buildSummary() {
            const total = EVENTS.length;
            const transports = {};
            const warnings = EVENTS.filter(e => e.warnings && e.warnings.length > 0).length;
            let minTime = Infinity, maxTime = 0;

            EVENTS.forEach(e => {
                transports[e.transport] = (transports[e.transport] || 0) + 1;
                const t = e.timestamp;
                if (t < minTime) minTime = t;
                if (t > maxTime) maxTime = t;
            });

            const timeRange = total > 0 ?
                formatTimestamp(minTime) + ' – ' + formatTimestamp(maxTime) : 'N/A';

            const transportStr = Object.entries(transports)
                .map(([k, v]) => `${k}: ${v}`)
                .join('  |  ');

            document.getElementById('summary').innerHTML = `
                <div class="summary-item"><div class="label">Events</div><div class="value">${total}</div></div>
                <div class="summary-item"><div class="label">Time Range</div><div class="value">${timeRange}</div></div>
                <div class="summary-item"><div class="label">Protocols</div><div class="value">${transportStr}</div></div>
                <div class="summary-item"><div class="label">Warnings</div><div class="value">${warnings}</div></div>
            `;
        }

        function getEventSummary(event) {
            if (event.metadata && event.metadata['grpc.method']) {
                return event.metadata['grpc.method'];
            }
            if (event.metadata && event.metadata['zmq.topic']) {
                return 'Topic: ' + event.metadata['zmq.topic'];
            }
            if (event.metadata && event.metadata['dds.topic_name']) {
                return 'Topic: ' + event.metadata['dds.topic_name'];
            }
            if (event.payload.type === 'decoded' && event.payload.schema_name) {
                return event.payload.schema_name;
            }
            return '—';
        }

        function getPayloadSize(event) {
            if (event.payload.type === 'raw') {
                return event.payload.raw ? atob(event.payload.raw).length : 0;
            }
            return event.payload.raw ? atob(event.payload.raw).length : 0;
        }

        function toggleDetails(id) {
            const row = document.getElementById('detail-' + id);
            row.classList.toggle('expanded');
        }

        function renderEvents(events) {
            const tbody = document.getElementById('events');
            tbody.innerHTML = events.map(event => {
                const srcAddr = event.source.network ? event.source.network.src : '—';
                const dstAddr = event.source.network ? event.source.network.dst : '—';
                const size = getPayloadSize(event);
                const summary = getEventSummary(event);

                const metadataHtml = Object.entries(event.metadata || {})
                    .map(([k, v]) => `<div><strong>${k}:</strong> ${v}</div>`)
                    .join('') || '<div>None</div>';

                const payloadInfo = event.payload.type === 'decoded' ?
                    `<pre>${JSON.stringify(event.payload.fields, null, 2)}</pre>` :
                    `<div>Raw bytes (${size} bytes)</div>`;

                const warningsHtml = event.warnings && event.warnings.length > 0 ?
                    `<div class="detail-section">
                        <h4>Warnings</h4>
                        <div class="content">${event.warnings.map(w => `<div>⚠ ${w}</div>`).join('')}</div>
                    </div>` : '';

                return `
                    <tr class="expandable" onclick="toggleDetails(${event.id})">
                        <td>${event.id}</td>
                        <td>${formatTimestamp(event.timestamp)}</td>
                        <td>${srcAddr}</td>
                        <td>${dstAddr}</td>
                        <td>${transportBadge(event.transport)}</td>
                        <td>${directionBadge(event.direction)}</td>
                        <td>${formatSize(size)}</td>
                        <td>${summary}</td>
                    </tr>
                    <tr class="detail-row" id="detail-${event.id}">
                        <td colspan="8">
                            <div class="detail-content">
                                <div class="detail-section">
                                    <h4>Source</h4>
                                    <div class="content">
                                        <div><strong>Adapter:</strong> ${event.source.adapter}</div>
                                        <div><strong>Origin:</strong> ${event.source.origin}</div>
                                    </div>
                                </div>
                                <div class="detail-section">
                                    <h4>Metadata</h4>
                                    <div class="content">${metadataHtml}</div>
                                </div>
                                <div class="detail-section">
                                    <h4>Payload (${size} bytes)</h4>
                                    <div class="content">${payloadInfo}</div>
                                </div>
                                ${warningsHtml}
                            </div>
                        </td>
                    </tr>
                `;
            }).join('');
        }

        let currentSort = { col: 0, asc: true };

        function sortTable(col) {
            const asc = currentSort.col === col ? !currentSort.asc : true;
            currentSort = { col, asc };

            const sorted = [...EVENTS].sort((a, b) => {
                let valA, valB;
                switch (col) {
                    case 0: valA = a.id; valB = b.id; break;
                    case 1: valA = a.timestamp; valB = b.timestamp; break;
                    case 2: valA = a.source.network?.src || ''; valB = b.source.network?.src || ''; break;
                    case 3: valA = a.source.network?.dst || ''; valB = b.source.network?.dst || ''; break;
                    case 4: valA = a.transport; valB = b.transport; break;
                    case 5: valA = a.direction; valB = b.direction; break;
                    case 6: valA = getPayloadSize(a); valB = getPayloadSize(b); break;
                    case 7: valA = getEventSummary(a); valB = getEventSummary(b); break;
                    default: return 0;
                }
                if (valA < valB) return asc ? -1 : 1;
                if (valA > valB) return asc ? 1 : -1;
                return 0;
            });

            renderEvents(sorted);
        }

        document.getElementById('search').addEventListener('input', (e) => {
            const query = e.target.value.toLowerCase();
            if (!query) {
                renderEvents(EVENTS);
                return;
            }

            const filtered = EVENTS.filter(event => {
                const searchText = [
                    event.id.toString(),
                    event.transport,
                    event.direction,
                    event.source.adapter,
                    event.source.origin,
                    event.source.network?.src || '',
                    event.source.network?.dst || '',
                    getEventSummary(event),
                    JSON.stringify(event.metadata || {}),
                ].join(' ').toLowerCase();

                return searchText.includes(query);
            });

            renderEvents(filtered);
        });

        buildSummary();
        renderEvents(EVENTS);
    </script>
</body>
</html>
"#;

impl Exporter for HtmlExporter {
    fn format_name(&self) -> &'static str {
        "html"
    }

    fn file_extension(&self) -> &'static str {
        "html"
    }

    fn export(&self, events: &[DebugEvent], writer: &mut dyn Write) -> Result<(), ExportError> {
        // Serialize events to compact JSON that will be embedded in the HTML
        let events_json = serde_json::to_string(events)?;

        // Get current timestamp for report generation time
        let now = chrono::Utc::now();
        let timestamp = now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

        // Replace placeholders in template
        let html = HTML_TEMPLATE
            .replace("{{TIMESTAMP}}", &timestamp)
            .replace("{{VERSION}}", env!("CARGO_PKG_VERSION"))
            .replace("{{EVENTS_JSON}}", &events_json);

        writer.write_all(html.as_bytes())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use prb_core::*;

    fn sample_event() -> DebugEvent {
        DebugEvent::builder()
            .id(EventId::from_raw(1))
            .timestamp(Timestamp::from_nanos(1_710_000_000_000_000_000))
            .source(EventSource {
                adapter: "pcap".into(),
                origin: "test.pcap".into(),
                network: Some(NetworkAddr {
                    src: "10.0.0.1:50051".into(),
                    dst: "10.0.0.2:8080".into(),
                }),
            })
            .transport(TransportKind::Grpc)
            .direction(Direction::Outbound)
            .payload(Payload::Raw {
                raw: Bytes::from_static(b"hello"),
            })
            .metadata("grpc.method", "/api.v1.Users/Get")
            .build()
    }

    #[test]
    fn html_contains_events() {
        let events = vec![sample_event()];
        let mut buf = Vec::new();
        HtmlExporter.export(&events, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();

        assert!(output.contains("<!DOCTYPE html>"));
        assert!(output.contains("/api.v1.Users/Get"));
        assert!(output.contains("10.0.0.1:50051"));
        assert!(output.contains("Probe Report"));

        // Verify JSON is embedded
        let parsed_events: Vec<DebugEvent> = serde_json::from_str(
            output
                .split("const EVENTS = ")
                .nth(1)
                .unwrap()
                .split(';')
                .next()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(parsed_events.len(), 1);
    }

    #[test]
    fn html_self_contained() {
        let events = vec![sample_event()];
        let mut buf = Vec::new();
        HtmlExporter.export(&events, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();

        // Ensure no external resource references
        assert!(!output.contains("http://"));
        assert!(!output.contains("https://"));
        assert!(!output.contains("<link rel"));
        assert!(!output.contains("<script src"));

        // Verify inline styles and scripts exist
        assert!(output.contains("<style>"));
        assert!(output.contains("<script>"));
    }

    #[test]
    fn html_empty_events() {
        let mut buf = Vec::new();
        HtmlExporter.export(&[], &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();

        assert!(output.contains("<!DOCTYPE html>"));
        assert!(output.contains("const EVENTS = []"));
    }
}
