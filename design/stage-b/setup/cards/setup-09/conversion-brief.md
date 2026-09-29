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
    "id": "t00",
    "x": 43.98,
    "y": 40.6,
    "w": 91.35,
    "h": 27.0,
    "text": "Context",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 18,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t01",
    "x": 45.11,
    "y": 101.5,
    "w": 161.27,
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
    "id": "t02",
    "x": 323.67,
    "y": 101.51,
    "w": 37.22,
    "h": 21.0,
    "text": "62%",
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
    "id": "t03",
    "x": 47.37,
    "y": 212.05,
    "w": 58.64,
    "h": 22.56,
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
    "id": "t04",
    "x": 46.14,
    "y": 265.64,
    "w": 105.08,
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
    "id": "t05",
    "x": 45.11,
    "y": 318.07,
    "w": 43.98,
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
    "id": "t06",
    "x": 339.46,
    "y": 212.05,
    "w": 21.43,
    "h": 21.0,
    "text": "8k",
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
    "id": "t07",
    "x": 327.06,
    "y": 266.19,
    "w": 34.96,
    "h": 21.0,
    "text": "96k",
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
    "id": "t08",
    "x": 327.06,
    "y": 318.07,
    "w": 33.83,
    "h": 21.0,
    "text": "20k",
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
    "id": "t09",
    "x": 43.95,
    "y": 428.47,
    "w": 97.05,
    "h": 21.69,
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
    "id": "t10",
    "x": 190.59,
    "y": 428.6,
    "w": 39.47,
    "h": 21.0,
    "text": "LLM",
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
    "id": "t11",
    "x": 283.07,
    "y": 428.6,
    "w": 67.67,
    "h": 21.0,
    "text": "Heuristic",
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
    "id": "t12",
    "x": 136.38,
    "y": 533.08,
    "w": 118.57,
    "h": 23.4,
    "text": "Compact now",
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
    "id": "t13",
    "x": 108.27,
    "y": 594.41,
    "w": 170.29,
    "h": 22.59,
    "text": "Keeps the last 4 turns",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 14,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
