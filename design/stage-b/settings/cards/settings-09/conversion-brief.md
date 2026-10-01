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
    "x": 72.06,
    "y": 40.2,
    "w": 60.77,
    "h": 40.84,
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
    "id": "t_row1",
    "x": 72.03,
    "y": 142.29,
    "w": 60.84,
    "h": 36.05,
    "text": "Sandbox",
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
    "id": "sandbox_write_pill",
    "x": 330.0,
    "y": 222.0,
    "w": 50.0,
    "h": 30.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "sandbox_write_knob",
    "x": 356.0,
    "y": 225.0,
    "w": 24.0,
    "h": 24.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "sandbox_write",
    "x": 330.0,
    "y": 222.0,
    "w": 50.0,
    "h": 30.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "sandbox_write_surface",
    "x": 330.0,
    "y": 222.0,
    "w": 50.0,
    "h": 30.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "sandbox_write_control",
    "x": 330.0,
    "y": 222.0,
    "w": 50.0,
    "h": 30.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "sandbox_write_label",
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
    "id": "t_row1_d",
    "x": 71.98,
    "y": 219.45,
    "w": 98.88,
    "h": 38.02,
    "text": "Workspace write",
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
    "id": "t_row2",
    "x": 72.09,
    "y": 266.75,
    "w": 185.93,
    "h": 32.33,
    "text": "Allow writes inside the workspace",
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
    "id": "sandbox_network_pill",
    "x": 330.0,
    "y": 369.0,
    "w": 50.0,
    "h": 30.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "sandbox_network_knob",
    "x": 333.0,
    "y": 372.0,
    "w": 24.0,
    "h": 24.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "sandbox_network",
    "x": 330.0,
    "y": 369.0,
    "w": 50.0,
    "h": 30.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "sandbox_network_surface",
    "x": 330.0,
    "y": 369.0,
    "w": 50.0,
    "h": 30.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "sandbox_network_control",
    "x": 330.0,
    "y": 369.0,
    "w": 50.0,
    "h": 30.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "sandbox_network_label",
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
    "id": "t_row2_d",
    "x": 72.09,
    "y": 366.44,
    "w": 92.33,
    "h": 29.64,
    "text": "Network access",
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
    "id": "t_row3",
    "x": 73.36,
    "y": 417.64,
    "w": 183.4,
    "h": 29.64,
    "text": "Allow outbound network requests",
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
    "id": "sandbox_read_outside_pill",
    "x": 330.0,
    "y": 517.0,
    "w": 50.0,
    "h": 30.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "sandbox_read_outside_knob",
    "x": 333.0,
    "y": 520.0,
    "w": 24.0,
    "h": 24.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "sandbox_read_outside",
    "x": 330.0,
    "y": 517.0,
    "w": 50.0,
    "h": 30.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "sandbox_read_outside_surface",
    "x": 330.0,
    "y": 517.0,
    "w": 50.0,
    "h": 30.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "sandbox_read_outside_control",
    "x": 330.0,
    "y": 517.0,
    "w": 50.0,
    "h": 30.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "sandbox_read_outside_label",
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
    "id": "t_row3_d",
    "x": 72.09,
    "y": 514.64,
    "w": 142.92,
    "h": 35.03,
    "text": "Read outside workspace",
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
    "id": "div",
    "x": 24.0,
    "y": 674.0,
    "w": 358.0,
    "h": 1.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_foot",
    "x": 73.36,
    "y": 565.83,
    "w": 222.6,
    "h": 35.03,
    "text": "Allow reading files outside the workspace",
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
