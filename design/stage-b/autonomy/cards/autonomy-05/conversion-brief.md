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
    "x": -0.09,
    "y": 60.33,
    "w": 117.65,
    "h": 32.19,
    "text": "5. Monitors",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 21.46,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t02",
    "x": 13.77,
    "y": 132.99,
    "w": 95.13,
    "h": 25.99,
    "text": "OctosCode",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 17.33,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t03",
    "x": 13.72,
    "y": 211.25,
    "w": 84.86,
    "h": 30.16,
    "text": "Monitors",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 20.11,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t04",
    "x": 24.02,
    "y": 314.48,
    "w": 117.81,
    "h": 27.51,
    "text": "cargo test -q",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 18.34,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t05",
    "x": 22.46,
    "y": 366.44,
    "w": 63.92,
    "h": 25.47,
    "text": "fired 3x",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 16.98,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t06",
    "x": 214.23,
    "y": 364.48,
    "w": 34.55,
    "h": 29.4,
    "text": "30s",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 19.6,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t07",
    "x": 24.19,
    "y": 499.7,
    "w": 160.67,
    "h": 25.47,
    "text": "tail -n 50 app.log",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 16.98,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t08",
    "x": 23.98,
    "y": 547.95,
    "w": 79.89,
    "h": 30.87,
    "text": "no change",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 20.58,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t09",
    "x": 215.96,
    "y": 548.69,
    "w": 32.83,
    "h": 25.47,
    "text": "30s",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 16.98,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t10",
    "x": 53.32,
    "y": 666.32,
    "w": 273.45,
    "h": 28.84,
    "text": "Monitors fire when the output changes",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 19.23,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
