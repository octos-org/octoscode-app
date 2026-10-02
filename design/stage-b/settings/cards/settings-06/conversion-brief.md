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
    "id": "back",
    "x": 18.0,
    "y": 44.0,
    "w": 22.0,
    "h": 22.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_title",
    "x": 75.14,
    "y": 43.87,
    "w": 64.59,
    "h": 35.65,
    "text": "General",
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
    "id": "div1",
    "x": 24.0,
    "y": 118.0,
    "w": 358.0,
    "h": 1.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_notif",
    "x": 73.82,
    "y": 150.81,
    "w": 133.14,
    "h": 35.65,
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
    "id": "notifications_toggle_pill",
    "x": 330.0,
    "y": 148.0,
    "w": 50.0,
    "h": 30.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "notifications_toggle_knob",
    "x": 356.0,
    "y": 151.0,
    "w": 24.0,
    "h": 24.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "notifications_toggle",
    "x": 330.0,
    "y": 148.0,
    "w": 50.0,
    "h": 30.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "notifications_toggle_surface",
    "x": 330.0,
    "y": 148.0,
    "w": 50.0,
    "h": 30.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "notifications_toggle_control",
    "x": 330.0,
    "y": 148.0,
    "w": 50.0,
    "h": 30.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "notifications_toggle_label",
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
    "id": "t_help1",
    "x": 73.82,
    "y": 211.14,
    "w": 184.55,
    "h": 32.9,
    "text": "Notify when a turn needs you or",
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
    "id": "t_help2",
    "x": 73.82,
    "y": 252.27,
    "w": 197.73,
    "h": 27.42,
    "text": "finishes while OctosCode is in the",
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
    "id": "t_help3",
    "x": 73.76,
    "y": 292.83,
    "w": 73.94,
    "h": 36.78,
    "text": "background",
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
    "id": "div2",
    "x": 24.0,
    "y": 372.0,
    "w": 358.0,
    "h": 1.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_theme",
    "x": 73.82,
    "y": 400.34,
    "w": 47.45,
    "h": 30.16,
    "text": "Theme",
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
    "id": "t_theme_v",
    "x": 320.32,
    "y": 400.34,
    "w": 61.95,
    "h": 35.65,
    "text": "System \u25be",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 14.5,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "div3",
    "x": 24.0,
    "y": 487.0,
    "w": 358.0,
    "h": 1.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_server",
    "x": 73.82,
    "y": 515.51,
    "w": 80.41,
    "h": 30.16,
    "text": "Octos server",
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
    "id": "t_server_v",
    "x": 205.64,
    "y": 515.51,
    "w": 174.0,
    "h": 32.9,
    "text": "127.0.0.1:50190 \u2022 Connected",
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
    "id": "div4",
    "x": 24.0,
    "y": 604.0,
    "w": 358.0,
    "h": 1.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_stop",
    "x": 73.2,
    "y": 635.3,
    "w": 86.79,
    "h": 39.73,
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
    "id": "stop_row",
    "x": 24.0,
    "y": 620.0,
    "w": 358.0,
    "h": 62.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "stop_row_surface",
    "x": 24.0,
    "y": 620.0,
    "w": 358.0,
    "h": 62.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "stop_row_control",
    "x": 24.0,
    "y": 620.0,
    "w": 358.0,
    "h": 62.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "stop_row_label",
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
    "id": "t_stop_help",
    "x": 73.82,
    "y": 682.77,
    "w": 212.23,
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
  }
]
```
