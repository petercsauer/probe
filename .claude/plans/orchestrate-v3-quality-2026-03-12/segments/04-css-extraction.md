---
segment: 4
title: "Extract CSS to external stylesheet"
depends_on: [1]
cycle_budget: 12
risk: 2
complexity: "Low"
commit_message: "refactor(orchestrate): Extract CSS to external stylesheet with design tokens"
---

# Segment 4: Extract CSS to external stylesheet

## Goal

Extract inline CSS from dashboard.html to external dashboard.css, consolidate design tokens, add static file endpoint.

## Context

`dashboard.html` contains 64 KB of inline CSS (~800 lines in `<style>`) and JavaScript (~1,600 lines). 159 CSS declarations with duplicates. No external stylesheet prevents browser caching.

## Scope

- **Create:** `dashboard.css` (~600 lines after deduplication)
- **Modify:** `dashboard.html` (remove `<style>` block, add `<link>`)
- **Modify:** `monitor.py` (add static file handler ~20 lines)

## Implementation Approach

1. **Create `dashboard.css`:**
   - Extract all content from `<style>...</style>`
   - Consolidate duplicate values into CSS custom properties
   - Add spacing tokens: --space-xs/sm/md/lg/xl
   - Add typography tokens: --font-size-xs/sm/base/lg
   - Organize by sections: Reset → Base → Components → Utilities → Media Queries

2. **Update `dashboard.html`:**
   - Remove `<style>` block
   - Add `<link rel="stylesheet" href="/api/static/dashboard.css">` in `<head>`
   - Keep JavaScript embedded (no changes to `<script>`)

3. **Add static file endpoint to `monitor.py`:**
   ```python
   app.router.add_get("/api/static/{filename}", _handle_static)

   async def _handle_static(request: web.Request) -> web.Response:
       filename = request.match_info["filename"]
       if filename != "dashboard.css":
           return web.Response(status=404)
       css_path = Path(__file__).parent / "dashboard.css"
       content = FileOps.read_text(css_path)
       return web.Response(text=content, content_type="text/css",
                         headers={"Cache-Control": "public, max-age=3600"})
   ```

4. **Consolidate design tokens:**
   - Replace hardcoded rgba(255,255,255,0.04) with --hover-bg
   - Replace font-size: 11px with var(--font-size-sm)
   - Document BEM naming convention in CSS header

## Pre-Mortem Risks

- **Cache invalidation:** Browser caches old CSS
  - Mitigation: Add version query param or ETag headers
- **Path traversal security:** Hardcoded whitelist (only dashboard.css allowed)
  - Mitigation: Security check in handler

## Exit Criteria

1. **Targeted tests:** `curl http://localhost:9876/api/static/dashboard.css` returns CSS
2. **Regression tests:** pytest passes, monitor starts
3. **Full build gate:** No Python syntax errors
4. **Full test gate:** Dashboard loads in browser, styles render correctly
5. **Self-review gate:** No inline styles in HTML
6. **Scope verification gate:** Only dashboard.html, dashboard.css, monitor.py modified

## Commands

```bash
# Build
python -m py_compile scripts/orchestrate_v3/monitor.py

# Test (targeted)
# Manual: start monitor, verify /api/static/dashboard.css returns CSS

# Test (regression)
pytest scripts/orchestrate_v3/ -v

# Test (full gate)
# Manual: open dashboard in browser, verify styles load
```
