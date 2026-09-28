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
    "id": "worked_row",
    "x": 16.0,
    "y": 32.0,
    "w": 374.0,
    "h": 44.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "worked_row_surface",
    "x": 16.0,
    "y": 32.0,
    "w": 374.0,
    "h": 44.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "worked_row_control",
    "x": 16.0,
    "y": 32.0,
    "w": 374.0,
    "h": 44.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "worked_row_label",
    "x": 25.94,
    "y": 42.86,
    "w": 180.44,
    "h": 22.64,
    "text": "Worked for 3m 4s \u203a",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 15.09,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t02",
    "x": 28.04,
    "y": 111.25,
    "w": 322.88,
    "h": 25.28,
    "text": "Queued steers now survive a reconnect.",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 16,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t03",
    "x": 19.33,
    "y": 171.37,
    "w": 312.56,
    "h": 32.33,
    "text": "\u2022 Fixed loss of queued steers when",
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
    "x": 45.11,
    "y": 210.0,
    "w": 241.67,
    "h": 32.5,
    "text": "reconnecting after a drop in",
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
    "id": "t05c",
    "x": 52.5,
    "y": 245.0,
    "w": 133.5,
    "h": 32.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t05c_t",
    "x": 56.5,
    "y": 248.0,
    "w": 125.5,
    "h": 26.0,
    "text": "steer_dropped",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 14,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t05b",
    "x": 182.0,
    "y": 252.0,
    "w": 91.89,
    "h": 32.53,
    "text": " handling.",
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
    "id": "t06a",
    "x": 22.56,
    "y": 320.0,
    "w": 113.94,
    "h": 32.5,
    "text": "\u2022 Updated ",
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
    "id": "t06c",
    "x": 132.5,
    "y": 315.0,
    "w": 238.5,
    "h": 32.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t06c_t",
    "x": 136.5,
    "y": 318.0,
    "w": 230.5,
    "h": 26.0,
    "text": "ui_protocol_transport.rs",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
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
    "x": 49.62,
    "y": 364.31,
    "w": 258.26,
    "h": 24.81,
    "text": "to persist queued steers to the",
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
    "x": 50.61,
    "y": 404.21,
    "w": 127.71,
    "h": 26.24,
    "text": "session ledger.",
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
    "id": "t09a",
    "x": 25.94,
    "y": 469.0,
    "w": 152.06,
    "h": 26.15,
    "text": "\u2022 All tests pass: ",
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
    "id": "t09c",
    "x": 174.0,
    "y": 463.0,
    "w": 97.0,
    "h": 30.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t09c_t",
    "x": 178.0,
    "y": 466.0,
    "w": 89.0,
    "h": 24.0,
    "text": "12 passed",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 14,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t09b",
    "x": 267.0,
    "y": 469.0,
    "w": 14.94,
    "h": 26.15,
    "text": ".",
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
    "id": "t10a",
    "x": 22.56,
    "y": 533.5,
    "w": 267.94,
    "h": 32.5,
    "text": "\u2022 Changes included in commit ",
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
    "id": "t10c",
    "x": 286.5,
    "y": 527.0,
    "w": 89.0,
    "h": 32.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t10c_t",
    "x": 290.5,
    "y": 530.0,
    "w": 81.0,
    "h": 26.0,
    "text": "a6ea8505",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 14,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t10b",
    "x": 371.5,
    "y": 533.5,
    "w": 15.17,
    "h": 32.5,
    "text": ".",
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
    "id": "icon_copy",
    "x": 24.0,
    "y": 640.0,
    "w": 20.0,
    "h": 20.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "icon_thumbs",
    "x": 52.0,
    "y": 640.0,
    "w": 20.0,
    "h": 20.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "icon_share",
    "x": 80.0,
    "y": 640.0,
    "w": 20.0,
    "h": 20.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t11",
    "x": 266.0,
    "y": 664.69,
    "w": 122.11,
    "h": 26.36,
    "text": "Sep 28, 9:41 PM",
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
