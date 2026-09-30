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
    "id": "t01",
    "x": 27.84,
    "y": 37.05,
    "w": 101.14,
    "h": 25.11,
    "text": "OctosCode",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 16.74,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t02",
    "x": 36.59,
    "y": 168.31,
    "w": 52.27,
    "h": 28.35,
    "text": "Goal",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 18.9,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t03",
    "x": 38.33,
    "y": 246.26,
    "w": 230.01,
    "h": 24.81,
    "text": "Fix steer queue on reconnect",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 16.54,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t04",
    "x": 50.42,
    "y": 306.24,
    "w": 47.27,
    "h": 21.78,
    "text": "Active",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 14.52,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t05",
    "x": 39.99,
    "y": 419.41,
    "w": 97.76,
    "h": 20.45,
    "text": "Token budget",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 13.63,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t06",
    "x": 266.6,
    "y": 414.58,
    "w": 94.09,
    "h": 23.03,
    "text": "41k of 100k",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15.35,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t07",
    "x": 38.33,
    "y": 542.14,
    "w": 60.99,
    "h": 26.58,
    "text": "Elapsed",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 17.72,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t08",
    "x": 324.1,
    "y": 540.37,
    "w": 36.59,
    "h": 23.03,
    "text": "18m",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15.35,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t09",
    "x": 85.38,
    "y": 675.01,
    "w": 52.27,
    "h": 21.26,
    "text": "Pause",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 14.17,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t10",
    "x": 254.4,
    "y": 673.24,
    "w": 43.56,
    "h": 28.35,
    "text": "Stop",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 18.9,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
