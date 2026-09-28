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
    "id": "tool_1",
    "x": 16.0,
    "y": 54.0,
    "w": 374.0,
    "h": 92.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "icon_file",
    "x": 42.0,
    "y": 78.0,
    "w": 24.0,
    "h": 26.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t01",
    "x": 87.97,
    "y": 77.83,
    "w": 231.19,
    "h": 22.67,
    "text": "Read ui_protocol_transport.rs",
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
    "id": "t02",
    "x": 86.84,
    "y": 104.9,
    "w": 78.94,
    "h": 19.5,
    "text": "\u2022 412 lines",
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
    "id": "icon_check1",
    "x": 356.0,
    "y": 75.83,
    "w": 20.0,
    "h": 20.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "tool_2",
    "x": 16.0,
    "y": 184.0,
    "w": 374.0,
    "h": 92.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "icon_search2",
    "x": 40.0,
    "y": 206.0,
    "w": 24.0,
    "h": 24.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t03",
    "x": 87.97,
    "y": 205.28,
    "w": 177.06,
    "h": 24.81,
    "text": "Search 'steer_dropped'",
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
    "id": "t04",
    "x": 86.62,
    "y": 234.57,
    "w": 88.41,
    "h": 20.37,
    "text": "\u2022 7 matches",
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
    "id": "icon_check2",
    "x": 356.0,
    "y": 203.28,
    "w": 20.0,
    "h": 20.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "tool_3",
    "x": 16.0,
    "y": 314.0,
    "w": 374.0,
    "h": 356.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "icon_term",
    "x": 40.0,
    "y": 338.0,
    "w": 22.0,
    "h": 22.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t05",
    "x": 87.5,
    "y": 336.27,
    "w": 270.67,
    "h": 32.33,
    "text": "Ran cargo test -p octos-cli",
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
    "id": "t06",
    "x": 87.66,
    "y": 370.55,
    "w": 89.57,
    "h": 21.0,
    "text": "steer_queue",
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
    "id": "icon_check3",
    "x": 356.0,
    "y": 334.27,
    "w": 20.0,
    "h": 20.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "tool_3_output",
    "x": 24.0,
    "y": 398.0,
    "w": 358.0,
    "h": 250.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t07",
    "x": 45.05,
    "y": 442.86,
    "w": 135.46,
    "h": 22.25,
    "text": "running 12 tests",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 13,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t08",
    "x": 46.0,
    "y": 484.23,
    "w": 112.0,
    "h": 17.55,
    "text": "\u00b7\u00b7\u00b7\u00b7\u00b7\u00b7\u00b7\u00b7\u00b7\u00b7\u00b7\u00b7\u00b7\u00b7\u00b7",
    "font_src": "self:resources/ux/Inter-700.ttf",
    "size": 13,
    "weight": 700,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t09",
    "x": 39.47,
    "y": 515.45,
    "w": 302.24,
    "h": 23.69,
    "text": "test steer_queue::reconnect_ok ... ok",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 13,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t10",
    "x": 42.86,
    "y": 568.47,
    "w": 303.37,
    "h": 23.69,
    "text": "test result: ok. 12 passed; 0 failed",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 13,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
