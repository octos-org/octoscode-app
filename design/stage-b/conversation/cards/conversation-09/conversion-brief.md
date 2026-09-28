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
    "id": "worked_row",
    "x": 17.94,
    "y": 34.86,
    "w": 196.44,
    "h": 38.64,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "t01",
    "x": 25.93889069521046,
    "y": 42.860464842219876,
    "w": 180.44,
    "h": 24.45,
    "text": "Worked for 3m 4s \u203a",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 16.3,
    "weight": 600,
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
    "h": 27.3,
    "text": "Queued steers now survive a reconnect.",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 18.2,
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
    "h": 33.0,
    "text": "\u2022 Fixed loss of queuedsteers when",
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
    "id": "t04",
    "x": 45.11,
    "y": 210.0,
    "w": 241.67,
    "h": 33.0,
    "text": "reconnecting after a drop in",
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
    "id": "t05",
    "x": 54.78,
    "y": 252.0,
    "w": 219.11,
    "h": 33.0,
    "text": "steer_dropped handling.",
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
    "id": "t06",
    "x": 22.56,
    "y": 320.0,
    "w": 341.56,
    "h": 33.0,
    "text": "\u2022 Updated ui_protocol_transport.rs",
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
    "id": "t07",
    "x": 49.62,
    "y": 364.31,
    "w": 258.26,
    "h": 26.8,
    "text": "to persist queued steers to the",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 17.87,
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
    "h": 28.34,
    "text": "session ledger.",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 18.89,
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
    "h": 28.24,
    "text": "\u2022 All tests pass: 12 passed .",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 18.83,
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
    "h": 33.0,
    "text": "\u2022 Changes included in commit a6ea8505 .",
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
    "id": "t11",
    "x": 266.0,
    "y": 664.69,
    "w": 122.11,
    "h": 28.47,
    "text": "Sep 28, 9:41 PM",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 18.98,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
