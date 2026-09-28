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
    "x": 78.94,
    "y": 82.34,
    "w": 246.98,
    "h": 27.16,
    "text": "Octos needs a decision",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 18.11,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t02",
    "x": 37.22,
    "y": 154.5,
    "w": 257.13,
    "h": 28.22,
    "text": "Where should queued steers",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 18.81,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t03",
    "x": 36.09,
    "y": 192.87,
    "w": 121.8,
    "h": 28.2,
    "text": "be persisted?",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 18.8,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t04",
    "x": 75.1,
    "y": 254.59,
    "w": 181.49,
    "h": 31.71,
    "text": "In the session ledger",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 21.14,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t05",
    "x": 76.61,
    "y": 293.91,
    "w": 140.01,
    "h": 25.77,
    "text": "(recommended)",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 17.18,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t06",
    "x": 76.66,
    "y": 355.12,
    "w": 138.78,
    "h": 26.28,
    "text": "In memory only",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 17.52,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t07",
    "x": 79.96,
    "y": 415.52,
    "w": 124.28,
    "h": 22.79,
    "text": "Ask each time",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15.19,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t08",
    "x": 39.26,
    "y": 493.14,
    "w": 92.89,
    "h": 24.33,
    "text": "Add a note",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 16.22,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t09",
    "x": 137.55,
    "y": 590.77,
    "w": 135.42,
    "h": 24.2,
    "text": "Submit answer",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 16.13,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t10",
    "x": 180.44,
    "y": 673.36,
    "w": 42.86,
    "h": 29.33,
    "text": "Skip",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 19.55,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
