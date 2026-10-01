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
    "id": "t_title",
    "x": 106.33,
    "y": 158.43,
    "w": 190.11,
    "h": 30.0,
    "text": "Pair with Octos",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 20,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "cal_nosup",
    "x": 28.0,
    "y": 226.5,
    "w": 350.0,
    "h": 78.57,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "cal_info",
    "x": 70.0,
    "y": 240.5,
    "w": 20.0,
    "h": 20.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_cal1",
    "x": 101.5,
    "y": 242.5,
    "w": 207.51,
    "h": 21.0,
    "text": "This server doesn't support",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 14,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_cal2",
    "x": 102.52,
    "y": 268.11,
    "w": 58.87,
    "h": 21.0,
    "text": "pairing.",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 14,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "cal_other",
    "x": 28.0,
    "y": 343.29,
    "w": 350.0,
    "h": 107.37,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_cal3",
    "x": 47.18,
    "y": 359.29,
    "w": 212.41,
    "h": 21.38,
    "text": "Octos on another computer",
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
    "id": "t_cal4",
    "x": 47.37,
    "y": 388.0,
    "w": 152.25,
    "h": 21.0,
    "text": "must be paired from",
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
    "id": "t_cal5",
    "x": 47.3,
    "y": 414.65,
    "w": 114.04,
    "h": 21.0,
    "text": "that computer.",
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
    "id": "pair_fallback",
    "x": 103.0,
    "y": 562.0,
    "w": 200.0,
    "h": 48.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "pair_fallback_surface",
    "x": 103.0,
    "y": 562.0,
    "w": 200.0,
    "h": 48.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "pair_fallback_control",
    "x": 103.0,
    "y": 562.0,
    "w": 200.0,
    "h": 48.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "pair_fallback_label",
    "x": 103.0,
    "y": 576.0,
    "w": 171.0,
    "h": 22.5,
    "text": "Use server and token",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 15,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
