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
    "id": "monitors_screen",
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
    "x": 18.62,
    "y": 94.66,
    "w": 82.17,
    "h": 28.5,
    "text": "Monitors",
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
    "id": "mon_1",
    "x": 16.0,
    "y": 175.1,
    "w": 374.0,
    "h": 106.67,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "mon_1_cmd",
    "x": 27.11,
    "y": 191.1,
    "w": 116.36,
    "h": 25.92,
    "text": "cargo test -q",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 14,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "mon_1_state",
    "x": 25.52,
    "y": 239.53,
    "w": 63.26,
    "h": 24.23,
    "text": "fired 3\u00d7",
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
    "id": "mon_1_int",
    "x": 214.94,
    "y": 237.92,
    "w": 35.82,
    "h": 27.45,
    "text": "30s",
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
    "id": "mon_1_pause",
    "x": 296.0,
    "y": 233.53,
    "w": 18.0,
    "h": 20.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "mon_1_trash",
    "x": 348.7,
    "y": 233.53,
    "w": 18.0,
    "h": 20.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "mon_2",
    "x": 16.0,
    "y": 348.21,
    "w": 374.0,
    "h": 105.58,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "mon_2_cmd",
    "x": 27.29,
    "y": 364.21,
    "w": 158.65,
    "h": 25.62,
    "text": "tail -n 50 app.log",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 14,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "mon_2_state",
    "x": 26.93,
    "y": 413.34,
    "w": 77.79,
    "h": 22.46,
    "text": "no change",
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
    "id": "mon_2_int",
    "x": 216.65,
    "y": 409.96,
    "w": 32.41,
    "h": 23.79,
    "text": "30s",
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
    "id": "mon_2_pause",
    "x": 296.0,
    "y": 407.34,
    "w": 18.0,
    "h": 20.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "mon_2_trash",
    "x": 348.7,
    "y": 407.34,
    "w": 18.0,
    "h": 20.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "monitors_footer",
    "x": 16.0,
    "y": 506.68,
    "w": 374.0,
    "h": 50.62,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "monitors_footer_label",
    "x": 57.66,
    "y": 516.68,
    "w": 268.53,
    "h": 30.62,
    "text": "Monitors fire when the output changes",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 13,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
