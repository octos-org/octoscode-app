# Conversion brief

This is an implementation brief, not a claim that these instructions were used
to generate the existing reference. Keep the submitted image prompt unchanged.

Apply MAPPING-RULES.md and mapping-rules.json. Resolve every needs_review/unknown
region. Prefer a matching native kit component, then built-in Makepad widgets,
then a reusable custom widget for missing behavior. Use SVG or cropped Image
assets only for artwork. Never substitute a chart or control with an asset.

For new image generation, include the exact text, font files/family/weights,
layout hierarchy, dimensions, spacing, colors, chart samples/units/domains and
selected control states. Preserve a separate machine-readable manifest. Request
complex illustrations as separate assets, or clearly bounded artwork-only regions
with no overlaid UI text. Do not invent missing numerical values from a mockup.

After generation, measure the actual reference. Requested layout is not measured
evidence. Inspect through Makepad's built-in HTTP instrument with a standalone
release binary; hidden windows support automated tests. See
`flows/core/NATIVE-INSTRUMENT.md`. Run semantic, geometry and visual checks;
legacy Studio capture/gate adapters require their own evidence schema.

```json
[
  {
    "id": "page",
    "x": 0,
    "y": 0,
    "w": 406,
    "h": 776,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "scrim_44",
    "x": 0,
    "y": 0,
    "w": 406,
    "h": 776,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "modal_card",
    "x": 16,
    "y": 44,
    "w": 374,
    "h": 700,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_title",
    "x": 32,
    "y": 62,
    "w": 240,
    "h": 24.4,
    "text": "Runtime inventory",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 20,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_sub",
    "x": 32,
    "y": 88,
    "w": 240,
    "h": 16,
    "text": "12 tools \u00b7 3 servers",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 12,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "f_search_row",
    "x": 32,
    "y": 116,
    "w": 342,
    "h": 40,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "f_search_bg",
    "x": 32,
    "y": 116,
    "w": 342,
    "h": 40,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "f_search_text",
    "x": 44,
    "y": 127.0,
    "w": 318,
    "h": 18,
    "text": "Search names, status, or tools\u2026",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 14,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "f_search",
    "x": 32,
    "y": 116,
    "w": 342,
    "h": 40,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "tab_tools_fill",
    "x": 32,
    "y": 168,
    "w": 60,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "tab_mcp_fill",
    "x": 104,
    "y": 168,
    "w": 92,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "tab_tools_row",
    "x": 32,
    "y": 168,
    "w": 60,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "tab_tools_t",
    "x": 32,
    "y": 168,
    "w": 60,
    "h": 17,
    "text": "Tools",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 13,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "tab_tools",
    "x": 32,
    "y": 168,
    "w": 60,
    "h": 20,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "tab_mcp_row",
    "x": 104,
    "y": 168,
    "w": 92,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "tab_mcp_t",
    "x": 104,
    "y": 168,
    "w": 92,
    "h": 17,
    "text": "MCP servers",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 13,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "tab_mcp",
    "x": 104,
    "y": 168,
    "w": 92,
    "h": 20,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "h_tools",
    "x": 32,
    "y": 208,
    "w": 120,
    "h": 13.42,
    "text": "TOOLS",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 11,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "ch_name",
    "x": 44,
    "y": 219,
    "w": 90,
    "h": 11,
    "text": "TOOL",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 9,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "ch_cat",
    "x": 138,
    "y": 219,
    "w": 52,
    "h": 11,
    "text": "CAT",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 9,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "ch_alias",
    "x": 192,
    "y": 219,
    "w": 82,
    "h": 11,
    "text": "ALIASES / BACKEND",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 9,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "ch_cnt",
    "x": 340,
    "y": 219,
    "w": 24,
    "h": 11,
    "text": "N",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 9,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "tool_0",
    "x": 32,
    "y": 236,
    "w": 342,
    "h": 44,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "tool_0_name",
    "x": 44,
    "y": 242,
    "w": 90,
    "h": 16,
    "text": "Bash",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 12,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "tool_0_cat",
    "x": 138,
    "y": 243,
    "w": 52,
    "h": 14,
    "text": "shell",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 10,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "tool_0_alias",
    "x": 192,
    "y": 242,
    "w": 82,
    "h": 14,
    "text": "Aliases: exec, run",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 10,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "tool_0_st",
    "x": 282,
    "y": 241,
    "w": 55.1,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "tool_0_st_t",
    "x": 290,
    "y": 244,
    "w": 39.1,
    "h": 14,
    "text": "enabled",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 9,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "tool_0_be",
    "x": 192,
    "y": 258,
    "w": 82,
    "h": 14,
    "text": "Backend: octos",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 10,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "tool_0_cnt",
    "x": 344,
    "y": 249,
    "w": 20,
    "h": 16,
    "text": "12",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 12,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "tool_1",
    "x": 32,
    "y": 286,
    "w": 342,
    "h": 44,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "tool_1_name",
    "x": 44,
    "y": 292,
    "w": 90,
    "h": 16,
    "text": "Read",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 12,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "tool_1_cat",
    "x": 138,
    "y": 293,
    "w": 52,
    "h": 14,
    "text": "fs",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 10,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "tool_1_alias",
    "x": 192,
    "y": 292,
    "w": 82,
    "h": 14,
    "text": "Aliases: cat, view",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 10,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "tool_1_st",
    "x": 282,
    "y": 291,
    "w": 55.1,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "tool_1_st_t",
    "x": 290,
    "y": 294,
    "w": 39.1,
    "h": 14,
    "text": "enabled",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 9,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "tool_1_be",
    "x": 192,
    "y": 308,
    "w": 82,
    "h": 14,
    "text": "Backend: octos",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 10,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "tool_1_cnt",
    "x": 344,
    "y": 299,
    "w": 20,
    "h": 16,
    "text": "4",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 12,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "tool_2",
    "x": 32,
    "y": 336,
    "w": 342,
    "h": 44,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "tool_2_name",
    "x": 44,
    "y": 342,
    "w": 90,
    "h": 16,
    "text": "Write",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 12,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "tool_2_cat",
    "x": 138,
    "y": 343,
    "w": 52,
    "h": 14,
    "text": "fs",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 10,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "tool_2_alias",
    "x": 192,
    "y": 342,
    "w": 82,
    "h": 14,
    "text": "Aliases: save",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 10,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "tool_2_st",
    "x": 282,
    "y": 341,
    "w": 55.1,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "tool_2_st_t",
    "x": 290,
    "y": 344,
    "w": 39.1,
    "h": 14,
    "text": "enabled",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 9,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "tool_2_be",
    "x": 192,
    "y": 358,
    "w": 82,
    "h": 14,
    "text": "Backend: octos",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 10,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "tool_2_cnt",
    "x": 344,
    "y": 349,
    "w": 20,
    "h": 16,
    "text": "3",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 12,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "tool_3",
    "x": 32,
    "y": 386,
    "w": 342,
    "h": 44,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "tool_3_name",
    "x": 44,
    "y": 392,
    "w": 90,
    "h": 16,
    "text": "Edit",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 12,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "tool_3_cat",
    "x": 138,
    "y": 393,
    "w": 52,
    "h": 14,
    "text": "fs",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 10,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "tool_3_alias",
    "x": 192,
    "y": 392,
    "w": 82,
    "h": 14,
    "text": "Aliases: patch",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 10,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "tool_3_st",
    "x": 282,
    "y": 391,
    "w": 55.1,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "tool_3_st_t",
    "x": 290,
    "y": 394,
    "w": 39.1,
    "h": 14,
    "text": "enabled",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 9,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "tool_3_be",
    "x": 192,
    "y": 408,
    "w": 82,
    "h": 14,
    "text": "Backend: octos",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 10,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "tool_3_cnt",
    "x": 344,
    "y": 399,
    "w": 20,
    "h": 16,
    "text": "8",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 12,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "tool_4",
    "x": 32,
    "y": 436,
    "w": 342,
    "h": 44,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "tool_4_name",
    "x": 44,
    "y": 442,
    "w": 90,
    "h": 16,
    "text": "Grep",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 12,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "tool_4_cat",
    "x": 138,
    "y": 443,
    "w": 52,
    "h": 14,
    "text": "search",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 10,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "tool_4_alias",
    "x": 192,
    "y": 442,
    "w": 82,
    "h": 14,
    "text": "Aliases: find",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 10,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "tool_4_st",
    "x": 282,
    "y": 441,
    "w": 55.1,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "tool_4_st_t",
    "x": 290,
    "y": 444,
    "w": 39.1,
    "h": 14,
    "text": "enabled",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 9,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "tool_4_be",
    "x": 192,
    "y": 458,
    "w": 82,
    "h": 14,
    "text": "Backend: octos",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 10,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "tool_4_cnt",
    "x": 344,
    "y": 449,
    "w": 20,
    "h": 16,
    "text": "6",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 12,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "tool_5",
    "x": 32,
    "y": 486,
    "w": 342,
    "h": 44,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "tool_5_name",
    "x": 44,
    "y": 492,
    "w": 90,
    "h": 16,
    "text": "ImageView",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 12,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "tool_5_cat",
    "x": 138,
    "y": 493,
    "w": 52,
    "h": 14,
    "text": "media",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 10,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "tool_5_alias",
    "x": 192,
    "y": 492,
    "w": 82,
    "h": 14,
    "text": "Aliases: see",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 10,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "tool_5_st",
    "x": 282,
    "y": 491,
    "w": 60.6,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "tool_5_st_t",
    "x": 290,
    "y": 494,
    "w": 44.6,
    "h": 14,
    "text": "disabled",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 9,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "tool_5_be",
    "x": 192,
    "y": 508,
    "w": 82,
    "h": 14,
    "text": "Backend: octos",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 10,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "tool_5_cnt",
    "x": 344,
    "y": 499,
    "w": 20,
    "h": 16,
    "text": "1",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 12,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "h_mcp",
    "x": 32,
    "y": 542,
    "w": 140,
    "h": 13.42,
    "text": "MCP SERVERS",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 11,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "mcp_counts",
    "x": 150,
    "y": 542,
    "w": 224,
    "h": 14,
    "text": "connected \u00b7 2    connecting \u00b7 1    failed \u00b7 0",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 11,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "srv_0",
    "x": 32,
    "y": 564,
    "w": 342,
    "h": 48,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "srv_0_id",
    "x": 44,
    "y": 572,
    "w": 140,
    "h": 16,
    "text": "fs-probe",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 13,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "srv_0_sum",
    "x": 44,
    "y": 592,
    "w": 200,
    "h": 14,
    "text": "workspace file tools",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 11,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "srv_0_dot",
    "x": 262,
    "y": 576,
    "w": 10.0,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "srv_0_dot_t",
    "x": 267,
    "y": 579,
    "w": 0.0,
    "h": 14,
    "text": "",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 6,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "srv_0_tr",
    "x": 200,
    "y": 571,
    "w": 47.0,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "srv_0_tr_t",
    "x": 208,
    "y": 574,
    "w": 31.0,
    "h": 14,
    "text": "stdio",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 10,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "srv_0_tc",
    "x": 340,
    "y": 572,
    "w": 24,
    "h": 16,
    "text": "6",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 13,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "srv_1",
    "x": 32,
    "y": 618,
    "w": 342,
    "h": 48,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "srv_1_id",
    "x": 44,
    "y": 626,
    "w": 140,
    "h": 16,
    "text": "web-probe",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 13,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "srv_1_sum",
    "x": 44,
    "y": 646,
    "w": 200,
    "h": 14,
    "text": "browser + fetch tools",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 11,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "srv_1_dot",
    "x": 262,
    "y": 630,
    "w": 10.0,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "srv_1_dot_t",
    "x": 267,
    "y": 633,
    "w": 0.0,
    "h": 14,
    "text": "",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 6,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "srv_1_tr",
    "x": 200,
    "y": 625,
    "w": 40.8,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "srv_1_tr_t",
    "x": 208,
    "y": 628,
    "w": 24.8,
    "h": 14,
    "text": "http",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 10,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "srv_1_tc",
    "x": 340,
    "y": 626,
    "w": 24,
    "h": 16,
    "text": "12",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 13,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "srv_2",
    "x": 32,
    "y": 672,
    "w": 342,
    "h": 48,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "srv_2_id",
    "x": 44,
    "y": 680,
    "w": 140,
    "h": 16,
    "text": "git-probe",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 13,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "srv_2_sum",
    "x": 44,
    "y": 700,
    "w": 200,
    "h": 14,
    "text": "repository inspection",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 11,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "srv_2_dot",
    "x": 262,
    "y": 684,
    "w": 10.0,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "srv_2_dot_t",
    "x": 267,
    "y": 687,
    "w": 0.0,
    "h": 14,
    "text": "",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 6,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "srv_2_tr",
    "x": 200,
    "y": 679,
    "w": 47.0,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "srv_2_tr_t",
    "x": 208,
    "y": 682,
    "w": 31.0,
    "h": 14,
    "text": "stdio",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 10,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "srv_2_tc",
    "x": 340,
    "y": 680,
    "w": 24,
    "h": 16,
    "text": "4",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 13,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "empty_tools",
    "x": 236,
    "y": 730,
    "w": 138,
    "h": 14,
    "text": "No matching tools.",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 11,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "empty_servers",
    "x": 236,
    "y": 748,
    "w": 138,
    "h": 14,
    "text": "No matching servers.",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 11,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "btn_close_row",
    "x": 330,
    "y": 60,
    "w": 24,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "btn_close_t",
    "x": 330,
    "y": 60,
    "w": 24,
    "h": 18.3,
    "text": "\u2715",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "btn_close",
    "x": 330,
    "y": 60,
    "w": 24,
    "h": 20,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  }
]
```
