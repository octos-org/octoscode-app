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
    "x": 0.0,
    "y": 0.0,
    "w": 406.0,
    "h": 776.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_title",
    "x": 42.75,
    "y": 37.95,
    "w": 92.68,
    "h": 28.5,
    "text": "Context",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 19,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_usage",
    "x": 42.86,
    "y": 99.26,
    "w": 163.53,
    "h": 21.0,
    "text": "124k of 200k tokens",
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
    "id": "t_pct",
    "x": 324.8,
    "y": 99.26,
    "w": 37.22,
    "h": 19.5,
    "text": "62%",
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
    "id": "bar_track",
    "x": 42.0,
    "y": 128.0,
    "w": 322.0,
    "h": 8.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "bar_fill",
    "x": 42.0,
    "y": 128.0,
    "w": 200.0,
    "h": 8.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_row3",
    "x": 45.11,
    "y": 209.79,
    "w": 59.77,
    "h": 24.81,
    "text": "System",
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
    "id": "t_val6",
    "x": 340.59,
    "y": 210.92,
    "w": 21.43,
    "h": 19.5,
    "text": "8k",
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
    "id": "t_row4",
    "x": 45.06,
    "y": 264.78,
    "w": 106.11,
    "h": 21.0,
    "text": "Conversation",
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
    "id": "t_val7",
    "x": 328.18,
    "y": 263.93,
    "w": 33.83,
    "h": 21.43,
    "text": "96k",
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
    "id": "t_row5",
    "x": 42.86,
    "y": 318.07,
    "w": 46.24,
    "h": 21.0,
    "text": "Tools",
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
    "id": "t_val8",
    "x": 328.18,
    "y": 318.07,
    "w": 34.96,
    "h": 20.3,
    "text": "20k",
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
    "id": "t_comp",
    "x": 42.7,
    "y": 427.93,
    "w": 97.29,
    "h": 23.91,
    "text": "Compaction:",
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
    "id": "seg_pill",
    "x": 182.0,
    "y": 419.0,
    "w": 84.0,
    "h": 32.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_llm",
    "x": 190.59,
    "y": 429.73,
    "w": 39.47,
    "h": 19.5,
    "text": "LLM",
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
    "id": "t_heur",
    "x": 283.03,
    "y": 429.58,
    "w": 68.89,
    "h": 20.62,
    "text": "Heuristic",
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
    "id": "btn_compact",
    "x": 103.0,
    "y": 522.0,
    "w": 200.0,
    "h": 46.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "btn_compact_surface",
    "x": 103.0,
    "y": 522.0,
    "w": 200.0,
    "h": 46.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "btn_compact_control",
    "x": 103.0,
    "y": 522.0,
    "w": 200.0,
    "h": 46.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "btn_compact_label",
    "x": 134.0,
    "y": 534.0,
    "w": 139.0,
    "h": 25.01,
    "text": "Compact now",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 16.67,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_keep",
    "x": 107.14,
    "y": 595.5,
    "w": 172.55,
    "h": 23.72,
    "text": "Keeps the last 4 turns",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 13,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
