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
    "x": 24.26,
    "y": 33.15,
    "w": 101.34,
    "h": 26.04,
    "text": "OctosCode",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 17.36,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t02",
    "x": 22.65,
    "y": 111.87,
    "w": 116.75,
    "h": 24.86,
    "text": "Code review",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 16.57,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t03",
    "x": 280.29,
    "y": 110.77,
    "w": 94.59,
    "h": 25.29,
    "text": "Start review",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 16.86,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t04",
    "x": 80.15,
    "y": 211.31,
    "w": 228.27,
    "h": 24.86,
    "text": "Reviewing 3 files \u2022 2 specialists",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 16.57,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t05",
    "x": 80.15,
    "y": 252.16,
    "w": 259.63,
    "h": 24.86,
    "text": "Analyzing changes and code quality...",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 16.57,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t06",
    "x": 40.08,
    "y": 371.13,
    "w": 36.59,
    "h": 24.86,
    "text": "High",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 16.57,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t07",
    "x": 33.11,
    "y": 426.18,
    "w": 341.53,
    "h": 23.09,
    "text": "crates/octos-cli/src/api/ui_protocol_transport.rs",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15.39,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t08",
    "x": 33.11,
    "y": 470.57,
    "w": 332.82,
    "h": 21.32,
    "text": "Potential message loss when reconnecting: pending",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 14.21,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t09",
    "x": 33.04,
    "y": 502.1,
    "w": 130.83,
    "h": 23.96,
    "text": "queue was dropped.",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15.97,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t10",
    "x": 38.33,
    "y": 621.51,
    "w": 34.85,
    "h": 19.53,
    "text": "Low",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 13.02,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t11",
    "x": 33.11,
    "y": 674.78,
    "w": 261.37,
    "h": 21.32,
    "text": "crates/octos-core/src/ui_protocol.rs",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 14.21,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t12",
    "x": 33.11,
    "y": 720.95,
    "w": 318.88,
    "h": 23.09,
    "text": "Missing documentation for new preserve_pending",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15.39,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t13",
    "x": 31.15,
    "y": 754.01,
    "w": 59.67,
    "h": 20.91,
    "text": "behavior.",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 13.94,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
