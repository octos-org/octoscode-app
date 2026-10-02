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
    "id": "rail_chip",
    "x": 11.0,
    "y": 121.0,
    "w": 42.0,
    "h": 50.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "rail_gear_i",
    "x": 16.0,
    "y": 130.0,
    "w": 24.0,
    "h": 34.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "rail_shield_i",
    "x": 16.0,
    "y": 218.0,
    "w": 24.0,
    "h": 48.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "rail_code_i",
    "x": 16.0,
    "y": 318.0,
    "w": 24.0,
    "h": 34.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "rail_box_i",
    "x": 16.0,
    "y": 404.0,
    "w": 24.0,
    "h": 54.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "rail_plug_i",
    "x": 16.0,
    "y": 500.0,
    "w": 24.0,
    "h": 50.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "rail_info_i",
    "x": 16.0,
    "y": 592.0,
    "w": 24.0,
    "h": 48.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "dim",
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
    "id": "b_title",
    "x": 74.08,
    "y": 43.22,
    "w": 62.65,
    "h": 42.43,
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
    "id": "b_sec",
    "x": 72.85,
    "y": 147.89,
    "w": 49.5,
    "h": 30.52,
    "text": "General",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 15,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "b_stop",
    "x": 73.2,
    "y": 635.3,
    "w": 84.87,
    "h": 46.88,
    "text": "Stop server\u2026",
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
    "id": "b_stop_help",
    "x": 74.17,
    "y": 685.51,
    "w": 208.21,
    "h": 32.9,
    "text": "Shuts down Octos on this computer",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 12.5,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "modal",
    "x": 28.0,
    "y": 176.0,
    "w": 350.0,
    "h": 372.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "m_title",
    "x": 110.61,
    "y": 227.59,
    "w": 187.38,
    "h": 46.61,
    "text": "Stop the Octos server?",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 16.5,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "m_body1",
    "x": 97.37,
    "y": 313.43,
    "w": 222.99,
    "h": 42.87,
    "text": "All sessions on this computer stop.",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 13.5,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "m_body2",
    "x": 101.5,
    "y": 367.43,
    "w": 199.1,
    "h": 35.65,
    "text": "Running turns are interrupted.",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 13.5,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "cancel_btn_fill",
    "x": 58.0,
    "y": 480.0,
    "w": 128.0,
    "h": 46.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "cancel_label",
    "x": 121.0,
    "y": 490.4,
    "w": 47.0,
    "h": 33.7,
    "text": "Cancel",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 14.5,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "cancel_btn",
    "x": 58.0,
    "y": 480.0,
    "w": 128.0,
    "h": 46.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "cancel_btn_surface",
    "x": 58.0,
    "y": 480.0,
    "w": 128.0,
    "h": 46.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "cancel_btn_control",
    "x": 58.0,
    "y": 480.0,
    "w": 128.0,
    "h": 46.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "cancel_btn_label",
    "x": 0.0,
    "y": 0.0,
    "w": 8.0,
    "h": 15.0,
    "text": "",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 10.0,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "stop_btn_fill",
    "x": 220.0,
    "y": 480.0,
    "w": 128.0,
    "h": 46.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "stop_btn_label",
    "x": 237.3,
    "y": 489.9,
    "w": 78.0,
    "h": 45.9,
    "text": "Stop server",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 14.5,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "stop_confirm",
    "x": 220.0,
    "y": 480.0,
    "w": 128.0,
    "h": 46.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "stop_confirm_surface",
    "x": 220.0,
    "y": 480.0,
    "w": 128.0,
    "h": 46.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "stop_confirm_control",
    "x": 220.0,
    "y": 480.0,
    "w": 128.0,
    "h": 46.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "stop_confirm_label",
    "x": 0.0,
    "y": 0.0,
    "w": 8.0,
    "h": 15.0,
    "text": "",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 10.0,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
