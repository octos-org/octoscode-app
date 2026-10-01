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
    "h": 688,
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
    "w": 50.1,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "tab_tools_fill_t",
    "x": 40,
    "y": 171,
    "w": 34.1,
    "h": 14,
    "text": "Tools",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 11,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "tab_mcp_fill",
    "x": 104,
    "y": 168,
    "w": 91.0,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "tab_mcp_fill_t",
    "x": 112,
    "y": 171,
    "w": 75.0,
    "h": 14,
    "text": "MCP servers",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 11,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
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
    "id": "tool_0",
    "x": 32,
    "y": 230,
    "w": 342,
    "h": 52,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "tool_0_name",
    "x": 44,
    "y": 238,
    "w": 180,
    "h": 16,
    "text": "Bash",
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
    "id": "tool_0_cat",
    "x": 44,
    "y": 258,
    "w": 120,
    "h": 14,
    "text": "shell",
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
    "id": "tool_0_alias",
    "x": 150,
    "y": 238,
    "w": 130,
    "h": 14,
    "text": "Aliases: exec, run",
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
    "id": "tool_0_st",
    "x": 296,
    "y": 236,
    "w": 59.4,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "tool_0_st_t",
    "x": 304,
    "y": 239,
    "w": 43.4,
    "h": 14,
    "text": "enabled",
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
    "id": "tool_0_be",
    "x": 150,
    "y": 256,
    "w": 130,
    "h": 14,
    "text": "Backend: octos",
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
    "id": "tool_0_cnt",
    "x": 340,
    "y": 248,
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
    "id": "tool_1",
    "x": 32,
    "y": 288,
    "w": 342,
    "h": 52,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "tool_1_name",
    "x": 44,
    "y": 296,
    "w": 180,
    "h": 16,
    "text": "Read",
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
    "id": "tool_1_cat",
    "x": 44,
    "y": 316,
    "w": 120,
    "h": 14,
    "text": "fs",
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
    "id": "tool_1_alias",
    "x": 150,
    "y": 296,
    "w": 130,
    "h": 14,
    "text": "Aliases: cat, view",
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
    "id": "tool_1_st",
    "x": 296,
    "y": 294,
    "w": 59.4,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "tool_1_st_t",
    "x": 304,
    "y": 297,
    "w": 43.4,
    "h": 14,
    "text": "enabled",
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
    "id": "tool_1_be",
    "x": 150,
    "y": 314,
    "w": 130,
    "h": 14,
    "text": "Backend: octos",
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
    "id": "tool_1_cnt",
    "x": 340,
    "y": 306,
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
    "id": "tool_2",
    "x": 32,
    "y": 346,
    "w": 342,
    "h": 52,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "tool_2_name",
    "x": 44,
    "y": 354,
    "w": 180,
    "h": 16,
    "text": "ImageView",
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
    "id": "tool_2_cat",
    "x": 44,
    "y": 374,
    "w": 120,
    "h": 14,
    "text": "media",
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
    "id": "tool_2_alias",
    "x": 150,
    "y": 354,
    "w": 130,
    "h": 14,
    "text": "Aliases: see",
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
    "id": "tool_2_st",
    "x": 296,
    "y": 352,
    "w": 65.6,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "tool_2_st_t",
    "x": 304,
    "y": 355,
    "w": 49.6,
    "h": 14,
    "text": "disabled",
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
    "id": "tool_2_be",
    "x": 150,
    "y": 372,
    "w": 130,
    "h": 14,
    "text": "Backend: octos",
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
    "id": "tool_2_cnt",
    "x": 340,
    "y": 364,
    "w": 24,
    "h": 16,
    "text": "1",
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
    "id": "h_mcp",
    "x": 32,
    "y": 410,
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
    "y": 410,
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
    "y": 432,
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
    "y": 440,
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
    "y": 460,
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
    "y": 444,
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
    "y": 447,
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
    "y": 439,
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
    "y": 442,
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
    "y": 440,
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
    "y": 486,
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
    "y": 494,
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
    "y": 514,
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
    "y": 498,
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
    "y": 501,
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
    "y": 493,
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
    "y": 496,
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
    "y": 494,
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
    "id": "empty_tools",
    "x": 236,
    "y": 544,
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
    "y": 562,
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
