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
    "id": "worked_row_icon",
    "x": 26.0,
    "y": 45.0,
    "w": 18.0,
    "h": 18.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "worked_row_label",
    "x": 25.94,
    "y": 42.86,
    "w": 280.0,
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
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 16,
    "weight": 600,
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
    "id": "t05",
    "x": 54.78,
    "y": 252.0,
    "w": 219.11,
    "h": 32.53,
    "text": "steer_dropped handling.",
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
    "x": 22.56,
    "y": 320.0,
    "w": 341.56,
    "h": 32.5,
    "text": "\u2022 Updated ui_protocol_transport.rs",
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
    "id": "t09",
    "x": 25.94,
    "y": 469.0,
    "w": 256.01,
    "h": 26.15,
    "text": "\u2022 All tests pass: 12 passed.",
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
    "x": 22.56,
    "y": 533.5,
    "w": 364.11,
    "h": 32.5,
    "text": "\u2022 Changes included in commit a6ea8505.",
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
