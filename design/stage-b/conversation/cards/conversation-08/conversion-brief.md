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
    "id": "composer_input",
    "x": 22.22,
    "y": 158.56,
    "w": 200,
    "h": 46.07,
    "role": "input",
    "native_candidates": [
      "TextInput",
      "KitFormField"
    ]
  },
  {
    "id": "t01",
    "x": 30.215171617750414,
    "y": 166.5588828563699,
    "w": 164.0,
    "h": 32.47,
    "text": "Ask Octos anything",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 21.65,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t02",
    "x": 23.68,
    "y": 257.16,
    "w": 32.71,
    "h": 39.48,
    "text": "+",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 22,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t03",
    "x": 76.67,
    "y": 266.09,
    "w": 102.67,
    "h": 23.36,
    "text": "Ask for approval",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15.57,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t04",
    "x": 213.13,
    "y": 270.62,
    "w": 74.47,
    "h": 20.88,
    "text": "v4-flash \u25be",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 13.92,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t05",
    "x": 30.45,
    "y": 427.48,
    "w": 209.77,
    "h": 25.58,
    "text": "1 queued \u00b7 Steer now \u00b7 \u2715",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 17.05,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t06",
    "x": 29.32,
    "y": 507.5,
    "w": 237.96,
    "h": 24.42,
    "text": "also add a test for reconnect",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 16.28,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t07",
    "x": 25.94,
    "y": 594.41,
    "w": 28.19,
    "h": 36.09,
    "text": "+",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 22,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t08",
    "x": 76.63,
    "y": 602.02,
    "w": 102.74,
    "h": 22.53,
    "text": "Ask for approval",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15.02,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t09",
    "x": 213.08,
    "y": 604.29,
    "w": 74.58,
    "h": 22.5,
    "text": "v4-flash \u25be",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15.0,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
