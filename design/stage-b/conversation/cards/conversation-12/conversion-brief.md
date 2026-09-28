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
    "h": 28.5,
    "text": "Permissions",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 19,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "perm_card",
    "x": 28.0,
    "y": 99.0,
    "w": 350.0,
    "h": 272.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_r1",
    "x": 45.0,
    "y": 128.96,
    "w": 158.12,
    "h": 26.32,
    "text": "Default permissions",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 15,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_d1",
    "x": 45.0,
    "y": 164.0,
    "w": 280.0,
    "h": 44.0,
    "text": "Ask before running commands that modify your system.",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 12,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_r2",
    "x": 46.2,
    "y": 280.67,
    "w": 88.05,
    "h": 22.5,
    "text": "Full access",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 15,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_d2",
    "x": 45.0,
    "y": 316.0,
    "w": 280.0,
    "h": 44.0,
    "text": "Allow Octos to run any command without asking.",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 12,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "toggle1",
    "x": 320.0,
    "y": 128.0,
    "w": 44.0,
    "h": 26.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "toggle1_knob",
    "x": 342.0,
    "y": 131.0,
    "w": 20.0,
    "h": 20.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "toggle2",
    "x": 320.0,
    "y": 280.0,
    "w": 44.0,
    "h": 26.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "toggle2_knob",
    "x": 323.0,
    "y": 283.0,
    "w": 20.0,
    "h": 20.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_sec2",
    "x": 30.45,
    "y": 469.21,
    "w": 60.9,
    "h": 25.5,
    "text": "Model",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 17,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "model_card",
    "x": 28.0,
    "y": 540.0,
    "w": 348.0,
    "h": 72.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_model",
    "x": 42.86,
    "y": 561.5,
    "w": 110.52,
    "h": 22.5,
    "text": "Default model",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 15,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_pick",
    "x": 205.0,
    "y": 561.5,
    "w": 160.0,
    "h": 21.63,
    "text": "deepseek-v4-flash v",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 14,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
