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
    "id": "fleet_screen",
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
    "x": 18.8,
    "y": 25.43,
    "w": 101.16,
    "h": 28.5,
    "text": "OctosCode",
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
    "id": "t_title",
    "x": 21.96,
    "y": 95.99,
    "w": 149.82,
    "h": 31.48,
    "text": "Fleet \u00b7 3 peers",
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
    "id": "fleet_goal",
    "x": 16.0,
    "y": 164.5,
    "w": 374.0,
    "h": 49.17,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "fleet_goal_icon",
    "x": 23.32,
    "y": 177.5,
    "w": 18.0,
    "h": 18.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "fleet_goal_label",
    "x": 49.32,
    "y": 176.5,
    "w": 112.06,
    "h": 25.17,
    "text": "Fix steer queue",
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
    "id": "peer_1",
    "x": 16.0,
    "y": 256.27,
    "w": 374.0,
    "h": 81.84,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "peer_1_status",
    "x": 40.96,
    "y": 272.27,
    "w": 58.54,
    "h": 24.64,
    "text": "Running",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 14,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "peer_1_name",
    "x": 143.69,
    "y": 255.71,
    "w": 46.68,
    "h": 23.0,
    "text": "tests",
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
    "id": "peer_1_meta",
    "x": 143.9,
    "y": 301.98,
    "w": 116.49,
    "h": 20.13,
    "text": "18m \u00b7 41k tokens",
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
    "id": "peer_1_steer",
    "x": 327.2,
    "y": 274.53,
    "w": 46.25,
    "h": 23.79,
    "text": "Steer",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 14,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "peer_2",
    "x": 16.0,
    "y": 382.98,
    "w": 374.0,
    "h": 82.06,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "peer_2_status",
    "x": 41.11,
    "y": 398.98,
    "w": 58.24,
    "h": 23.79,
    "text": "Blocked",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 14,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "peer_2_name",
    "x": 145.61,
    "y": 382.51,
    "w": 42.83,
    "h": 23.79,
    "text": "docs",
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
    "id": "peer_2_meta",
    "x": 143.8,
    "y": 427.62,
    "w": 108.13,
    "h": 21.41,
    "text": "7m \u00b7 12k tokens",
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
    "id": "peer_2_steer",
    "x": 327.06,
    "y": 400.52,
    "w": 46.53,
    "h": 26.2,
    "text": "Steer",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 14,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "peer_3",
    "x": 16.0,
    "y": 510.53,
    "w": 374.0,
    "h": 81.98,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "peer_3_status",
    "x": 40.84,
    "y": 526.53,
    "w": 39.94,
    "h": 23.09,
    "text": "Done",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 14,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "peer_3_name",
    "x": 145.56,
    "y": 510.48,
    "w": 54.93,
    "h": 24.08,
    "text": "review",
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
    "id": "peer_3_meta",
    "x": 143.9,
    "y": 556.38,
    "w": 128.48,
    "h": 20.13,
    "text": "24m \u00b7 28k tokens",
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
    "id": "peer_3_steer",
    "x": 327.2,
    "y": 528.92,
    "w": 44.54,
    "h": 21.96,
    "text": "Steer",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 14,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
