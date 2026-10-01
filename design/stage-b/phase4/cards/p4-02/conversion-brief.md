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
    "id": "pair_back_chev",
    "x": 17.0,
    "y": 58.0,
    "w": 14.0,
    "h": 22.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "pair_back_control",
    "x": 7.0,
    "y": 49.0,
    "w": 34.0,
    "h": 40.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "t_title",
    "x": 106.01,
    "y": 97.0,
    "w": 193.98,
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
    "id": "spinner",
    "x": 191.0,
    "y": 288.0,
    "w": 24.0,
    "h": 24.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_pairing",
    "x": 69.92,
    "y": 346.27,
    "w": 261.64,
    "h": 24.0,
    "text": "Pairing with 192.168.1.20...",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 16,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_once",
    "x": 104.88,
    "y": 420.5,
    "w": 198.49,
    "h": 19.5,
    "text": "This code works once.",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 13,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "pair_cancel",
    "x": 133.0,
    "y": 622.0,
    "w": 140.0,
    "h": 44.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "pair_cancel_surface",
    "x": 133.0,
    "y": 622.0,
    "w": 140.0,
    "h": 44.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "pair_cancel_control",
    "x": 133.0,
    "y": 622.0,
    "w": 140.0,
    "h": 44.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "pair_cancel_label",
    "x": 174.0,
    "y": 634.0,
    "w": 61.0,
    "h": 22.5,
    "text": "Cancel",
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
