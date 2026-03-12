# Chat UI/UX Patterns for Developer Tools - Research Report

## Executive Summary

This research synthesizes UI/UX patterns from leading developer tools including GitHub Copilot, VS Code, Cursor, Warp Terminal, and others to provide recommendations for implementing chat interfaces in developer environments, particularly terminal-based tools.

---

## 1. LAYOUT PATTERNS

### 1.1 Fixed Bottom Bar vs Collapsible Panel

**Fixed Bottom Bar (Recommended for TUIs)**
- **Examples**: Warp Terminal, Fig
- **Advantages**:
  - Always accessible without mode switching
  - Minimal visual disruption to main content
  - Quick access with keyboard shortcuts
  - Clear separation between chat and work area
- **Implementation**:
  - Fixed height bar (3-5 lines when inactive, expandable to 30-50% on focus)
  - Docked at bottom of terminal window
  - Expands upward when user types, showing history
- **Best for**: Terminal emulators, TUI applications where context needs to remain visible

**Collapsible Side Panel**
- **Examples**: GitHub Copilot Chat (VS Code), Cursor sidebar
- **Advantages**:
  - More screen real estate for chat history
  - Side-by-side code viewing
  - Rich UI with buttons, syntax highlighting
- **Disadvantages**:
  - Not practical for terminal-based UIs
  - Requires horizontal space (typically 300-400px)
- **Best for**: GUI IDEs with horizontal space

**Overlay/Modal**
- **Examples**: Command palettes (Cmd+K interfaces)
- **Advantages**:
  - Full focus on chat interaction
  - Can be dismissed quickly
  - Works well for one-off queries
- **Disadvantages**:
  - Blocks underlying content
  - Not suitable for ongoing conversation
- **Best for**: Quick queries, command execution

### 1.2 Terminal-Specific Layout Recommendation

For a TUI tool like `prb-tui`, **fixed bottom bar** is optimal:

```
┌─────────────────────────────────────────┐
│ Main Content Area                        │
│ (Event list, hex dump, etc.)            │
│                                          │
│                                          │
│                                          │
├─────────────────────────────────────────┤ ← Resizable divider
│ Chat History (when expanded)            │
│ Agent: Analysis shows...                │
│ You: What caused this error?            │
├─────────────────────────────────────────┤
│ > Type message... (Ctrl+/ to focus)     │ ← Input bar (always visible)
└─────────────────────────────────────────┘
```

**Key Properties**:
- Input bar: 1-3 lines high (auto-expand as user types)
- History panel: 0 lines (collapsed) to 50% of screen (expanded)
- Transitions smoothly between states
- Main content adjusts dynamically

---

## 2. VISUAL DISTINCTION (User vs Agent Messages)

### 2.1 Message Display Patterns

**Prefix-Based (Recommended for TUI)**
```
You: Can you explain this packet?
AI:  This is a TCP SYN packet initiating a connection...
You: What about the next one?
AI:  That's the SYN-ACK response...
```
**Advantages**:
- Simple, text-only (perfect for terminals)
- No color dependencies
- Works with screen readers
- Scannable

**Color-Coded Backgrounds**
- **Example**: VS Code Copilot Chat
```
┌────────────────────────────────┐
│ ░░░░ You: Question here ░░░░░░ │ ← Light gray background
│                                │
│ ████ AI: Answer here... ████ │ ← Darker/accent color
└────────────────────────────────┘
```
**Advantages**: Strong visual separation
**Disadvantages**: Requires color support, harder in TUI

**Avatar/Icon Based**
```
👤 You: Can you help with...
🤖 AI: Sure! Here's how...
```
**Advantages**: Friendly, clear
**Disadvantages**: Unicode support required, can look unprofessional

**Alignment-Based**
```
                    You: Question? ⎤
                                   ⎦
⎡ AI: Response here...
⎣
```
**Disadvantages**: Wastes horizontal space, harder to read in terminals

### 2.2 TUI-Specific Recommendation

Use **prefix + subtle color** combination:
- `You:` in default text color
- `AI:` in accent color (cyan/blue)
- Optional: Add box drawing characters for separation
```
┌─ You ────────────────────────────────┐
│ What protocol is this?                │
└───────────────────────────────────────┘
┌─ AI ─────────────────────────────────┐
│ This appears to be MQTT. The packet   │
│ shows a CONNECT message...            │
└───────────────────────────────────────┘
```

---

## 3. INPUT PATTERNS

### 3.1 Input Field Design

**Textarea vs Single-Line Input**

**Single-line with auto-expand (Recommended)**
- **Examples**: Slack, Discord, modern chat apps
- Start as 1 line, expand to 3-5 lines as user types
- Shift+Enter for newlines (Enter sends)
- **Best for**: Quick queries (80% of use cases)

**Multi-line textarea**
- **Examples**: GitHub comment boxes
- Fixed multi-line from start
- Explicit "Send" button required
- **Best for**: Long-form content, code snippets

**Hybrid approach (Best for developer tools)**:
```
Initially: > Type message... [Ctrl+Enter to send]
           ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛

When typing:
> Can you explain
  this packet in detail
  and show the headers? [Ctrl+Enter]
  ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛
```

### 3.2 Send Mechanism

**Enter vs Ctrl+Enter**

Developer tools trend toward **Enter = newline, Ctrl+Enter = send**:
- Reasoning: Developers often paste multi-line code
- Example: VS Code Copilot, GitHub comment boxes
- Trade-off: Requires explicit send action

**Alternative (Recommended for TUI)**:
- **Enter = send** (single-line mode, 90% of queries)
- **Shift+Enter = newline** (switches to multi-line mode)
- Visual indicator: `[Enter↵] Send  [Shift+↵] New line`

**Send Button**:
- Not practical in TUI
- Use keyboard shortcuts only
- Provide clear hints in placeholder text

### 3.3 Focus Management

**Auto-focus patterns**:
1. **On keyboard shortcut** (e.g., `Ctrl+/`, `Ctrl+K`)
   - Recommended: Makes chat easily accessible
2. **On chat panel open**
   - Natural expectation
3. **After message sent**
   - Keeps input focused for follow-up
4. **Never auto-focus on app launch**
   - Avoid interrupting user's workflow

**Focus indicators**:
- Cursor in input field (standard)
- Border highlight (accent color)
- Status bar hint: `Chat focused - Esc to close`

---

## 4. STATE MANAGEMENT

### 4.1 Message Persistence

**Session-based (Minimum)**
```rust
struct ChatSession {
    messages: Vec<Message>,
    session_id: Uuid,
    created_at: SystemTime,
}

// Persist for current app session only
// Clear on app restart
```

**File-based persistence (Recommended)**
```
~/.prb/chat_history/
  ├── session_2024-03-12_143022.json
  ├── session_2024-03-12_151545.json
  └── current.json (symlink to active session)

Format:
{
  "session_id": "uuid",
  "created_at": "2024-03-12T14:30:22Z",
  "messages": [
    {"role": "user", "content": "...", "timestamp": "..."},
    {"role": "assistant", "content": "...", "timestamp": "..."}
  ]
}
```

**Best practices**:
- Auto-save after each message exchange
- Limit history to last N sessions (e.g., 50)
- Provide `/clear` command to reset current session
- Support `/history` command to browse past sessions

### 4.2 Showing History

**Scroll behavior**:
- Auto-scroll to bottom on new message
- Lock scroll if user scrolls up (inspecting history)
- Show "New message ↓" indicator when scrolled up
- Home/End keys for quick navigation

**Truncation**:
- Show last 50-100 messages by default
- "Load more..." option at top of history
- Or: Infinite scroll with virtualization

**Search in history**:
- `/search <term>` command
- Or: Dedicated search box above chat history
- Highlight matches, jump between results

---

## 5. ERROR HANDLING

### 5.1 Message Send Failures

**Network/API errors**:
```
You: Explain this packet
╔════════════════════════════════════╗
║ ⚠️  Message failed to send         ║
║ Error: Connection timeout          ║
║ [Retry] [Cancel]                   ║
╚════════════════════════════════════╝
```

**In TUI, show inline error**:
```
You: Explain this packet
AI:  ❌ Failed to send (timeout). Press 'r' to retry.
```

**Patterns**:
1. **Show error inline** (don't lose user's message)
2. **Provide retry mechanism** (automatic or manual)
3. **Keep message in input field** on failure (allow edit)
4. **Log errors** for debugging

### 5.2 Streaming Response Failures

**Mid-stream errors**:
```
AI: This is a TCP packet with the following
    characteristics:
    - Source port: 443
    - Destination port:

    ⚠️  Stream interrupted (connection lost)
    Partial response saved. [Retry from beginning]
```

**Patterns**:
1. **Preserve partial response** (may still be useful)
2. **Clear error indication** (don't silently fail)
3. **Offer retry** (from beginning, not resume)

### 5.3 Empty/Invalid Responses

```
You: Explain this
AI:  No response received. The AI service may be unavailable.
     Try again in a moment.
```

**Validation**:
- Trim whitespace from user input
- Reject empty messages (show hint: "Message cannot be empty")
- Handle malformed API responses gracefully

---

## 6. REAL-WORLD EXAMPLES

### 6.1 GitHub Copilot Chat (VS Code)

**Layout**: Side panel (300px wide)

**Message display**:
- User messages: light gray background, right-aligned avatar
- AI messages: darker background, left-aligned avatar
- Code blocks: syntax highlighted, copy button

**Input**:
- Multi-line textarea
- Enter = newline, Ctrl+Enter = send
- Inline suggestions as you type

**Features**:
- `/explain`, `/fix`, `/tests` commands
- Context awareness (current file, selection)
- Inline code actions

### 6.2 Cursor

**Layout**: Side panel or inline (toggle)

**Message display**:
- Markdown rendering
- Code diffs shown inline
- "Apply" button for code suggestions

**Input**:
- Cmd+K opens inline chat at cursor
- Cmd+L opens side panel chat
- Context: automatically includes current file

**Features**:
- Multi-file editing
- "Accept" / "Reject" for changes
- Chat history persisted per project

### 6.3 Warp Terminal

**Layout**: Bottom panel (expandable)

**Message display**:
- Command suggestions inline
- AI explanations in bottom drawer
- Plain text + basic formatting

**Input**:
- `#` prefix triggers AI mode
- Tab completion for common queries
- Enter sends immediately

**Features**:
- Command history search
- Explain errors from terminal output
- Generate commands from natural language

### 6.4 Terminal-based Chat UIs (General patterns)

**K9s (Kubernetes TUI)** - command mode:
- Bottom bar shows commands
- `:` prefix for commands
- Autocomplete dropdown

**Vim/Neovim** with copilot:
- Floating window for suggestions
- `<Tab>` to accept
- Ghost text for inline suggestions

**Helix editor** - command palette:
- Bottom command line
- Space bar activates palette
- Fuzzy search through actions

---

## 7. RECOMMENDATIONS FOR `prb-tui`

### 7.1 Layout Choice

**Recommended: Fixed bottom bar with expandable history**

**Rationale**:
- Maintains context of packet data (primary focus)
- Always accessible via keyboard shortcut
- Minimal visual disruption
- Follows terminal emulator conventions (status bars)

**Implementation**:
```
Default state (collapsed):
┌─────────────────────────────────────────┐
│ Event List / Hex Dump / Decode Tree     │
│ ... (main content) ...                  │
│                                          │
├─────────────────────────────────────────┤
│ > Chat (Ctrl+/ to focus, Esc to close)  │ ← 1 line, subtle
└─────────────────────────────────────────┘

Focused state (expanded):
┌─────────────────────────────────────────┐
│ Main content (60% height)                │
│ ...                                      │
├─────────────────────────────────────────┤
│ You: What protocol is this?             │
│ AI:  This is MQTT...                    │
│ You: Explain the payload                │
│ AI:  The payload contains...            │ ← Chat history (30%)
├─────────────────────────────────────────┤
│ > Type message... [↵ Send, Esc Close]  │ ← Input (10%)
└─────────────────────────────────────────┘
```

### 7.2 Visual Distinction

**Recommended: Prefix + color + box characters**

```rust
// Pseudocode
fn render_message(msg: &Message, theme: &Theme) {
    match msg.role {
        Role::User => {
            render_with_style("You: ", theme.accent_color);
            render_text(&msg.content, theme.text_color);
        }
        Role::Assistant => {
            render_with_style("AI:  ", theme.ai_color); // e.g., cyan
            render_text(&msg.content, theme.text_color);
            if msg.is_streaming {
                render_cursor(); // blinking cursor
            }
        }
    }
}
```

**Enhanced version with boxes**:
```
┌─ You ────────────────────────────────┐
│ Explain packet #42                    │
└───────────────────────────────────────┘
┌─ AI ─────────────────────────────────┐
│ Packet #42 is an HTTP GET request    │
│ to example.com/api/data...            │
│ ▌ (streaming)                         │
└───────────────────────────────────────┘
```

### 7.3 Input Pattern

**Recommended: Single-line with auto-expand**

```rust
struct ChatInput {
    content: String,
    cursor_pos: usize,
    lines: usize, // 1-5
}

impl ChatInput {
    fn calculate_lines(&self) -> usize {
        let width = terminal_width() - 4; // padding
        let needed = (self.content.len() / width) + 1;
        needed.min(5).max(1) // 1-5 lines max
    }

    fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.insert_char('\n'); // Multi-line mode
                } else {
                    self.send_message();
                }
            }
            // ... other keys
        }
    }
}
```

**Key bindings**:
- `Ctrl+/` or `Ctrl+I` - Focus chat input
- `Esc` - Close/unfocus chat
- `Enter` - Send message
- `Shift+Enter` - New line (multi-line mode)
- `Ctrl+C` - Cancel current input (clear field)
- `Up/Down` - Navigate history (when input empty)

### 7.4 State Management

**Recommended: File-based with session management**

```rust
// ~/.prb/chat/
// ├── sessions/
// │   ├── 20240312-143022.json
// │   └── 20240312-151545.json
// └── current_session -> sessions/20240312-151545.json

#[derive(Serialize, Deserialize)]
struct ChatSession {
    session_id: String,
    created_at: SystemTime,
    messages: Vec<Message>,
    context: Option<CaptureContext>, // e.g., current capture file
}

impl ChatSession {
    fn save(&self) -> Result<()> {
        let path = chat_dir().join(format!("sessions/{}.json", self.session_id));
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)?;
        Ok(())
    }

    fn auto_save_after_message(&mut self, msg: Message) {
        self.messages.push(msg);
        let _ = self.save(); // Don't fail app on save errors
    }
}
```

**Commands**:
- `/clear` - Clear current session
- `/history` - Show past sessions
- `/load <id>` - Load previous session
- `/export` - Export chat to markdown

### 7.5 Error Handling

**Recommended: Inline errors with retry**

```rust
enum MessageStatus {
    Sending,
    Sent,
    Failed { error: String, retryable: bool },
}

struct Message {
    role: Role,
    content: String,
    status: MessageStatus,
    timestamp: SystemTime,
}

fn render_message_with_status(msg: &Message) {
    match &msg.status {
        MessageStatus::Sending => {
            render_text(&msg.content);
            render_spinner(); // ⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏
        }
        MessageStatus::Sent => {
            render_text(&msg.content);
        }
        MessageStatus::Failed { error, retryable } => {
            render_text(&msg.content);
            render_error(format!("❌ Failed: {}", error));
            if *retryable {
                render_hint("Press 'r' to retry");
            }
        }
    }
}
```

**Error scenarios**:
1. **Network timeout**: Show "❌ Timeout", allow retry
2. **API error**: Show "❌ API error: <details>", allow retry
3. **Streaming interrupted**: Show partial response + "⚠️ Stream interrupted"
4. **Rate limit**: Show "⏳ Rate limited, retry in 30s", auto-retry
5. **Empty response**: Show "⚠️ Empty response received"

### 7.6 Advanced Features (Future)

**Context awareness**:
```rust
struct ChatContext {
    selected_event: Option<EventId>,
    visible_events: Range<usize>,
    current_filter: Option<String>,
    decode_tree: Option<DecodeNode>,
}

// When user asks "explain this", automatically include:
// - Selected event details
// - Surrounding events (context window)
// - Any applied filters
// - Visible decode tree
```

**Commands**:
- `/explain [event_id]` - Explain specific event
- `/why <error>` - Explain errors in capture
- `/filter <query>` - Generate filter from natural language
- `/export chat` - Export conversation to markdown

**Streaming indicators**:
```
AI: This is a TCP SYN packet with ▌
AI: This is a TCP SYN packet with the following ▌
AI: This is a TCP SYN packet with the following characteristics...▌
```

---

## 8. ACCESSIBILITY CONSIDERATIONS

1. **Keyboard-only navigation** (no mouse required)
   - Tab to cycle focus areas
   - Vim-like j/k for scrolling
   - Clear keyboard shortcuts

2. **Screen reader support**
   - Use semantic labels ("Chat input", "Message from AI")
   - Announce new messages
   - Describe streaming state

3. **Color-blind friendly**
   - Don't rely solely on color for user/AI distinction
   - Use prefixes, borders, or icons
   - Provide theme options

4. **Low-bandwidth consideration**
   - Cache responses
   - Show progress indicators
   - Allow cancelling requests

---

## 9. PERFORMANCE CONSIDERATIONS

1. **Virtualization** for long chat history
   - Render only visible messages
   - Lazy load older messages
   - Keep last N messages in memory

2. **Streaming optimization**
   - Update UI in batches (not per-character)
   - Use efficient string building (StringBuilder)
   - Debounce render updates (60 FPS max)

3. **Memory management**
   - Limit total messages in memory (e.g., 200)
   - Persist older messages to disk
   - Clear cache on session end

---

## 10. SUMMARY: KEY DECISIONS

| Aspect | Recommendation | Rationale |
|--------|---------------|-----------|
| **Layout** | Fixed bottom bar (expandable) | Maintains context, terminal-friendly |
| **Visual distinction** | Prefix + subtle color + boxes | Clear, text-based, accessible |
| **Input** | Single-line with auto-expand | Handles 90% of queries, can expand |
| **Send mechanism** | Enter = send, Shift+Enter = newline | Fast for single-line, supports multi-line |
| **Persistence** | File-based sessions | Survives restarts, browsable history |
| **Error handling** | Inline with retry option | Clear feedback, doesn't interrupt flow |
| **Focus shortcut** | `Ctrl+/` or `Ctrl+I` | Common in terminal tools |
| **Streaming** | Show cursor/spinner, batch updates | Clear indication, performant |

---

## 11. IMPLEMENTATION CHECKLIST

**Phase 1: Basic Chat (MVP)**
- [ ] Bottom bar layout with fixed height
- [ ] Input field with basic text entry
- [ ] Send message on Enter
- [ ] Display user/AI messages with prefixes
- [ ] Simple in-memory history (session only)
- [ ] Ctrl+/ to focus, Esc to close

**Phase 2: Enhanced UX**
- [ ] Auto-expand input (1-5 lines)
- [ ] Shift+Enter for multi-line
- [ ] Message history scrolling (j/k, PgUp/PgDn)
- [ ] File-based persistence
- [ ] Error handling with retry
- [ ] Streaming with progress indicator

**Phase 3: Advanced Features**
- [ ] Context awareness (selected event)
- [ ] Commands (/explain, /filter, etc.)
- [ ] Chat history browser
- [ ] Export to markdown
- [ ] Search in history
- [ ] Theme customization

---

## 12. REFERENCES

- GitHub Copilot Chat: https://code.visualstudio.com/docs/copilot/copilot-chat
- VS Code Panel Guidelines: https://code.visualstudio.com/api/ux-guidelines/panel
- Cursor Documentation: https://cursor.sh/docs
- Warp Terminal AI: https://www.warp.dev/ai
- Terminal UI Best Practices: Various Rust TUI examples (ratatui, tui-rs)
- Chat UX Research: Nielsen Norman Group, Material Design guidelines

---

**Report Generated**: 2026-03-12
**For**: `prb-tui` chat feature implementation
**Author**: Research synthesis from multiple sources
