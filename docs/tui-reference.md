# TUI Reference

Complete reference for the interactive Terminal User Interface.

## Overview

The TUI provides real-time exploration of network captures with 8 specialized panes, 13 interactive overlays, and 70+ keyboard shortcuts.

## Panes

### Core Panes

| Pane | Purpose | Focus Key |
|------|---------|-----------|
| **Event List** | Scrollable event table with columns | `Tab` to cycle |
| **Decode Tree** | Hierarchical field tree viewer | `Tab` to cycle |
| **Hex Dump** | Raw byte viewer with ASCII display | `Tab` to cycle |
| **Timeline** | Visual event distribution over time | `Tab` to cycle |

### View Toggles

| Pane | Toggle Key | Purpose |
|------|------------|---------|
| **Waterfall** | `W` | Waterfall visualization (replaces event list) |
| **Conversation List** | `v` | Request/response conversation grouping |
| **Trace Correlation** | `t` | OpenTelemetry trace correlation view |
| **AI Panel** | `a` | AI-powered event explanation panel |

## Input Modes

| Mode | Trigger | Purpose |
|------|---------|---------|
| **Normal** | Default | Standard navigation |
| **Filter** | `/` | Filter expression input |
| **AI Filter** | `@` | Natural language filter generation |
| **Command Palette** | `:` or `Ctrl+P` | Fuzzy command search |
| **Go To Event** | `#` | Jump to event by number |
| **Copy Mode** | `y` | Select data for copying |
| **Help** | `?` | Interactive help overlay |

## Keyboard Shortcuts

### Navigation

| Key | Action |
|-----|--------|
| `j` / `k` / `↓` / `↑` | Navigate events/items |
| `PageUp` / `PageDown` | Page through lists |
| `Home` / `End` | Jump to start/end |
| `#` | Go to event by number |
| `Tab` / `Shift+Tab` | Cycle focus between panes |

### View Management

| Key | Action |
|-----|--------|
| `Esc` | Clear filter/zoom/overlays (progressive) |
| `+` / `=` | Increase focused pane size |
| `-` / `_` | Decrease focused pane size |
| `v` | Toggle conversation view |
| `W` | Toggle waterfall view |
| `t` | Toggle trace correlation view |
| `m` | Toggle metrics overlay |
| `z` | Toggle zoom (full-screen pane) |

### Filtering

| Key | Action |
|-----|--------|
| `/` | Open filter input |
| `F3` | Open filter templates |
| `@` | AI natural language filter mode |
| `↑` / `↓` | Browse filter history (in filter mode) |
| `Ctrl+F` | Toggle filter as favorite (in filter mode) |
| `Tab` or `Enter` | Accept autocomplete suggestion |

**Quick Filter Prefix (`f` key):**
- `f` → `s` - Filter by source address
- `f` → `d` - Filter by destination address
- `f` → `p` - Filter by protocol
- `f` → `c` - Filter by conversation

### AI Features

| Key | Action |
|-----|--------|
| `a` | Toggle AI explain panel for selected event |
| `A` | Generate capture summary |
| `D` | Run anomaly detection |
| `P` | Identify unknown protocol |
| `@` | Natural language to filter expression |

### Export & Data Management

| Key | Action |
|-----|--------|
| `e` | Open export dialog |
| `w` | Quick save filtered view to file |
| `y` | Enter copy mode |

### Session & Configuration

| Key | Action |
|-----|--------|
| `i` | Show session info overlay |
| `L` | Live capture configuration |
| `:` or `Ctrl+P` | Command palette |
| `?` | Toggle help overlay |
| `T` | Theme editor overlay |

### Live Capture Controls

(Only available when running live capture)

| Key | Action |
|-----|--------|
| `S` | Stop capture |
| `P` | Pause/resume capture |
| `f` | Toggle auto-follow mode |

### Control Keys

| Key | Action |
|-----|--------|
| `q` | Quit |
| `Ctrl+C` | Interrupt/cancel operation |

## Mouse Support

### Click Actions

- **Left click on event** - Select event
- **Left click on pane** - Focus pane
- **Left click on tree node** - Expand/collapse
- **Double click** - Quick expand/collapse all children

### Drag Actions

- **Drag vertical border** - Resize event list height
- **Drag horizontal border** - Resize decode tree/hex dump width

### Scroll Actions

- **Scroll wheel** - Scroll content in focused pane
- **Shift+Scroll** - Horizontal scroll (hex dump)

## Overlays

Interactive overlays activated by keyboard shortcuts:

| Overlay | Key | Purpose |
|---------|-----|---------|
| **Welcome** | (first launch) | Getting started guide |
| **Help** | `?` | Context-sensitive keybinding help |
| **Command Palette** | `:` or `Ctrl+P` | Fuzzy command search |
| **Filter Templates** | `F3` | Pre-defined filter library |
| **Export Dialog** | `e` | Export format selector |
| **Session Info** | `i` | Session statistics and metadata |
| **Plugin Manager** | `Ctrl+P` (when in manager) | Plugin installation |
| **Capture Config** | `L` | Live capture settings |
| **Theme Editor** | `T` | Live theme customization |
| **TLS Keylog Picker** | (via command palette) | TLS keylog file browser |
| **Copy Mode** | `y` | Multi-line data selection |
| **Metrics Dashboard** | `m` | Performance metrics |
| **Diff View** | `--diff` flag | Side-by-side capture comparison |

## Filter System

### Filter Input Features

- **Autocomplete** - Context-aware suggestions for fields, operators, values
- **History** - Up/Down arrows to browse last 50 filters
- **Favorites** - `Ctrl+F` to favorite current filter
- **Templates** - `F3` to load pre-defined filters
- **Incremental Preview** - Real-time match count with 100ms debounce

### Filter Persistence

Filters are automatically saved to `~/.config/prb/filters.toml`:
- **History**: Last 50 filters (FIFO)
- **Favorites**: Up to 100 saved filters with names
- **Templates**: Shared filter library

### Autocomplete Behavior

- Type to see suggestions
- `↑` / `↓` to navigate suggestions
- `Tab` or `Enter` to accept selected suggestion
- `Esc` to dismiss dropdown

## Theme System

### Built-in Themes

Cycle with theme toggle key (default: `T`):

1. Dark (default)
2. Light
3. Solarized Dark
4. Monokai
5. Catppuccin Mocha
6. Dracula
7. Colorblind Safe (accessible)
8. Deuteranopia (red-green colorblind)
9. Protanopia (red-green colorblind variant)
10. Tritanopia (blue-yellow colorblind)
11. High Contrast

### Theme Customization

- Press `T` to open theme editor
- Adjust colors in real-time
- Save custom theme to config

## Session Management

### Session Save/Restore

Sessions preserve TUI state including:
- Input file path
- Active filter expression
- Scroll position and selected event
- Pane focus and sizes
- View toggles (conversation, waterfall, trace, AI panel)
- TLS keylog path

Save session via command palette (`:`) → "Save session"

Restore with: `prb tui --session <file.json>`

### Session File Format

JSON format with version 1.0 schema. See [Configuration](configuration.md) for details.

## Command Palette

Press `:` or `Ctrl+P` to open fuzzy command search:

Available commands:
- Filter events
- Clear filter
- Save session
- Load session
- Export...
- Toggle conversations
- Toggle waterfall
- Toggle trace view
- Show metrics
- Show help
- Quit

Type to filter commands, `↑`/`↓` to navigate, `Enter` to execute.

## Copy Mode

Press `y` to enter copy mode for extracting data:

1. Navigate to field/data you want to copy
2. Press `y`
3. Select copy format:
   - JSON (selected event)
   - JSON (all filtered events)
   - Hex dump
   - Decoded tree
   - Source address
   - Destination address

Copies to clipboard via OSC 52 escape sequence (works over SSH).

## Performance

### Large Dataset Handling

The TUI efficiently handles large captures:
- **Ring buffer** - Memory-bounded live capture (configurable size)
- **Query planner** - Optimized filter execution with index usage
- **Incremental rendering** - Only visible rows rendered
- **Lazy decoding** - Decode tree computed on-demand

### Metrics Dashboard

Press `m` to view performance metrics:
- Event count and filter matches
- Memory usage
- Decode cache hit rate
- Frame rendering time

## Configuration

### Keybinding Customization

Override default keybindings in `~/.config/prb/config.toml`:

```toml
[tui.keybindings]
quit = "q"
help = "?"
filter = "/"
zoom = "z"
theme_cycle = "T"
```

See [Configuration](configuration.md) for all options.

## Tips & Tricks

### Workflow Shortcuts

- **Quick error filter**: `f` → `p` → select protocol → `grpc.status != 0`
- **Find slow requests**: Filter by `frame.len > 10000` or use AI anomaly detection
- **Trace debugging**: `t` to group by trace, then `a` to explain each span
- **Export findings**: `e` → HAR format for sharing with web team

### Performance Tips

- Use specific filters instead of wildcards
- Enable protocol-specific decoders only when needed
- For very large captures, filter during ingest: `prb ingest --where '...'`
- Use `--demo` flag to test TUI without loading files

### Accessibility

- Use colorblind-safe themes (options 7-10)
- High contrast theme for low-vision users
- Keyboard-only operation (no mouse required)
- Screen reader compatible with terminal accessibility tools
