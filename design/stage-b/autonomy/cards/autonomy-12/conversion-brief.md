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
    "id": "t01",
    "x": 11.84,
    "y": 191.22,
    "w": 73.97,
    "h": 28.5,
    "text": "Settings",
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
    "id": "settings_card",
    "x": 20.0,
    "y": 250.0,
    "w": 350.0,
    "h": 305.48,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_conn",
    "x": 20.46,
    "y": 271.73,
    "w": 89.28,
    "h": 23.25,
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
    "id": "t_live",
    "x": 306.45,
    "y": 268.3,
    "w": 91.17,
    "h": 28.5,
    "text": "\u25cf Live \u203a",
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
    "id": "t_ws",
    "x": 20.5,
    "y": 344.38,
    "w": 87.47,
    "h": 27.67,
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
    "id": "t_wsval",
    "x": 325.49,
    "y": 347.75,
    "w": 41.11,
    "h": 21.0,
    "text": "octos",
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
    "id": "chev_ws",
    "x": 356.0,
    "y": 346.38,
    "w": 16.0,
    "h": 16.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_prof",
    "x": 20.56,
    "y": 418.59,
    "w": 49.68,
    "h": 22.5,
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
    "id": "t_profval",
    "x": 291.12,
    "y": 418.27,
    "w": 75.58,
    "h": 21.58,
    "text": "octos-dev",
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
    "id": "chev_prof",
    "x": 356.0,
    "y": 420.59,
    "w": 16.0,
    "h": 16.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_notif",
    "x": 20.52,
    "y": 505.33,
    "w": 152.54,
    "h": 26.15,
    "text": "Desktop notifications",
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
    "id": "toggle_notif",
    "x": 320.0,
    "y": 502.33,
    "w": 50.0,
    "h": 30.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "toggle_notif_knob",
    "x": 344.0,
    "y": 505.33,
    "w": 24.0,
    "h": 24.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "div_1",
    "x": 24.0,
    "y": 330.0,
    "w": 342.0,
    "h": 1.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "div_2",
    "x": 24.0,
    "y": 404.0,
    "w": 342.0,
    "h": 1.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "div_3",
    "x": 24.0,
    "y": 478.0,
    "w": 342.0,
    "h": 1.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "copy_row",
    "x": 20.0,
    "y": 565.74,
    "w": 350.0,
    "h": 48.62,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_diag",
    "x": 20.51,
    "y": 577.74,
    "w": 123.45,
    "h": 24.62,
    "text": "Copy diagnostics",
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
    "id": "icon_copy",
    "x": 344.0,
    "y": 579.74,
    "w": 20.0,
    "h": 20.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "disconnect_card",
    "x": 20.0,
    "y": 652.28,
    "w": 350.0,
    "h": 49.42,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "icon_power",
    "x": 20.49,
    "y": 666.28,
    "w": 20.0,
    "h": 20.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_disconnect",
    "x": 50.49,
    "y": 666.28,
    "w": 89.21,
    "h": 22.5,
    "text": "Disconnect",
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
