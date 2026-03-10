# OpenTelemetry Trace Correlation Fixtures

Synthetic test fixtures for validating OTel trace context extraction and correlation.

## Fixture Files

### synthetic-trace.spans.json
- **Description:** Simple multi-span trace with parent-child relationships
- **Trace ID:** `4bf92f3577b34da6a3ce929d0e0e4736`
- **Spans:** 2 spans (HTTP request → Database query)
- **Use:** Basic correlation testing

### multi-service-trace.spans.json
- **Description:** Multi-service distributed trace
- **Trace ID:** `a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6`
- **Spans:** 3 spans across frontend, gateway, and payment services
- **Use:** Cross-service correlation testing

## Span JSON Format

Each span follows OpenTelemetry span export format:

```json
{
  "trace_id": "32 hex char trace ID",
  "span_id": "16 hex char span ID",
  "parent_span_id": "16 hex char parent span ID or null",
  "name": "Human-readable span name",
  "start_time_unix_nano": 1234567890000000000,
  "end_time_unix_nano": 1234567890123000000,
  "attributes": {
    "key": "value"
  }
}
```

## Usage

These fixtures are used by `crates/prb-core/tests/real_data_otel_correlation_tests.rs` to verify:

1. W3C traceparent header extraction
2. B3 propagation format support
3. Trace context correlation across network events
4. Multi-service trace correlation
5. Graceful handling of missing spans

## Attribution

These are synthetic fixtures generated for testing purposes. They follow OpenTelemetry specification formats for trace context propagation.
