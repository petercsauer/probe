# prb-ai

LLM-powered event explanation engine for PRB. Generates plain-English explanations of decoded network events, grounded in structured protocol data to minimize hallucination. Supports privacy-first local inference via Ollama and optional cloud providers (OpenAI, custom endpoints).

> **Note:** This crate is currently disabled pending `async-openai` API changes. The architecture and types are stable but the runtime integration needs updating.

## Key types

| Type | Description |
|------|-------------|
| `AiConfig` | Provider configuration — endpoint URL, model name, temperature, token limits |
| `AiProvider` | Provider enum: `Ollama`, `OpenAi`, `Custom` |
| `ExplainContext` | Converts `DebugEvent`s into structured summaries for LLM consumption |
| `AiError` | Error type for AI operations (network, parse, provider-specific) |

### Functions

| Function | Description |
|----------|-------------|
| `explain_event(&events, index, &config)` | Returns a full explanation for one event in context |
| `explain_event_stream(&events, index, &config)` | Streaming variant — yields explanation tokens as they arrive |

### Modules

| Module | Purpose |
|--------|---------|
| `config` | AI provider configuration and defaults |
| `context` | Event-to-summary conversion for LLM prompts |
| `prompt` | Protocol-specific system prompts with RFC grounding |
| `explain` | Orchestrates prompt building and LLM calls |

## Usage

```rust
use prb_ai::{AiConfig, AiProvider, explain_event};
use prb_core::DebugEvent;

async fn explain(events: &[DebugEvent]) -> Result<String, Box<dyn std::error::Error>> {
    let config = AiConfig::for_provider(AiProvider::Ollama);
    let explanation = explain_event(events, 0, &config).await?;
    Ok(explanation)
}
```

## Relationship to other crates

- **prb-core** — provides `DebugEvent`, the input to the explanation engine
- **prb-tui** — will surface AI explanations in the decode tree pane (planned)

See the [PRB documentation](../../docs/) for the full user guide.

<!-- cargo-rdme start -->

AI-powered packet explanation engine for Probe.

This crate provides LLM-powered plain-English explanations of decoded network events,
grounded in structured protocol data to minimize hallucination. Supports privacy-first
local models via Ollama and optional cloud providers (`OpenAI`, custom endpoints).

### Architecture

- `config`: AI provider configuration (Ollama, `OpenAI`, custom)
- `context`: Converts `DebugEvents` into structured summaries for LLM consumption
- `prompt`: Protocol-specific system prompts with RFC grounding
- `explain`: Main engine that orchestrates prompt building and LLM calls
- `error`: Error types for AI operations

### Example

```rust
use prb_ai::{AiConfig, AiProvider, explain_event};
use prb_core::DebugEvent;

let config = AiConfig::for_provider(AiProvider::Ollama);
let explanation = explain_event(&events, 0, &config).await?;
println!("{}", explanation);
```

<!-- cargo-rdme end -->
