# Protocol Decoder Architecture Patterns Research
**Date:** 2026-03-13
**Context:** Probe project has 3 protocol decoders (prb-grpc, prb-zmq, prb-dds) with 90%+ identical structure

---

## SOURCE 3: EXISTING SOLUTIONS

### 3.1 tokio-util Codec Framework
**URL:** https://docs.rs/tokio-util/latest/tokio_util/codec/  
**Crates.io:** https://crates.io/crates/tokio-util  
**Maintenance:** Active (Tokio project, 1M+ downloads/day)

**Architecture Pattern:**
- **Trait-based codec system** with `Encoder` and `Decoder` traits
- Separates framing logic from application logic
- Pattern: `Decoder` trait provides `decode()` method that consumes bytes from buffer

```rust
pub trait Decoder {
    type Item;
    type Error: From<std::io::Error>;
    
    fn decode(&mut self, src: &mut BytesMut) 
        -> Result<Option<Self::Item>, Self::Error>;
}
```

**Extension Model:**
- Implement `Decoder` trait for your protocol
- Built-in codecs: `LinesCodec`, `BytesCodec`, `LengthDelimitedCodec`
- Composable via `Framed` adapters

**Key Insights:**
- **Stateful decoders:** Decoder instance maintains state (buffer position, protocol state machine)
- **Partial consumption:** `decode()` returns `Option<Item>` - None if incomplete data
- **Buffer management:** Framework handles buffer allocation, decoder just consumes

**Recommendation:** **ADAPT**
- Pattern is excellent for stateful stream decoders
- However, tokio-util is async-focused, probe is sync
- Can adopt the trait pattern without the async machinery

---

### 3.2 prost (Protocol Buffers)
**URL:** https://docs.rs/prost/latest/prost/  
**Crates.io:** https://crates.io/crates/prost  
**Maintenance:** Active (500K+ downloads/day)

**Architecture Pattern:**
- **Code generation** from `.proto` files
- Trait: `Message` with `encode()`/`decode()` methods
- Decoder is stateless - pure function from bytes to struct

```rust
pub trait Message: Default {
    fn encode<B>(&self, buf: &mut B) -> Result<(), EncodeError>;
    fn decode<B>(buf: B) -> Result<Self, DecodeError>;
}
```

**Extension Model:**
- Add new protocols via `.proto` definitions + code generation
- Not a runtime plugin system

**Key Insights:**
- **Stateless design:** Each decode() is independent
- **Type-driven:** Strong typing from schema definitions
- **Not applicable here:** Probe already uses prost for protobuf payloads, but decoders need to be stateful (HTTP/2 HPACK tables, ZMQ handshakes)

**Recommendation:** **REJECT**
- Too rigid for stateful protocol decoders
- Probe's decoders need to maintain connection state across multiple calls

---

### 3.3 nom (Parser Combinators)
**URL:** https://docs.rs/nom/latest/nom/  
**Crates.io:** https://crates.io/crates/nom  
**Maintenance:** Active (Probe already uses nom for query parsing)

**Architecture Pattern:**
- **Parser combinator library** for building parsers from small components
- Pattern: Functions that return `IResult<Input, Output, Error>`

```rust
fn parse_header(input: &[u8]) -> IResult<&[u8], Header> {
    let (input, magic) = tag(b"RTPS")(input)?;
    let (input, version) = take(2u8)(input)?;
    Ok((input, Header { magic, version }))
}
```

**Extension Model:**
- Compose parsers using combinators (`map`, `tuple`, `alt`)
- Build complex parsers from simple building blocks

**Key Insights:**
- **Stateless parsers:** Each parser function is pure
- **Excellent for wire format parsing:** Probe already uses this in `rtps_parser`, `zmtp` parser modules
- **Not for decoder orchestration:** Good for parsing individual messages, not managing decoder lifecycle

**Recommendation:** **KEEP USING**
- Probe already uses nom effectively for low-level parsing
- Not a solution for decoder abstraction (which is about managing state + event building)

---

### 3.4 bytes (Buffer Management)
**URL:** https://docs.rs/bytes/latest/bytes/  
**Maintenance:** Active (Tokio project, 5M+ downloads/day)

**Architecture Pattern:**
- **Zero-copy buffer types:** `Bytes` (immutable), `BytesMut` (mutable)
- Cheap cloning via reference counting

**Key Insights:**
- Probe already uses `Bytes` for payload storage
- Good for avoiding memcpy when passing data between decoder layers

**Recommendation:** **ALREADY ADOPTED**

---

### 3.5 serde (Serialization Framework)
**URL:** https://docs.rs/serde/latest/serde/  
**Crates.io:** https://crates.io/crates/serde

**Architecture Pattern:**
- **Trait-based serialization:** `Serialize` and `Deserialize` traits
- Derive macros for automatic implementation
- **Data format agnostic:** Same traits work for JSON, YAML, CBOR, etc.

```rust
pub trait Deserialize<'de>: Sized {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'de>;
}
```

**Extension Model:**
- Add new formats by implementing `Deserializer` trait
- Applications implement `Deserialize` once, works with all formats

**Key Insights:**
- **Trait + trait object pattern:** Decoder abstraction should work similarly
- **Default implementations:** Many types get auto-generated impls via derive macros
- **Format-agnostic consumer code:** Like serde's format-agnostic structs, probe's pipeline should be protocol-agnostic

**Recommendation:** **ADAPT PATTERN**
- Use trait-based abstraction like serde
- Protocol-specific decoders implement trait, pipeline uses trait objects
- Consider derive macros for reducing boilerplate (future work)

---

### 3.6 tower (Middleware/Service Abstraction)
**URL:** https://docs.rs/tower/latest/tower/  
**Crates.io:** https://crates.io/crates/tower

**Architecture Pattern:**
- **Service trait** for request/response patterns
- **Layer trait** for composable middleware

```rust
pub trait Service<Request> {
    type Response;
    type Error;
    type Future: Future<Output = Result<Self::Response, Self::Error>>;
    
    fn call(&mut self, req: Request) -> Self::Future;
}
```

**Key Insights:**
- **Middleware composition:** Layers wrap services to add functionality (logging, metrics, retry)
- **Applicable to decoders?** Could model decoder as `Service<&[u8]>` returning `Vec<DebugEvent>`
- **Async-focused:** Like tokio-util, designed for async Rust

**Recommendation:** **CONSIDER FOR FUTURE**
- Interesting pattern for composable decoder middleware (e.g., compression, encryption)
- Overkill for current problem (need simpler trait abstraction first)

---

## SOURCE 4: EXTERNAL BEST PRACTICES

### 4.1 Rust API Guidelines - Trait Design
**Source:** https://rust-lang.github.io/api-guidelines/

**Key Principles:**

#### C-REUSE: Encourage code reuse
- **Guideline:** Provide default implementations for trait methods where possible
- **Application:** Create `ProtocolDecoder` trait with helper methods that have default impls

#### C-COMMON-TRAITS: Implement common traits
- **Guideline:** Types should implement `Debug`, `Clone`, `Send`, etc. where appropriate
- **Application:** Decoders already require `Send` for threading

#### C-OBJECT-SAFE: Support trait objects
- **Guideline:** Make traits object-safe unless there's a strong reason not to
- **Application:** `ProtocolDecoder` should support `Box<dyn ProtocolDecoder>` for dynamic dispatch

---

### 4.2 Trait-Based Plugin Systems (Blog Post Analysis)

**Source:** "Rust Plugins with Traits" - Multiple blog posts pattern

**Common Patterns:**

#### Pattern 1: Trait + Factory
```rust
pub trait Decoder {
    fn decode(&mut self, data: &[u8]) -> Result<Vec<Event>>;
}

pub trait DecoderFactory {
    fn create(&self) -> Box<dyn Decoder>;
}
```

**Probe already uses this pattern!** See `DecoderFactory` in `prb-detect/src/registry.rs`.

#### Pattern 2: Trait with Associated Types
```rust
pub trait Decoder {
    type Config;
    type Event;
    
    fn decode(&mut self, data: &[u8]) -> Result<Vec<Self::Event>>;
}
```

**Analysis:** More flexible but adds complexity. Probe uses concrete `DebugEvent` type, so not needed.

#### Pattern 3: Blanket Implementations
```rust
impl<T: RawDecoder> ProtocolDecoder for T {
    fn decode_stream(&mut self, data: &[u8], ctx: &DecodeContext) 
        -> Result<Vec<DebugEvent>, CoreError> 
    {
        // Common logic here
        let raw_events = self.decode_raw(data)?;
        raw_events.into_iter().map(|e| self.build_event(e, ctx)).collect()
    }
}
```

**Analysis:** Could extract common event building logic this way.

---

### 4.3 Reducing Code Duplication: Macros vs Generics vs Traits

**Source:** Rust community best practices

#### When to use Generics:
- Type-parametric code that works the same for all types
- Zero-cost abstraction (no runtime overhead)
- **Not applicable here:** Decoders have different behavior, not just different types

#### When to use Trait Objects:
- Runtime polymorphism needed (decoder selected at runtime based on protocol detection)
- **Probe already uses this:** `Box<dyn ProtocolDecoder>` in decoder registry
- **Trade-off:** Small vtable overhead vs static dispatch

#### When to use Macros:
- Generating repetitive boilerplate code
- **Could apply:** Common event builder patterns
- **Caution:** Macros are harder to debug, prefer traits first

**Recommendation for Probe:**
1. **Primary:** Trait-based abstraction with default implementations
2. **Secondary:** Helper functions/modules for common logic
3. **Last resort:** Macros only if trait approach leaves significant duplication

---

### 4.4 Event Building Patterns

**Source:** Builder pattern analysis (effective Rust)

#### Current Probe Pattern:
```rust
// 75+ LOC duplicated across 3 decoders
let event_builder = DebugEvent::builder()
    .timestamp(ctx.timestamp.unwrap_or_else(Timestamp::now))
    .source(EventSource {
        adapter: "pcap".to_string(),
        origin: ctx.metadata.get("origin").cloned().unwrap_or("unknown".to_string()),
        network: Some(NetworkAddr {
            src: ctx.src_addr.clone().unwrap_or("unknown".to_string()),
            dst: ctx.dst_addr.clone().unwrap_or("unknown".to_string()),
        }),
    })
    .transport(TransportKind::Grpc) // Only difference!
    .direction(direction)
    // ... protocol-specific metadata
```

#### Proposed Pattern: Context-Aware Builder Extension
```rust
// In prb-core or new prb-decoder-common crate
impl DecodeContext {
    pub fn create_event_builder(&self, transport: TransportKind) 
        -> DebugEventBuilder 
    {
        DebugEvent::builder()
            .timestamp(self.timestamp.unwrap_or_else(Timestamp::now))
            .source(EventSource {
                adapter: "pcap".to_string(),
                origin: self.metadata.get("origin").cloned()
                    .unwrap_or_else(|| "unknown".to_string()),
                network: Some(NetworkAddr {
                    src: self.src_addr.clone()
                        .unwrap_or_else(|| "unknown".to_string()),
                    dst: self.dst_addr.clone()
                        .unwrap_or_else(|| "unknown".to_string()),
                }),
            })
            .transport(transport)
    }
}

// In decoders
let mut event_builder = ctx.create_event_builder(TransportKind::Grpc)
    .direction(direction)
    .payload(payload)
    .metadata("grpc.method", method);
```

**Benefits:**
- Reduces 20 lines to 1 line
- Centralized source/network extraction logic
- Decoders focus on protocol-specific metadata

---

### 4.5 State Management in Decoders

**Source:** Stateful protocol decoding patterns

#### Common Approaches:

**Approach 1: Decoder Owns State (Current Probe)**
```rust
pub struct GrpcDecoder {
    h2_codec: H2Codec,
    lpm_parsers: HashMap<u32, LpmParser>,
    sequence: u64,
}
```

**Pros:** Simple, decoder fully encapsulates state  
**Cons:** Hard to share state across decoders (e.g., DDS discovery)

**Approach 2: External State Manager**
```rust
pub struct DecoderState {
    protocol_state: Box<dyn Any>,  // Per-protocol state
    shared_context: Arc<SharedContext>,  // Cross-protocol state
}
```

**Pros:** Can share state (e.g., discovery data)  
**Cons:** More complex, type erasure with `Any`

**Recommendation:** Keep Approach 1 for now. Probe's decoders don't need shared state (DDS discovery is per-decoder instance).

---

### 4.6 Transport-Agnostic Protocol Decoders

**Source:** OSI layer separation principles

**Question:** Should decoders be transport-agnostic?

**Analysis:**
- **gRPC:** HTTP/2-specific (runs over TCP)
- **ZMQ:** ZMTP wire format (TCP or IPC)
- **DDS:** RTPS protocol (UDP)

**Current Probe Design:** Decoders are protocol-specific, not transport-agnostic. This is correct - gRPC decoder assumes TCP stream reassembly, DDS assumes UDP datagrams.

**Recommendation:** Don't over-abstract. Protocol and transport are coupled in probe's domain.

---

## SPECIFIC ANSWERS TO QUESTIONS

### Q1: Should we create a generic `DecoderBase` trait with default implementations?

**Answer:** **YES, but as extension trait, not base trait**

**Approach:**
```rust
// Keep existing ProtocolDecoder trait minimal (object-safe)
pub trait ProtocolDecoder: Send {
    fn protocol(&self) -> TransportKind;
    fn decode_stream(&mut self, data: &[u8], ctx: &DecodeContext) 
        -> Result<Vec<DebugEvent>, CoreError>;
}

// Add extension trait with helper methods
pub trait ProtocolDecoderExt: ProtocolDecoder {
    fn increment_sequence(&mut self, seq: &mut u64) -> u64 {
        *seq += 1;
        *seq
    }
    
    // Could add more helpers, but event building should be in DecodeContext
}

// Blanket impl
impl<T: ProtocolDecoder> ProtocolDecoderExt for T {}
```

**Why not base trait with defaults?**
- Current `ProtocolDecoder` trait is minimal and object-safe
- Adding methods with default impls wouldn't reduce duplication much (decoders don't call common methods)
- The duplication is in event building, not trait methods

---

### Q2: Should event building logic be extracted to a separate module/crate?

**Answer:** **YES - extract to helper method on `DecodeContext`**

**Recommendation:**

**Option A: Method on DecodeContext (RECOMMENDED)**
```rust
// In prb-core/src/decode.rs
impl DecodeContext {
    pub fn create_event_builder(&self, transport: TransportKind) -> DebugEventBuilder {
        // 20 lines of common boilerplate here
    }
}
```

**Why:** 
- `DecodeContext` already carries all the metadata needed
- No new crate needed
- Decoders already depend on `DecodeContext`

**Option B: New prb-decoder-common crate**
- Only if we extract MORE common logic (e.g., common correlation key extraction, sequence management)
- For now, Option A is sufficient

---

### Q3: Is there a standard pattern for "transport-agnostic protocol decoders"?

**Answer:** **Yes, but not applicable here**

**Standard Pattern:** ISO-TP (transport protocol) pattern - separate framing from payload processing.

Example: HTTP can run over TCP, QUIC, or Unix sockets. HTTP parser is transport-agnostic.

**Probe's Case:** 
- gRPC requires HTTP/2 which requires TCP (not transport-agnostic)
- ZMQ requires ZMTP wire format (TCP-specific)
- DDS requires RTPS protocol (UDP-specific)

**Recommendation:** Don't abstract transport layer. Probe's decoders are correctly coupled to their transport.

---

### Q4: How do other projects handle decoder → event transformation?

**Answer:** **Two common patterns**

**Pattern 1: Builder Pattern (Probe uses this)**
```rust
Event::builder()
    .field1(value1)
    .field2(value2)
    .build()
```

**Used by:** serde_json, prost, many Rust projects  
**Pros:** Fluent API, compile-time field checking  
**Cons:** Verbose if many fields are common

**Pattern 2: From/Into Traits**
```rust
impl From<RawEvent> for DebugEvent {
    fn from(raw: RawEvent) -> Self {
        // transformation logic
    }
}
```

**Used by:** std library conversions  
**Pros:** Idiomatic, concise at call site  
**Cons:** Probe has context (DecodeContext) that needs to be threaded through

**Probe's Optimal Pattern:** Hybrid
- Keep builder pattern for flexibility
- Add helper on `DecodeContext` to construct initial builder with common fields
- Decoders call `ctx.create_event_builder(transport)` then add protocol-specific fields

---

## ARCHITECTURAL RECOMMENDATIONS

### Recommendation 1: Extract Event Builder Helper (HIGH PRIORITY)

**What:** Add method to `DecodeContext` to create pre-populated event builder

**Where:** `crates/prb-core/src/decode.rs`

**Implementation:**
```rust
impl DecodeContext {
    /// Create a DebugEventBuilder with common fields pre-populated from context.
    pub fn create_event_builder(&self, transport: TransportKind) -> DebugEventBuilder {
        DebugEvent::builder()
            .timestamp(self.timestamp.unwrap_or_else(Timestamp::now))
            .source(EventSource {
                adapter: "pcap".to_string(),
                origin: self.metadata.get("origin")
                    .cloned()
                    .unwrap_or_else(|| "unknown".to_string()),
                network: Some(NetworkAddr {
                    src: self.src_addr.clone()
                        .unwrap_or_else(|| "unknown".to_string()),
                    dst: self.dst_addr.clone()
                        .unwrap_or_else(|| "unknown".to_string()),
                }),
            })
            .transport(transport)
    }
}
```

**Impact:** Eliminates 20-25 lines of duplication per event creation call

**Files to modify:**
- `/Users/psauer/probe/crates/prb-core/src/decode.rs`
- `/Users/psauer/probe/crates/prb-grpc/src/decoder.rs`
- `/Users/psauer/probe/crates/prb-zmq/src/decoder.rs`
- `/Users/psauer/probe/crates/prb-dds/src/decoder.rs`

**Estimated reduction:** ~75-100 LOC removed across decoder crates

---

### Recommendation 2: Add Sequence Management Helper (MEDIUM PRIORITY)

**What:** Extract common sequence increment pattern

**Current pattern** (repeated in all decoders):
```rust
self.sequence += 1;
// ... build event ...
.sequence(self.sequence)
```

**Proposed pattern:**
```rust
// In each decoder
fn next_sequence(&mut self) -> u64 {
    self.sequence += 1;
    self.sequence
}

// Usage
let seq = self.next_sequence();
// ... build event ...
.sequence(seq)
```

**Impact:** Minor (3 lines → 1 line), but improves clarity

---

### Recommendation 3: Keep ProtocolDecoder Trait Minimal (CURRENT STATE)

**What:** Do NOT add methods to `ProtocolDecoder` trait

**Rationale:**
- Current 2-method trait is object-safe and minimal
- Decoders have different internal structures (no common methods to add)
- Duplication is in event building, not trait implementation

**Keep:**
```rust
pub trait ProtocolDecoder: Send {
    fn protocol(&self) -> TransportKind;
    fn decode_stream(&mut self, data: &[u8], ctx: &DecodeContext) 
        -> Result<Vec<DebugEvent>, CoreError>;
}
```

---

### Recommendation 4: Document Decoder Lifecycle Pattern (LOW PRIORITY)

**What:** Add documentation about stateful decoder pattern

**Where:** `crates/prb-core/src/traits.rs` - add doc comments to `ProtocolDecoder`

**Content:**
```rust
/// Decodes protocol-specific byte sequences into structured events.
///
/// # Stateful Decoding
///
/// Decoders maintain internal state across multiple `decode_stream()` calls:
/// - **GrpcDecoder**: HTTP/2 HPACK dynamic table, per-stream reassembly
/// - **ZmqDecoder**: ZMTP handshake state, multipart message accumulation  
/// - **DdsDecoder**: SEDP discovery mappings (writer GUID → topic name)
///
/// The decoder registry (`DecoderRegistry`) caches active decoder instances
/// keyed by stream/connection to ensure state persistence.
///
/// # Implementation Guidelines
///
/// 1. Use `ctx.create_event_builder(transport)` to create pre-populated builders
/// 2. Maintain sequence counters for event ordering
/// 3. Add protocol-specific metadata and correlation keys
/// 4. Handle partial data gracefully (return empty Vec if incomplete)
```

---

## SUMMARY DECISION MATRIX

| Solution | Adopt | Adapt | Reject | Rationale |
|----------|-------|-------|--------|-----------|
| tokio-util Codec | | ✓ | | Trait pattern excellent, skip async parts |
| prost | | | ✓ | Stateless design, probe needs stateful |
| nom | ✓ | | | Already using for parsing |
| bytes | ✓ | | | Already using for zero-copy buffers |
| serde pattern | | ✓ | | Trait-based abstraction model to follow |
| tower Service | | | ✓ | Too complex, async-focused |
| Trait + defaults | | ✓ | | Extension trait if needed, not base trait |
| Event builder helper | ✓ | | | High priority - eliminates 75+ LOC |
| Sequence helper | ✓ | | | Low-hanging fruit |
| Macros | | | ✓ | Prefer trait-based approach first |
| New decoder crate | | | ✓ | Not needed - helper on DecodeContext sufficient |

---

## IMMEDIATE ACTION ITEMS

1. **Add `DecodeContext::create_event_builder()` method** - 15 min  
   File: `crates/prb-core/src/decode.rs`

2. **Refactor GrpcDecoder event building** - 20 min  
   File: `crates/prb-grpc/src/decoder.rs`  
   Methods: `create_message_event`, `create_trailers_event`

3. **Refactor ZmqDecoder event building** - 15 min  
   File: `crates/prb-zmq/src/decoder.rs`  
   Method: `create_message_event`

4. **Refactor DdsDecoder event building** - 15 min  
   File: `crates/prb-dds/src/decoder.rs`  
   Method: `create_data_event`

5. **Add tests for new helper** - 10 min  
   File: `crates/prb-core/src/decode.rs` (inline tests)

**Total estimated effort:** ~75 minutes  
**Expected LOC reduction:** ~75-100 lines  
**Code quality improvement:** High (DRY principle, centralized context extraction)

---

## REFERENCES

- Rust API Guidelines: https://rust-lang.github.io/api-guidelines/
- tokio-util codecs: https://docs.rs/tokio-util/latest/tokio_util/codec/
- bytes crate: https://docs.rs/bytes/
- nom parser combinators: https://docs.rs/nom/
- Effective Rust builder pattern: https://rust-unofficial.github.io/patterns/patterns/creational/builder.html

