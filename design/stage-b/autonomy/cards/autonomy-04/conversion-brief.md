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
    "x": 4.96,
    "y": 36.23,
    "w": 100.19,
    "h": 24.98,
    "text": "OctosCode",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 16.65,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t02",
    "x": -0.0,
    "y": 116.93,
    "w": 67.09,
    "h": 30.12,
    "text": "Loops",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 20.08,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t03",
    "x": 12.02,
    "y": 210.72,
    "w": 113.58,
    "h": 23.25,
    "text": "Run Cl smoke",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15.5,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t04",
    "x": 11.96,
    "y": 260.1,
    "w": 89.62,
    "h": 23.71,
    "text": "every 15 min",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15.81,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t05",
    "x": 11.87,
    "y": 350.16,
    "w": 88.09,
    "h": 27.84,
    "text": "Sync main",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 18.56,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t06",
    "x": 11.89,
    "y": 399.81,
    "w": 91.47,
    "h": 25.98,
    "text": "every 30 min",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 17.32,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t07",
    "x": 11.8,
    "y": 489.63,
    "w": 120.91,
    "h": 30.62,
    "text": "Nightly review",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 20.41,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t08",
    "x": 11.9,
    "y": 539.54,
    "w": 120.71,
    "h": 24.68,
    "text": "every day \u2022 01:00",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 16.45,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t09",
    "x": 199.56,
    "y": 232.09,
    "w": 24.08,
    "h": 31.89,
    "text": "\u2022",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 21.26,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t10",
    "x": 287.15,
    "y": 116.35,
    "w": 101.8,
    "h": 29.51,
    "text": "+ New loop",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 19.67,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
