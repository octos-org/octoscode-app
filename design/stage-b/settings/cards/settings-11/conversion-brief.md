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
    "x": 74.22,
    "y": 45.11,
    "w": 65.17,
    "h": 41.35,
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
    "id": "t_sec",
    "x": 75.54,
    "y": 144.51,
    "w": 47.94,
    "h": 35.88,
    "text": "Model",
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
    "id": "t_model",
    "x": 75.63,
    "y": 244.35,
    "w": 42.46,
    "h": 32.22,
    "text": "Model",
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
    "id": "t_model_v",
    "x": 307.68,
    "y": 243.0,
    "w": 69.26,
    "h": 32.24,
    "text": "v4-flash \u25be",
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
    "id": "div",
    "x": 24.0,
    "y": 316.0,
    "w": 358.0,
    "h": 1.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_think",
    "x": 74.25,
    "y": 354.14,
    "w": 54.49,
    "h": 35.5,
    "text": "Thinking",
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
    "id": "seg_track",
    "x": 170.0,
    "y": 348.0,
    "w": 196.0,
    "h": 44.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "seg_sel",
    "x": 236.0,
    "y": 351.0,
    "w": 62.0,
    "h": 38.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "seg_off",
    "x": 181.8,
    "y": 357.1,
    "w": 23.9,
    "h": 29.5,
    "text": "Off",
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
    "id": "seg_on",
    "x": 258.7,
    "y": 357.1,
    "w": 21.2,
    "h": 32.2,
    "text": "On",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 13.5,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "seg_high",
    "x": 330.4,
    "y": 359.8,
    "w": 29.2,
    "h": 32.2,
    "text": "High",
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
    "id": "think_off",
    "x": 170.0,
    "y": 348.0,
    "w": 62.0,
    "h": 44.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "think_off_surface",
    "x": 170.0,
    "y": 348.0,
    "w": 62.0,
    "h": 44.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "think_off_control",
    "x": 170.0,
    "y": 348.0,
    "w": 62.0,
    "h": 44.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "think_off_label",
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
    "id": "think_on",
    "x": 236.0,
    "y": 348.0,
    "w": 62.0,
    "h": 44.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "think_on_surface",
    "x": 236.0,
    "y": 348.0,
    "w": 62.0,
    "h": 44.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "think_on_control",
    "x": 236.0,
    "y": 348.0,
    "w": 62.0,
    "h": 44.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "think_on_label",
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
    "id": "think_high",
    "x": 298.0,
    "y": 348.0,
    "w": 62.0,
    "h": 44.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "think_high_surface",
    "x": 298.0,
    "y": 348.0,
    "w": 62.0,
    "h": 44.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "think_high_control",
    "x": 298.0,
    "y": 348.0,
    "w": 62.0,
    "h": 44.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "think_high_label",
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
    "id": "t_help",
    "x": 74.3,
    "y": 434.99,
    "w": 248.11,
    "h": 32.22,
    "text": "Shows the model's reasoning while it works",
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
