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
    "x": 31.53,
    "y": 48.27,
    "w": 108.37,
    "h": 24.15,
    "text": "Permissions",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 16.1,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t02",
    "x": 45.0,
    "y": 128.96,
    "w": 158.12,
    "h": 26.32,
    "text": "Default permissions",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 17.54,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t03",
    "x": 45.11,
    "y": 168.0,
    "w": 199.62,
    "h": 22.62,
    "text": "Ask before running commands",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15.08,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t04",
    "x": 45.11,
    "y": 200.77,
    "w": 160.14,
    "h": 21.43,
    "text": "that modify your system.",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 14.29,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t05",
    "x": 46.2,
    "y": 280.67,
    "w": 88.05,
    "h": 20.66,
    "text": "Full access",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 13.77,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t06",
    "x": 46.24,
    "y": 319.0,
    "w": 210.89,
    "h": 21.63,
    "text": "Allow Octos to run any command",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 14.42,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t07",
    "x": 45.11,
    "y": 353.0,
    "w": 98.12,
    "h": 21.5,
    "text": "without asking.",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 14.33,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t08",
    "x": 30.45,
    "y": 469.21,
    "w": 60.9,
    "h": 24.81,
    "text": "Model",
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
    "id": "t09",
    "x": 42.86,
    "y": 561.5,
    "w": 110.52,
    "h": 21.63,
    "text": "Default model",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 14.42,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t10",
    "x": 205.21,
    "y": 561.43,
    "w": 150.08,
    "h": 24.22,
    "text": "deepseek-v4-flash v",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 16.14,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
