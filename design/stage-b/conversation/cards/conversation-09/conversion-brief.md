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
    "id": "answer_md",
    "x": 27.0,
    "y": 106.0,
    "w": 356.0,
    "h": 492.0,
    "text": "Queued steers now survive a reconnect.\n\n\u2022 Fixed loss of queued steers when reconnecting after a drop in steer_dropped handling.\n\n\u2022 Updated ui_protocol_transport.rs to persist queued steers to the session ledger.\n\n\u2022 All tests pass: 12 passed.\n\n\u2022 Changes included in commit a6ea8505.",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 17.5,
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
    "y": 662.0,
    "w": 20.0,
    "h": 20.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "icon_thumbs",
    "x": 52.0,
    "y": 662.0,
    "w": 20.0,
    "h": 20.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "icon_share",
    "x": 80.0,
    "y": 662.0,
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
