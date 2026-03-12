---
segment: 5
title: "AI-Powered Packet Explanation (prb-ai)"
depends_on: []
risk: 5
complexity: Medium
cycle_budget: 3
status: pending
commit_message: "feat(prb-ai): add LLM-powered event explanation with Ollama/OpenAI support"
---

# Phase 2: AI-Powered Packet Explanation ("Probe Explain")

**Date**: March 10, 2026
**Addresses**: Competitive Analysis Recommendation #5
**Goal**: Add LLM-powered plain-English explanation of decoded network events,
grounded in structured protocol data to minimize hallucination, with privacy-first
local model support via Ollama and optional cloud providers.

---

## Executive Summary

Probe's decoded `DebugEvent`s are ideal RAG (Retrieval-Augmented Generation) context
for LLM explanation. Unlike generic packet analyzers that feed raw hex to an LLM,
Probe provides structured fields (gRPC method, status codes, ZMQ socket types, DDS
topics, protobuf-decoded payloads) that ground the LLM in factual data. This is the
key insight from ReGAIN (ICNC 2026): structured traffic summaries + LLM reasoning
achieves 95-99% accuracy vs. hallucination-prone raw analysis.

**State of the art referenced**:
- ReGAIN (2026): RAG over structured traffic summaries → 95-99% accuracy
- pktai (2025): Terminal UI + Ollama chat copilot for PCAP analysis
- eX-NIDS (2025): Prompt augmentation with threat intelligence improves LLM
  explanations by 20%+
- Cisco Meraki AI PCAP Analyzer: Cloud-based packet explanation
- LitenAI: Natural language PCAP reasoning

**Library choice**: `async-openai` (v0.33, 3.1M+ downloads, most mature Rust LLM
client). Works with OpenAI, Azure OpenAI, and any OpenAI-compatible API including
Ollama (`http://localhost:11434/v1`). Streaming support for progressive output.

---

## Architecture

```
┌─────────────────────────────────────────────────┐
│                  prb explain                     │
│  CLI command: reads events, selects target,      │
│  builds context window, calls AI engine          │
└─────────────┬───────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────────────┐
│                   prb-ai                         │
│                                                  │
│  ┌──────────┐  ┌───────────┐  ┌──────────────┐ │
│  │ Provider  │  │  Context  │  │   Prompt     │ │
│  │ Config    │──│  Builder  │──│   Templates  │ │
│  │(Ollama,   │  │(events →  │  │(system +     │ │
│  │ OpenAI,   │  │ structured│  │ protocol RFC │ │
│  │ Anthropic)│  │ summary)  │  │ grounding)   │ │
│  └──────────┘  └───────────┘  └──────────────┘ │
│                                                  │
│  ┌──────────────────────────────────────────┐   │
│  │            Explain Engine                 │   │
│  │  - Builds grounded prompt from context    │   │
│  │  - Calls LLM via async-openai            │   │
│  │  - Streams response to stdout/callback    │   │
│  │  - Handles errors gracefully              │   │
│  └──────────────────────────────────────────┘   │
└─────────────────────────────────────────────────┘
              │
              ▼ (OpenAI-compatible HTTP)
┌─────────────────────────────────────────────────┐
│  Ollama (local)  │  OpenAI  │  Anthropic (via   │
│  localhost:11434 │  api     │  openai-compat)    │
└─────────────────────────────────────────────────┘
```

---

## Sub-phases

### Phase 2A: `prb-ai` Crate Foundation

**New crate**: `crates/prb-ai/`

**Files**:
- `src/lib.rs` — Public API surface
- `src/config.rs` — `AiConfig` with provider, model, base URL, API key, temperature
- `src/provider.rs` — `AiProvider` enum (Ollama, OpenAI, Custom) with defaults
- `src/context.rs` — `ExplainContext` builder: serializes target event + surrounding
  events into a structured text summary suitable for LLM consumption
- `src/prompt.rs` — System prompt templates with protocol-specific RFC grounding
  sections (gRPC/HTTP2, ZMTP, DDS-RTPS, TLS, TCP)
- `src/explain.rs` — `ExplainEngine`: async function that builds the prompt, calls
  the LLM, and returns the explanation (with streaming support)
- `src/error.rs` — `AiError` enum

**Dependencies**:
- `prb-core` (for `DebugEvent`, `TransportKind`, metadata constants)
- `async-openai` (LLM client)
- `tokio` (async runtime, `rt-multi-thread` + `macros` features)
- `serde`, `serde_json` (config serialization)
- `thiserror` (error types)
- `tracing` (logging)

**Key design decisions**:
1. **Privacy-first**: Ollama is the default provider. No data leaves the machine
   unless the user explicitly sets `--provider openai` or `--provider custom`.
2. **Structured context, not raw packets**: The LLM never sees hex dumps. It receives
   structured summaries: "gRPC call to /api.v1.Users/Get, status UNAVAILABLE (14),
   payload: {user_id: 42}, from 10.0.0.1:52341 to 10.0.0.2:50051".
3. **Grounded prompts**: System prompts include relevant RFC/protocol snippets so the
   LLM can reference authoritative sources (e.g., gRPC status code meanings from the
   gRPC spec, HTTP/2 error codes from RFC 9113).
4. **Streaming**: Use SSE streaming for progressive output in the terminal.

### Phase 2B: CLI `explain` Command

**Modified files**:
- `crates/prb-cli/Cargo.toml` — add `prb-ai` dependency, `tokio` runtime
- `crates/prb-cli/src/cli.rs` — add `Explain(ExplainArgs)` variant to `Commands`
- `crates/prb-cli/src/commands/mod.rs` — add `explain` module
- `crates/prb-cli/src/commands/explain.rs` — command implementation
- `crates/prb-cli/src/main.rs` — dispatch `Explain` command

**CLI interface**:
```
prb explain <input> [OPTIONS]

Arguments:
  <input>       Path to NDJSON or MCAP file

Options:
  --event-id <ID>         Event ID to explain (default: last event)
  --context <N>           Number of surrounding events for context (default: 5)
  --provider <PROVIDER>   AI provider: ollama, openai, custom (default: ollama)
  --model <MODEL>         Model name (default: provider-dependent)
  --base-url <URL>        Custom API base URL
  --api-key <KEY>         API key (or set PRB_AI_API_KEY env var)
  --temperature <FLOAT>   Generation temperature 0.0-1.0 (default: 0.3)
  --no-stream             Disable streaming output
```

**Examples**:
```bash
# Explain the last event using local Ollama (default)
prb explain capture.mcap

# Explain a specific event with more context
prb explain events.ndjson --event-id 42 --context 10

# Use OpenAI GPT-4
prb explain capture.mcap --provider openai --model gpt-4o --api-key sk-...

# Use a custom OpenAI-compatible endpoint
prb explain capture.mcap --provider custom --base-url http://my-llm:8080/v1

# Pipe from ingest
prb ingest capture.pcap | prb explain - --event-id 7
```

### Phase 2C: Protocol-Specific Prompt Engineering

**System prompt structure** (inspired by ReGAIN's evidence-grounded approach and
eX-NIDS's prompt augmentation):

```
[ROLE]
You are Probe, an expert network protocol analyzer. You explain decoded network
events in plain English, helping developers debug communication issues.

[PROTOCOL KNOWLEDGE]
{Dynamically inserted based on transport type}

For gRPC events:
- gRPC status codes and their meanings (from grpc/grpc spec)
- HTTP/2 frame types and error codes (from RFC 9113)
- Common failure patterns (RST_STREAM, GOAWAY, deadline exceeded)

For ZMQ events:
- ZMTP socket types and patterns (from ZMQ RFC 23, 28)
- Common issues (slow subscriber, HWM drops)

For DDS events:
- RTPS discovery protocol (from OMG DDS-RTPS spec)
- QoS policy implications

For TLS-related:
- TLS handshake stages, cipher suite meanings
- Common decryption failure causes

[GROUNDING RULES]
- ONLY reference information present in the provided event data
- Cite specific field values when making claims
- If uncertain, say so explicitly
- Suggest concrete next debugging steps

[EVENT DATA]
{Structured event summary from ExplainContext}

[TASK]
Explain what is happening in this network event. Include:
1. What the event represents (in plain English)
2. Whether anything looks abnormal or concerning
3. Possible root causes if there's an error
4. Concrete next steps for debugging
```

### Phase 2D: Tests

**Unit tests** (`crates/prb-ai/src/`):
- `test_context_builder_grpc`: Build context from gRPC events, verify structured output
- `test_context_builder_zmq`: Build context from ZMQ events
- `test_context_builder_dds`: Build context from DDS events
- `test_context_window_selection`: Verify surrounding event selection logic
- `test_prompt_template_grpc`: System prompt includes gRPC RFC references
- `test_prompt_template_zmq`: System prompt includes ZMQ references
- `test_prompt_template_tls`: TLS event triggers TLS-specific grounding
- `test_config_defaults_ollama`: Default config targets Ollama
- `test_config_openai_requires_key`: OpenAI provider validates API key presence
- `test_config_from_env`: Config reads PRB_AI_API_KEY from environment

**Integration tests** (`crates/prb-ai/tests/`):
- `test_explain_prompt_assembly`: End-to-end prompt assembly without LLM call
- `test_explain_context_serialization`: Verify JSON roundtrip of context

**CLI tests** (`crates/prb-cli/tests/`):
- `test_cli_explain_help`: `prb explain --help` prints usage
- `test_cli_explain_missing_input`: Error message for missing file
- `test_cli_explain_no_provider`: Graceful error when Ollama not running

---

## Execution Order

1. **Phase 2A** — Create `prb-ai` crate with all modules
2. **Phase 2B** — Wire into CLI as `prb explain` command
3. **Phase 2C** — Implement protocol-specific prompt templates
4. **Phase 2D** — Add tests, verify compilation, clippy clean

---

## Acceptance Criteria

- [ ] `cargo build --workspace` — zero errors, zero warnings
- [ ] `cargo clippy --workspace --all-targets` — zero new warnings
- [ ] `cargo test --workspace` — all tests pass
- [ ] `prb explain --help` shows usage with all options
- [ ] `prb explain fixtures/grpc_sample.json` produces structured prompt (even if
      Ollama is not running, the prompt assembly is testable)
- [ ] Default provider is Ollama (privacy-first)
- [ ] No data sent externally without explicit `--provider openai` or `--provider custom`
- [ ] Protocol-specific prompt grounding activates per transport type
- [ ] Streaming output works when provider is available
- [ ] Graceful error handling when provider is unreachable
