# AI Features

PRB includes AI-powered analysis features for event explanation, anomaly detection, protocol identification, and natural language filter generation.

## Overview

AI features use Large Language Models (LLMs) to provide intelligent insights into network captures. Supported providers:

| Provider | Privacy | Requirements |
|----------|---------|--------------|
| **Ollama** (default) | Local inference, data never leaves your machine | Install Ollama and pull a model |
| **OpenAI** | Cloud API | API key required |
| **Custom** | Configurable | Any OpenAI-compatible endpoint |

## Configuration

### Ollama (Recommended)

Privacy-first local AI with no external API calls:

1. Install Ollama: https://ollama.ai
2. Pull a model: `ollama pull llama3.1`
3. No additional configuration needed

PRB will automatically use `http://localhost:11434/v1` with model `llama3.1`.

### OpenAI

Cloud-based API with latest models:

```toml
# ~/.config/prb/config.toml
[ai]
provider = "openai"
api_key = "sk-..."  # or set PRB_AI_API_KEY env var
model = "gpt-4o-mini"  # optional, default is gpt-4o-mini
```

Or use environment variable:
```bash
export PRB_AI_API_KEY="sk-..."
prb tui capture.pcap
```

### Custom Provider

Use any OpenAI-compatible endpoint (Anthropic, LM Studio, Groq, etc.):

```toml
# ~/.config/prb/config.toml
[ai]
provider = "custom"
base_url = "https://api.anthropic.com/v1"
api_key = "your-api-key"
model = "claude-3-sonnet"
```

## Features

### 1. Event Explanation

**What:** Provides detailed analysis of a single event with surrounding context.

**How to use:**
1. Select an event in the TUI
2. Press `a` to open AI panel
3. View streaming explanation with:
   - What happened
   - Technical analysis
   - Root cause (for errors)
   - Next steps

**Context Window:** Includes 5 surrounding events for better context understanding.

**Protocol-Aware:** System prompts reference relevant RFCs (gRPC, HTTP/2, ZMQ, DDS, TLS) for accurate explanations.

**Caching:** Explanations are cached per event ID to avoid redundant API calls.

### 2. Anomaly Detection

**What:** Analyzes entire capture to identify:
- High error rates
- Repeated failures
- Performance issues
- Unusual patterns

**How to use:**
1. Open a capture in TUI
2. Press `D` to run anomaly detection
3. Review detected anomalies with:
   - Title
   - Description
   - Severity (High, Medium, Low)
   - Filter expression to find related events
   - Affected event indices

**Output Example:**
```
Anomaly: High gRPC Error Rate
Severity: High
Description: 15 of 20 gRPC calls (75%) failed with status 14 (UNAVAILABLE)
Filter: grpc.status == 14
Events: [5, 8, 12, 15, ...]
```

### 3. Protocol Identification

**What:** Identifies unknown protocols by analyzing packet hex dumps.

**How to use:**
1. Select an event with unknown protocol
2. Press `P` to identify protocol
3. View results:
   - Protocol name
   - Confidence score (0.0-1.0)
   - Reasoning (magic bytes, structure, patterns)

**Analysis:** Examines first 256 bytes for:
- Magic bytes and headers
- Binary vs text encoding
- Structure patterns
- Known protocol signatures

**Use Case:** Useful for proprietary protocols, legacy systems, or when auto-detection fails.

### 4. Natural Language Filters

**What:** Converts natural language queries to prb-query filter expressions.

**How to use:**
1. Press `@` in TUI
2. Type natural language query, e.g.:
   - "show errors"
   - "find DNS traffic"
   - "gRPC calls to Users service"
3. Press Enter
4. AI generates and validates filter expression
5. Filter is applied to event list

**Example Translations:**
| Natural Language | Generated Filter |
|------------------|------------------|
| "show errors" | `grpc.status != "0"` |
| "DNS traffic" | `transport == "UDP" && dst contains ":53"` |
| "slow requests" | `len(payload) > 10000` |
| "gRPC to Users" | `transport == "gRPC" && grpc.method contains "Users"` |

**Validation:** Generated filters are parsed and validated before application.

**Rate Limiting:** 10 requests per 60 seconds to prevent API abuse.

### 5. Capture Summary

**What:** Generates high-level summary of entire capture with:
- Event count and time range
- Protocol breakdown
- Error rate analysis
- Performance insights
- Notable patterns

**How to use:**
1. Open a capture in TUI
2. Press `A` (capital A)
3. View summary in AI panel

**Use Case:** Quick overview of large captures, executive summaries, incident reports.

## TUI Integration

### AI Panel

Press `a` to toggle AI panel on the right side of the screen. The panel displays:
- Streaming explanations with cursor indicator
- Anomaly detection results
- Protocol identification hints
- Capture summaries

**Navigation:**
- `a` - Toggle panel visibility
- `j` / `k` - Scroll panel content
- `Esc` - Close panel

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `a` | Toggle AI explain panel for selected event |
| `A` | Generate capture summary |
| `D` | Run anomaly detection |
| `P` | Identify protocol |
| `@` | Natural language filter mode |

## Privacy & Security

### Data Sent to APIs

When using cloud providers (OpenAI, Custom), the following data is sent:

**Event Explanation:**
- Selected event and 5 surrounding events
- Fields: transport, direction, src, dst, metadata, payload (first 256 bytes)
- NO complete payloads or sensitive data

**Anomaly Detection:**
- Aggregate statistics (error counts, protocol distribution)
- Sample events (up to 10) with metadata only
- NO individual payloads

**Protocol Identification:**
- First 256 bytes of payload as hex dump
- Transport and addressing metadata

**Natural Language Filters:**
- Only the natural language query text
- NO capture data

### Recommendations

- **Use Ollama for sensitive data** - All processing stays local
- **Review cloud provider terms** - Understand data retention policies
- **Audit AI requests** - Check `/tmp/prb-tui.log` for API calls
- **Disable for compliance** - Set `PRB_AI_API_KEY=""` to disable cloud features

## Troubleshooting

### "MissingApiKey" Error

**Cause:** No API key configured for OpenAI or Custom provider.

**Solution:**
```bash
export PRB_AI_API_KEY="your-key"
# or edit ~/.config/prb/config.toml
```

### Ollama Connection Failed

**Cause:** Ollama service not running or model not pulled.

**Solution:**
```bash
# Start Ollama
ollama serve

# Pull a model
ollama pull llama3.1
```

### Slow Response Times

**Cause:** Large context window or slow model.

**Solutions:**
- Use faster model (gpt-4o-mini vs gpt-4)
- Reduce context window size (edit config)
- Use local Ollama with smaller model

### Rate Limit Errors

**Cause:** Too many API requests.

**Solution:** Wait 60 seconds or upgrade API plan. Natural language filters have built-in rate limiting (10 req/60s).

## Performance

### Streaming vs Blocking

- **Event Explanation:** Streaming - results appear as generated
- **Anomaly Detection:** Blocking - analyzes all events before returning
- **Protocol ID:** Blocking - single response
- **Natural Language Filters:** Blocking - returns validated filter

### Caching

Event explanations are cached in memory per event ID. Navigate back to previously explained events for instant display.

### Background Processing

AI operations run in background threads. TUI remains responsive during analysis.

## Examples

### Debugging gRPC Errors

```bash
# 1. Open capture
prb tui grpc-errors.pcap

# 2. Filter to errors
/
grpc.status != 0
<Enter>

# 3. Explain first error
<select event>
a

# 4. Check for patterns
A  # capture summary

# 5. Detect more anomalies
D
```

### Identifying Unknown Protocol

```bash
# 1. Open capture
prb tui unknown.pcap

# 2. Select unknown event
<navigate to event>

# 3. Identify protocol
P

# 4. Review hex dump with hint
<Tab to hex pane>
```

### Natural Language Exploration

```bash
# 1. Open capture
prb tui mixed.pcap

# 2. Ask questions
@
"show DNS queries"
<Enter>

@
"find failed gRPC calls"
<Enter>

@
"large payloads over 10KB"
<Enter>
```

## API Rate Limits

Default limits to prevent abuse:

| Feature | Limit |
|---------|-------|
| Event Explanation | Unlimited (cached) |
| Capture Summary | 1 per session |
| Anomaly Detection | 1 per minute |
| Protocol ID | 10 per minute |
| Natural Language Filters | 10 per minute |

Override in config:
```toml
[ai]
rate_limit_per_minute = 20
```

## Cost Estimation (OpenAI)

Approximate token usage per feature:

| Feature | Input Tokens | Output Tokens | Cost (gpt-4o-mini) |
|---------|-------------|---------------|---------------------|
| Event Explanation | 2,000 | 500 | $0.0004 |
| Capture Summary | 5,000 | 1,000 | $0.0010 |
| Anomaly Detection | 3,000 | 800 | $0.0006 |
| Protocol ID | 1,000 | 300 | $0.0002 |
| NL Filter | 500 | 100 | $0.0001 |

**Note:** Ollama is free and unlimited. Use for cost-sensitive workflows.

## Future Features

Planned enhancements:
- Multi-event correlation analysis
- Automated regression detection
- Performance baseline comparison
- Custom prompt templates
- Local model fine-tuning
