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
    "x": 1.28,
    "y": 57.12,
    "w": 82.91,
    "h": 34.68,
    "text": "6. Fleet",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 23.12,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t02",
    "x": 21.32,
    "y": 136.94,
    "w": 105.15,
    "h": 25.94,
    "text": "OctosCode",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 17.29,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t03",
    "x": 26.63,
    "y": 211.21,
    "w": 153.3,
    "h": 36.13,
    "text": "Fleet \u2022 3 peers",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 24.09,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t04",
    "x": 28.53,
    "y": 296.55,
    "w": 117.83,
    "h": 28.02,
    "text": "Fix steer queue",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 18.68,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t05",
    "x": 44.52,
    "y": 397.8,
    "w": 62.32,
    "h": 29.4,
    "text": "Running",
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
    "id": "t06",
    "x": 151.36,
    "y": 384.08,
    "w": 48.08,
    "h": 23.52,
    "text": "tests",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15.68,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t07",
    "x": 151.36,
    "y": 433.07,
    "w": 121.09,
    "h": 21.56,
    "text": "18m \u2022 41k tokens",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 14.37,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t08",
    "x": 340.11,
    "y": 405.64,
    "w": 46.3,
    "h": 23.52,
    "text": "Steer",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15.68,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t09",
    "x": 44.52,
    "y": 536.93,
    "w": 60.54,
    "h": 25.47,
    "text": "Blocked",
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
    "x": 151.36,
    "y": 521.25,
    "w": 44.52,
    "h": 23.52,
    "text": "docs",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15.68,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t11",
    "x": 151.33,
    "y": 568.11,
    "w": 110.46,
    "h": 21.91,
    "text": "7m \u2022 12k tokens",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 14.6,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t12",
    "x": 340.11,
    "y": 538.89,
    "w": 48.08,
    "h": 25.47,
    "text": "Steer",
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
    "id": "t13",
    "x": 44.52,
    "y": 674.1,
    "w": 40.96,
    "h": 23.52,
    "text": "Done",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15.68,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t14",
    "x": 151.36,
    "y": 656.46,
    "w": 56.98,
    "h": 23.52,
    "text": "review",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15.68,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t15",
    "x": 151.36,
    "y": 703.49,
    "w": 133.55,
    "h": 23.52,
    "text": "24m \u2022 28k tokens",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15.68,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t16",
    "x": 341.8,
    "y": 675.85,
    "w": 44.7,
    "h": 23.94,
    "text": "Steer",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15.96,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
