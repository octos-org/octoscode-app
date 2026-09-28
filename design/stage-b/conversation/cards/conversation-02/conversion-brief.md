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
    "id": "t01",
    "x": 87.97,
    "y": 272.95,
    "w": 223.3,
    "h": 33.0,
    "text": "What should we",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 22,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t02",
    "x": 100.21,
    "y": 326.07,
    "w": 202.19,
    "h": 34.75,
    "text": "build in octos?",
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
    "x": 34.93,
    "y": 506.36,
    "w": 41.79,
    "h": 18.43,
    "text": "octos",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 12.29,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t04",
    "x": 127.44,
    "y": 503.05,
    "w": 42.86,
    "h": 23.14,
    "text": "Local",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15.43,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t05",
    "x": 236.58,
    "y": 504.28,
    "w": 125.66,
    "h": 22.83,
    "text": "feat/steer-queue",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15.22,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t06",
    "x": 30.37,
    "y": 596.13,
    "w": 164.82,
    "h": 29.17,
    "text": "Ask Octos anything",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 19.45,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t07",
    "x": 21.43,
    "y": 671.1,
    "w": 36.09,
    "h": 43.99,
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
    "x": 75.45,
    "y": 682.92,
    "w": 103.98,
    "h": 23.21,
    "text": "Ask for approval",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15.47,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t09",
    "x": 214.28,
    "y": 686.9,
    "w": 69.92,
    "h": 19.56,
    "text": "v4-flash \u25be",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 13.04,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
