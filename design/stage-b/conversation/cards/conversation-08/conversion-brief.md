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
    "id": "composer_idle",
    "x": 16.0,
    "y": 140.0,
    "w": 374.0,
    "h": 160.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "composer_idle_input",
    "x": 30.0,
    "y": 150.0,
    "w": 300.0,
    "h": 40.0,
    "text": "",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15,
    "weight": 400,
    "role": "input",
    "native_candidates": [
      "TextInput",
      "KitFormField"
    ]
  },
  {
    "id": "icon_plus1",
    "x": 30.0,
    "y": 262.0,
    "w": 18.0,
    "h": 24.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "approval_pill1",
    "x": 66.67,
    "y": 260.09,
    "w": 122.67,
    "h": 34.62,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t03",
    "x": 76.67,
    "y": 266.09,
    "w": 102.67,
    "h": 21.62,
    "text": "Ask for approval",
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
    "id": "icon_mic1",
    "x": 306.0,
    "y": 259.0,
    "w": 16.0,
    "h": 27.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t04",
    "x": 213.13,
    "y": 270.62,
    "w": 74.47,
    "h": 19.5,
    "text": "v4-flash \u25be",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 13,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "send1",
    "x": 344.0,
    "y": 252.0,
    "w": 36.0,
    "h": 36.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "icon_send",
    "x": 354.0,
    "y": 262.0,
    "w": 16.0,
    "h": 16.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "queued_row",
    "x": 16.0,
    "y": 417.48,
    "w": 237.77,
    "h": 43.69,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t05",
    "x": 30.45,
    "y": 427.48,
    "w": 209.77,
    "h": 23.69,
    "text": "1 queued \u00b7 Steer now \u00b7 \u2715",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 13,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "composer_active",
    "x": 16.0,
    "y": 480.0,
    "w": 374.0,
    "h": 170.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t06",
    "x": 29.32,
    "y": 507.5,
    "w": 237.96,
    "h": 22.62,
    "text": "also add a test for reconnect",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "icon_plus2",
    "x": 28.0,
    "y": 598.0,
    "w": 18.0,
    "h": 24.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "approval_pill2",
    "x": 66.63,
    "y": 596.02,
    "w": 122.74,
    "h": 33.86,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t08",
    "x": 76.63,
    "y": 602.02,
    "w": 102.74,
    "h": 20.86,
    "text": "Ask for approval",
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
    "id": "icon_mic2",
    "x": 306.0,
    "y": 598.0,
    "w": 16.0,
    "h": 26.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t09",
    "x": 213.08,
    "y": 604.29,
    "w": 74.58,
    "h": 20.84,
    "text": "v4-flash \u25be",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 13,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "stop2",
    "x": 346.0,
    "y": 590.0,
    "w": 36.0,
    "h": 36.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "icon_stop2",
    "x": 356.0,
    "y": 600.0,
    "w": 16.0,
    "h": 16.0,
    "role": "unknown",
    "native_candidates": []
  }
]
```
