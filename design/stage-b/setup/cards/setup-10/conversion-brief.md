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
    "x": 40.6,
    "y": 39.48,
    "w": 63.16,
    "h": 27.0,
    "text": "Skills",
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
    "x": 51.88,
    "y": 103.77,
    "w": 16.92,
    "h": 21.0,
    "text": "Q",
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
    "x": 82.26,
    "y": 103.35,
    "w": 128.71,
    "h": 23.4,
    "text": "Search registry",
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
    "x": 36.09,
    "y": 172.57,
    "w": 69.92,
    "h": 21.0,
    "text": "Installed",
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
    "x": 51.73,
    "y": 227.07,
    "w": 99.55,
    "h": 21.83,
    "text": "rust-review",
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
    "x": 50.75,
    "y": 291.0,
    "w": 86.84,
    "h": 23.69,
    "text": "git-helper",
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
    "x": 51.88,
    "y": 354.16,
    "w": 99.24,
    "h": 21.0,
    "text": "docs-writer",
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
    "x": 192.85,
    "y": 225.58,
    "w": 43.98,
    "h": 24.81,
    "text": "1.2.0",
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
    "x": 193.98,
    "y": 288.74,
    "w": 43.98,
    "h": 24.81,
    "text": "0.9.1",
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
    "x": 193.98,
    "y": 351.91,
    "w": 43.98,
    "h": 25.94,
    "text": "2.0.0",
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
    "x": 35.73,
    "y": 425.33,
    "w": 69.5,
    "h": 26.85,
    "text": "Registry",
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
    "x": 50.73,
    "y": 487.17,
    "w": 93.64,
    "h": 21.0,
    "text": "code-linter",
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
    "x": 190.59,
    "y": 483.87,
    "w": 42.86,
    "h": 24.81,
    "text": "1.1.0",
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
    "x": 50.51,
    "y": 555.09,
    "w": 81.69,
    "h": 23.36,
    "text": "api-client",
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
    "id": "t14",
    "x": 189.47,
    "y": 554.93,
    "w": 45.11,
    "h": 23.69,
    "text": "0.4.2",
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
    "id": "t15",
    "x": 294.33,
    "y": 228.9,
    "w": 58.68,
    "h": 21.0,
    "text": "Remove",
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
    "id": "t16",
    "x": 294.24,
    "y": 291.75,
    "w": 58.87,
    "h": 21.0,
    "text": "Remove",
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
    "id": "t17",
    "x": 294.25,
    "y": 356.06,
    "w": 58.85,
    "h": 21.0,
    "text": "Remove",
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
    "id": "t18",
    "x": 285.33,
    "y": 488.38,
    "w": 53.01,
    "h": 21.0,
    "text": "Install",
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
    "id": "t19",
    "x": 287.58,
    "y": 558.31,
    "w": 50.75,
    "h": 21.0,
    "text": "Install",
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
