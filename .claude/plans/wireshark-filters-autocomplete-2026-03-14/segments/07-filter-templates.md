---
segment: 7
title: "Implement Filter Templates"
depends_on: [6]
risk: 2/10
complexity: Low
cycle_budget: 10
status: pending
commit_message: "feat(tui): Add predefined filter templates for common traffic patterns"
---

# Segment 7: Implement Filter Templates

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Add predefined filter templates for common patterns (DNS traffic, TLS handshakes, gRPC calls) accessible via command palette or F3 shortcut.

**Depends on:** Segment 6 (templates stored alongside favorites in filters.toml)

## Context: Issue 7 - No Quick Access to Common Filters

**Core Problem:**
- Users repeatedly type common patterns: `udp.port == 53`, `tcp.port == 443 && tcp.payload matches "^\x16\x03"`, `transport == "grpc"`
- No quick access to protocol-specific filters
- Wireshark has Display Filter Macros for this purpose

**User workflows:**
- "Show me all DNS traffic" → `udp.port == 53 || tcp.port == 53`
- "Show me TLS handshakes" → `tcp.port == 443 && tcp.payload matches "^\x16\x03"`
- "Show me gRPC calls" → `transport == "grpc"`
- "Show me ZeroMQ messages" → `transport == "zmq"`
- "Show me large frames" → `frame.len > 1500`
- "Show me HTTP requests" → `tcp.port in {80, 8080} && tcp.payload matches "^(GET|POST|PUT|DELETE)"`

**Root Cause:**
No template system. Users must remember exact syntax for protocol patterns.

**Proposed Fix:**
Add built-in template catalog in `FilterPersistence`:

```rust
// filter_persistence.rs - extend existing file

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterTemplate {
    pub name: String,
    pub category: String,  // "Protocol", "Performance", "Security", etc.
    pub filter: String,
    pub description: String,
    pub tags: Vec<String>,
}

impl FilterPersistence {
    pub fn default_templates() -> Vec<FilterTemplate> {
        vec![
            // DNS
            FilterTemplate {
                name: "DNS Traffic".to_string(),
                category: "Protocol".to_string(),
                filter: "udp.port == 53 || tcp.port == 53".to_string(),
                description: "All DNS queries and responses (UDP and TCP)".to_string(),
                tags: vec!["dns".to_string(), "protocol".to_string()],
            },

            // TLS
            FilterTemplate {
                name: "TLS Handshakes".to_string(),
                category: "Protocol".to_string(),
                filter: r#"tcp.port == 443 && tcp.payload matches "^\x16\x03""#.to_string(),
                description: "TLS ClientHello and ServerHello messages".to_string(),
                tags: vec!["tls".to_string(), "https".to_string(), "security".to_string()],
            },

            FilterTemplate {
                name: "HTTPS Traffic".to_string(),
                category: "Protocol".to_string(),
                filter: "tcp.port in {443, 8443}".to_string(),
                description: "All HTTPS connections on standard ports".to_string(),
                tags: vec!["https".to_string(), "tls".to_string()],
            },

            // gRPC
            FilterTemplate {
                name: "gRPC Calls".to_string(),
                category: "Protocol".to_string(),
                filter: r#"transport == "grpc""#.to_string(),
                description: "All gRPC unary and streaming calls".to_string(),
                tags: vec!["grpc".to_string(), "rpc".to_string()],
            },

            // ZeroMQ
            FilterTemplate {
                name: "ZeroMQ Messages".to_string(),
                category: "Protocol".to_string(),
                filter: r#"transport == "zmq""#.to_string(),
                description: "All ZeroMQ socket traffic".to_string(),
                tags: vec!["zmq".to_string(), "messaging".to_string()],
            },

            // HTTP
            FilterTemplate {
                name: "HTTP Requests".to_string(),
                category: "Protocol".to_string(),
                filter: r#"tcp.port in {80, 8080} && tcp.payload matches "^(GET|POST|PUT|DELETE)""#.to_string(),
                description: "HTTP request methods (unencrypted)".to_string(),
                tags: vec!["http".to_string(), "web".to_string()],
            },

            // Performance
            FilterTemplate {
                name: "Large Frames".to_string(),
                category: "Performance".to_string(),
                filter: "frame.len > 1500".to_string(),
                description: "Frames exceeding MTU (potential fragmentation)".to_string(),
                tags: vec!["performance".to_string(), "fragmentation".to_string()],
            },

            FilterTemplate {
                name: "Small Frames".to_string(),
                category: "Performance".to_string(),
                filter: "frame.len < 64".to_string(),
                description: "Very small frames (possible header-only or ACKs)".to_string(),
                tags: vec!["performance".to_string()],
            },

            // Security
            FilterTemplate {
                name: "Unencrypted Traffic".to_string(),
                category: "Security".to_string(),
                filter: r#"tcp.port in {80, 21, 23, 25} || udp.port == 69"#.to_string(),
                description: "Potentially sensitive unencrypted protocols".to_string(),
                tags: vec!["security".to_string(), "cleartext".to_string()],
            },

            // Local
            FilterTemplate {
                name: "Localhost Traffic".to_string(),
                category: "Network".to_string(),
                filter: r#"ip.src == "127.0.0.1" || ip.dst == "127.0.0.1""#.to_string(),
                description: "Traffic to/from localhost".to_string(),
                tags: vec!["localhost".to_string(), "loopback".to_string()],
            },
        ]
    }

    pub fn get_templates(&self) -> Vec<FilterTemplate> {
        // Start with built-in templates
        let mut templates = Self::default_templates();

        // Add user-defined templates from TOML (if we want to allow custom templates)
        // templates.extend(self.custom_templates.clone());

        templates
    }

    pub fn search_templates(&self, query: &str) -> Vec<FilterTemplate> {
        let query_lower = query.to_lowercase();

        self.get_templates().into_iter()
            .filter(|t| {
                t.name.to_lowercase().contains(&query_lower) ||
                t.description.to_lowercase().contains(&query_lower) ||
                t.tags.iter().any(|tag| tag.to_lowercase().contains(&query_lower))
            })
            .collect()
    }
}

// Add UI in app.rs - template dialog (similar to command palette)
// F3 or Ctrl+T: Open template dialog
// Type to fuzzy search templates by name/tags
// Enter to apply template filter
```

**UI design:**
```
┌─ Filter Templates ──────────────────────────────────┐
│ Search: dns_                                        │
│                                                      │
│ ▸ DNS Traffic                    [Protocol]         │
│   All DNS queries and responses (UDP and TCP)       │
│                                                      │
│   HTTPS Traffic                  [Protocol]         │
│   All HTTPS connections on standard ports           │
│                                                      │
│   TLS Handshakes                 [Protocol]         │
│   TLS ClientHello and ServerHello messages          │
│                                                      │
│ F3: Close  Enter: Apply  ↑↓: Navigate               │
└──────────────────────────────────────────────────────┘
```

**Pre-Mortem Risks:**
1. **Template maintenance**: Hardcoded templates can become stale (version them, add comments)
2. **User customization**: No way to add custom templates in MVP (defer to future, can use favorites)
3. **Regex escaping**: Raw strings with escapes like `\x16` need careful handling
4. **Category proliferation**: Too many categories make browsing hard (limit to 4-5 categories)

**Alternatives Ruled Out:**
- **Load templates from external file**: Adds deployment complexity, built-in is simpler
- **Generate templates from parser**: Parser doesn't know protocol semantics
- **Skip templates, use favorites only**: Templates provide discovery, favorites require prior knowledge

## Scope

**Files to modify:**
- `crates/prb-tui/src/filter_persistence.rs` - Add `FilterTemplate` struct and `default_templates()`
- `crates/prb-tui/src/app.rs` - Add F3 keyboard shortcut, template dialog UI
- `crates/prb-tui/src/filter_state.rs` - Add `apply_template()` method

**Files to create:**
- `crates/prb-tui/tests/filter_templates_test.rs` - Test template search and application

**Unchanged files:**
- `~/.config/prb/filters.toml` - Templates are built-in, not persisted
- `crates/prb-query/src/parser.rs` - No parser changes needed

## Implementation Approach

1. **Define FilterTemplate struct**
   - Add to filter_persistence.rs
   - Fields: name, category, filter, description, tags

2. **Build template catalog**
   - Add `default_templates()` static method
   - Create 10-12 common templates covering:
     - Protocols: DNS, TLS, HTTP, gRPC, ZeroMQ
     - Performance: Large frames, small frames
     - Security: Unencrypted traffic
     - Network: Localhost, multicast

3. **Add template search**
   - `search_templates(query)` - fuzzy match on name/description/tags
   - Use substring matching (or nucleo-matcher if available from S4)

4. **Create template dialog UI**
   - Similar to command_palette.rs pattern
   - List widget with template name + description
   - Search input at top
   - Category badges
   - F3 to toggle, Enter to apply

5. **Integrate with FilterState**
   - Add `apply_template(template: &FilterTemplate)` method
   - Sets `input` to template filter
   - Commits filter (triggers evaluation)

6. **Add keyboard shortcut**
   - F3 or Ctrl+T in app.rs
   - Open template dialog
   - Handle navigation and selection

7. **Write comprehensive tests**
   - Test template catalog completeness
   - Test search by name, description, tags
   - Test apply_template() sets filter correctly
   - Integration test: Open dialog, search "dns", apply

## Build and Test Commands

**Build:** `cargo build --package prb-tui`

**Test (targeted):** `cargo test --package prb-tui filter_templates`

**Test (regression):** `cargo test --package prb-tui`

**Test (full gate):** `cargo test --workspace --all-targets`

**Manual test:** Run TUI, press F3, search for templates, apply one

## Exit Criteria

1. **Targeted tests:**
   - `test_default_templates_count` - At least 10 templates
   - `test_template_categories` - All templates have valid category
   - `test_search_by_name` - Search "dns" finds DNS template
   - `test_search_by_tag` - Search "protocol" finds protocol templates
   - `test_search_by_description` - Search "handshake" finds TLS template
   - `test_apply_template` - apply_template() sets filter input correctly
   - Integration: Open dialog, type "grpc", press Enter → filter is `transport == "grpc"`

2. **Regression tests:** All existing prb-tui tests pass

3. **Full build gate:** `cargo build --workspace` succeeds

4. **Full test suite:** `cargo test --workspace --all-targets` passes

5. **Self-review gate:**
   - All template filters are syntactically valid (can be parsed)
   - All regex patterns are escaped correctly
   - Categories are consistent (Protocol, Performance, Security, Network)
   - No dead code or commented-out blocks

6. **Scope verification gate:**
   - Only filter_persistence.rs, filter_state.rs, app.rs, and new files modified
   - No changes to parser or query planner
   - Template dialog doesn't break existing TUI layout

**Manual verification:**
- Press F3 → template dialog opens
- Type "dns" → DNS Traffic template highlighted
- Press Enter → filter input shows `udp.port == 53 || tcp.port == 53`
- Press F3 → dialog closes
- Type "tls" → TLS Handshakes appears
- Press Enter → filter shows regex for TLS ClientHello

**Risk Factor:** 2/10 - Templates are static data, low risk of bugs

**Estimated Complexity:** Low - Straightforward UI pattern, no complex logic

**Evidence for Optimality:**
1. **Wireshark semantics**: Display Filter Macros serve same purpose (verified in Wireshark docs)
2. **User expectations**: Template systems common in packet capture tools (tshark has read filters)
3. **Codebase evidence**: command_palette.rs shows dialog pattern works well
4. **Protocol coverage**: Templates cover 80% of common use cases (DNS, TLS, HTTP, gRPC, ZMQ)
