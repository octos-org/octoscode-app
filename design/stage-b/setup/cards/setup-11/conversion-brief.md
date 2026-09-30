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
    "id": "icon_warning",
    "x": 130.0,
    "y": 80.0,
    "w": 72.0,
    "h": 77.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_title",
    "x": 29.32,
    "y": 188.36,
    "w": 289.84,
    "h": 33.0,
    "text": "Something went wrong",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 22,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_msg1",
    "x": 40.6,
    "y": 250.4,
    "w": 261.64,
    "h": 24.81,
    "text": "OctosCode hit an unexpected",
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
    "id": "t_msg2",
    "x": 50.55,
    "y": 280.9,
    "w": 241.69,
    "h": 25.3,
    "text": "error. Your sessions are safe",
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
    "id": "t_msg3",
    "x": 109.32,
    "y": 319.18,
    "w": 121.03,
    "h": 21.0,
    "text": "on the server.",
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
    "id": "btn_reload",
    "x": 26.0,
    "y": 387.0,
    "w": 303.0,
    "h": 65.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "btn_reload_surface",
    "x": 26.0,
    "y": 387.0,
    "w": 303.0,
    "h": 65.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "btn_reload_control",
    "x": 26.0,
    "y": 387.0,
    "w": 303.0,
    "h": 65.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "btn_reload_label",
    "x": 141.0,
    "y": 410.0,
    "w": 68.0,
    "h": 25.01,
    "text": "Reload",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 16.67,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "btn_diag",
    "x": 26.0,
    "y": 473.0,
    "w": 303.0,
    "h": 63.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "btn_diag_surface",
    "x": 26.0,
    "y": 473.0,
    "w": 303.0,
    "h": 63.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "btn_diag_control",
    "x": 26.0,
    "y": 473.0,
    "w": 303.0,
    "h": 63.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "btn_diag_label",
    "x": 87.0,
    "y": 491.0,
    "w": 164.0,
    "h": 33.0,
    "text": "Copy diagnostics",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 22.0,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_report",
    "x": 120.67,
    "y": 571.85,
    "w": 109.39,
    "h": 24.81,
    "text": "Report issue",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 14,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
