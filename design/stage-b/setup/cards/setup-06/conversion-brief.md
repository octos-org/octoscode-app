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
    "id": "outer_card",
    "x": 10.0,
    "y": 10.0,
    "w": 386.0,
    "h": 756.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_title",
    "x": 43.82,
    "y": 77.29,
    "w": 92.81,
    "h": 33.0,
    "text": "Settings",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 27,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "group1",
    "x": 28.0,
    "y": 142.0,
    "w": 350.0,
    "h": 196.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_g1_r0",
    "x": 52.99,
    "y": 156.69,
    "w": 91.39,
    "h": 21.0,
    "text": "Connection",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 15,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "live_dot",
    "x": 251.05,
    "y": 162.93,
    "w": 8.0,
    "h": 8.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_g1_v0",
    "x": 267.05,
    "y": 156.04,
    "w": 63.61,
    "h": 21.78,
    "text": "Live",
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
    "id": "chev_g1_r0",
    "x": 352.0,
    "y": 158.93,
    "w": 16.0,
    "h": 16.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_g1_r1",
    "x": 52.54,
    "y": 227.9,
    "w": 89.76,
    "h": 21.88,
    "text": "Workspace",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 15,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_g1_v1",
    "x": 283.07,
    "y": 226.5,
    "w": 74.43,
    "h": 19.5,
    "text": "octos",
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
    "id": "chev_g1_r1",
    "x": 352.0,
    "y": 228.25,
    "w": 16.0,
    "h": 16.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "hair_g1_1",
    "x": 44.0,
    "y": 201.97,
    "w": 318.0,
    "h": 1.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_g1_r2",
    "x": 51.88,
    "y": 298.9,
    "w": 53.01,
    "h": 21.0,
    "text": "Profile",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 15,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_g1_v2",
    "x": 249.12,
    "y": 299.44,
    "w": 82.57,
    "h": 19.5,
    "text": "octos-dev",
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
    "id": "chev_g1_r2",
    "x": 352.0,
    "y": 301.05,
    "w": 16.0,
    "h": 16.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "hair_g1_2",
    "x": 44.0,
    "y": 274.34,
    "w": 318.0,
    "h": 1.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "group2",
    "x": 28.0,
    "y": 370.0,
    "w": 350.0,
    "h": 76.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_notif",
    "x": 54.13,
    "y": 404.92,
    "w": 164.66,
    "h": 22.58,
    "text": "Desktop notifications",
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
    "id": "toggle1",
    "x": 312.0,
    "y": 398.92,
    "w": 50.0,
    "h": 30.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "toggle1_knob",
    "x": 338.0,
    "y": 401.92,
    "w": 24.0,
    "h": 24.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "group3",
    "x": 28.0,
    "y": 462.0,
    "w": 350.0,
    "h": 76.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_diag",
    "x": 51.78,
    "y": 487.62,
    "w": 134.39,
    "h": 24.77,
    "text": "Copy diagnostics",
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
    "id": "copy_diag",
    "x": 338.0,
    "y": 485.0,
    "w": 30.0,
    "h": 30.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "disconnect",
    "x": 28.0,
    "y": 612.94,
    "w": 350.0,
    "h": 48.66,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_disconnect",
    "x": 152.21,
    "y": 626.94,
    "w": 93.68,
    "h": 21.0,
    "text": "Disconnect",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 15,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
