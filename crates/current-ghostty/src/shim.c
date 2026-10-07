// Compile against the exact checked-out Ghostty headers, not stale Rust bindings.
#include <ghostty/vt.h>
#include <string.h>

typedef struct {
  uint8_t r, g, b, valid, bold;
} BenchColor;

GhosttyTerminal bench_new(uint16_t cols, uint16_t rows) {
  GhosttyTerminal t = NULL;
  if (ghostty_terminal_new(NULL, &t, cols, rows) != GHOSTTY_SUCCESS) return NULL;
  size_t limit = 1000;
  if (ghostty_terminal_set(t, GHOSTTY_TERMINAL_OPT_SCROLLBACK_MAX_LINES, &limit) != GHOSTTY_SUCCESS) {
    ghostty_terminal_free(t);
    return NULL;
  }
  return t;
}
void bench_free(GhosttyTerminal t) { ghostty_terminal_free(t); }
void bench_write(GhosttyTerminal t, const uint8_t* bytes, size_t len) { ghostty_terminal_vt_write(t, bytes, len); }
bool bench_resize(GhosttyTerminal t, uint16_t cols, uint16_t rows) {
  return ghostty_terminal_resize(t, cols, rows, 8, 16) == GHOSTTY_SUCCESS;
}
void bench_scroll_top(GhosttyTerminal t) {
  GhosttyTerminalScrollViewport scroll = { .tag = GHOSTTY_SCROLL_VIEWPORT_TOP };
  ghostty_terminal_scroll_viewport(t, scroll);
}
bool bench_cursor(GhosttyTerminal t, uint16_t* x, uint16_t* y) {
  return ghostty_terminal_get(t, GHOSTTY_TERMINAL_DATA_CURSOR_X, x) == GHOSTTY_SUCCESS &&
         ghostty_terminal_get(t, GHOSTTY_TERMINAL_DATA_CURSOR_Y, y) == GHOSTTY_SUCCESS;
}

// Appends a Unicode scalar to the caller's UTF-8 buffer.
static bool append_cp(uint32_t cp, char* out, size_t cap, size_t* n) {
  if (cp <= 0x7f) { if (*n + 1 > cap) return false; out[(*n)++] = (char)cp; }
  else if (cp <= 0x7ff) {
    if (*n + 2 > cap) return false;
    out[(*n)++] = (char)(0xc0 | (cp >> 6)); out[(*n)++] = (char)(0x80 | (cp & 0x3f));
  } else if (cp <= 0xffff) {
    if (*n + 3 > cap) return false;
    out[(*n)++] = (char)(0xe0 | (cp >> 12)); out[(*n)++] = (char)(0x80 | ((cp >> 6) & 0x3f)); out[(*n)++] = (char)(0x80 | (cp & 0x3f));
  } else if (cp <= 0x10ffff) {
    if (*n + 4 > cap) return false;
    out[(*n)++] = (char)(0xf0 | (cp >> 18)); out[(*n)++] = (char)(0x80 | ((cp >> 12) & 0x3f));
    out[(*n)++] = (char)(0x80 | ((cp >> 6) & 0x3f)); out[(*n)++] = (char)(0x80 | (cp & 0x3f));
  } else return false;
  return true;
}

// Returns bytes written, or SIZE_MAX on failure. Rows are newline-separated.
size_t bench_snapshot(GhosttyTerminal t, char* out, size_t cap, BenchColor* first_fg, BenchColor* first_bg) {
  GhosttyRenderState state = NULL;
  GhosttyRenderStateRowIterator rows = NULL;
  GhosttyRenderStateRowCells cells = NULL;
  size_t n = SIZE_MAX;
  if (ghostty_render_state_new(NULL, &state) != GHOSTTY_SUCCESS ||
      ghostty_render_state_update(state, t) != GHOSTTY_SUCCESS ||
      ghostty_render_state_row_iterator_new(NULL, &rows) != GHOSTTY_SUCCESS ||
      ghostty_render_state_row_cells_new(NULL, &cells) != GHOSTTY_SUCCESS ||
      ghostty_render_state_get(state, GHOSTTY_RENDER_STATE_DATA_ROW_ITERATOR, &rows) != GHOSTTY_SUCCESS) goto done;
  n = 0;
  bool first = true;
  while (ghostty_render_state_row_iterator_next(rows)) {
    if (!first) { if (n >= cap) { n = SIZE_MAX; goto done; } out[n++] = '\n'; }
    if (ghostty_render_state_row_get(rows, GHOSTTY_RENDER_STATE_ROW_DATA_CELLS, &cells) != GHOSTTY_SUCCESS) { n = SIZE_MAX; goto done; }
    while (ghostty_render_state_row_cells_next(cells)) {
      if (first && first_fg && first_bg) {
        GhosttyStyle style = GHOSTTY_INIT_SIZED(GhosttyStyle);
        GhosttyColorRgb color;
        if (ghostty_render_state_row_cells_get(cells, GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_STYLE, &style) == GHOSTTY_SUCCESS) {
          first_fg->bold = first_bg->bold = style.bold;
        }
        if (ghostty_render_state_row_cells_get(cells, GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_FG_COLOR, &color) == GHOSTTY_SUCCESS) {
          *first_fg = (BenchColor){ color.r, color.g, color.b, 1, first_fg->bold };
        }
        if (ghostty_render_state_row_cells_get(cells, GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_BG_COLOR, &color) == GHOSTTY_SUCCESS) {
          *first_bg = (BenchColor){ color.r, color.g, color.b, 1, first_bg->bold };
        }
      }
      first = false;
      GhosttyCell raw;
      GhosttyCellWide wide;
      if (ghostty_render_state_row_cells_get(cells, GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_RAW, &raw) != GHOSTTY_SUCCESS ||
          ghostty_cell_get(raw, GHOSTTY_CELL_DATA_WIDE, &wide) != GHOSTTY_SUCCESS) { n = SIZE_MAX; goto done; }
      if (wide == GHOSTTY_CELL_WIDE_SPACER_HEAD || wide == GHOSTTY_CELL_WIDE_SPACER_TAIL) continue;
      uint32_t len = 0;
      if (ghostty_render_state_row_cells_get(cells, GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_GRAPHEMES_LEN, &len) != GHOSTTY_SUCCESS) { n = SIZE_MAX; goto done; }
      if (len == 0) { if (n >= cap) { n = SIZE_MAX; goto done; } out[n++] = ' '; continue; }
      if (len > 64) { n = SIZE_MAX; goto done; }
      uint32_t codepoints[64];
      if (ghostty_render_state_row_cells_get(cells, GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_GRAPHEMES_BUF, codepoints) != GHOSTTY_SUCCESS) { n = SIZE_MAX; goto done; }
      for (uint32_t i = 0; i < len; i++) if (!append_cp(codepoints[i], out, cap, &n)) { n = SIZE_MAX; goto done; }
    }
  }
done:
  ghostty_render_state_row_cells_free(cells);
  ghostty_render_state_row_iterator_free(rows);
  ghostty_render_state_free(state);
  return n;
}
