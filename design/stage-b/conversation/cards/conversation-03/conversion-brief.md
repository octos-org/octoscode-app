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
    "x": 104.88,
    "y": 60.91,
    "w": 256.01,
    "h": 29.27,
    "text": "Fix the steer queue so queued",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 19.51,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t02",
    "x": 104.88,
    "y": 100.38,
    "w": 226.68,
    "h": 23.34,
    "text": "steers survive a reconnect",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15.56,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t03",
    "x": 64.28,
    "y": 195.0,
    "w": 122.93,
    "h": 28.16,
    "text": "Working \u2022 12s",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 18.77,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t04",
    "x": 21.43,
    "y": 256.0,
    "w": 275.18,
    "h": 27.0,
    "text": "I'm tracing how queued steers are",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 18.0,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t05",
    "x": 23.55,
    "y": 292.75,
    "w": 229.21,
    "h": 22.62,
    "text": "handled across reconnects\u2026",
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
    "id": "t06",
    "x": 21.43,
    "y": 349.5,
    "w": 251.49,
    "h": 22.14,
    "text": "I'll run tests to confirm the fix",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 14.76,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t07",
    "x": 22.56,
    "y": 381.5,
    "w": 280.33,
    "h": 31.47,
    "text": "and update the affected code\u2026",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 20.98,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t08",
    "x": 37.13,
    "y": 509.62,
    "w": 44.15,
    "h": 21.12,
    "text": "octos",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 14.08,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t09",
    "x": 129.69,
    "y": 508.69,
    "w": 43.98,
    "h": 21.93,
    "text": "Local",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 14.62,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t10",
    "x": 228.75,
    "y": 509.34,
    "w": 122.21,
    "h": 22.0,
    "text": "feat/steer-queue",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 14.67,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "composer_input",
    "x": 21.32,
    "y": 593.0,
    "w": 200,
    "h": 41.0,
    "text": "",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 18.0,
    "weight": 400,
    "role": "input",
    "native_candidates": [
      "TextInput",
      "KitFormField"
    ]
  },
  {
    "id": "t11",
    "x": 29.322226521551865,
    "y": 601.0000001745416,
    "w": 163.53,
    "h": 27.0,
    "text": "Ask Octos anything",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 18.0,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t12",
    "x": 19.17,
    "y": 673.36,
    "w": 38.34,
    "h": 47.37,
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
    "id": "t13",
    "x": 73.25,
    "y": 687.76,
    "w": 102.73,
    "h": 22.5,
    "text": "Ask for approval",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15.0,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t14",
    "x": 215.36,
    "y": 691.24,
    "w": 73.39,
    "h": 19.84,
    "text": "v4-flash \u25be",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 13.23,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
