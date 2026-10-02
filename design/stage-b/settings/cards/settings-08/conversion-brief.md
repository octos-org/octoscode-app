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
    "x": 72.74,
    "y": 43.63,
    "w": 62.63,
    "h": 41.61,
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
    "id": "perm_ask_ring",
    "x": 46.0,
    "y": 228.0,
    "w": 22.0,
    "h": 22.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "perm_ask_dot",
    "x": 53.0,
    "y": 235.0,
    "w": 8.0,
    "h": 8.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "perm_ask",
    "x": 46.0,
    "y": 228.0,
    "w": 22.0,
    "h": 22.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "perm_ask_surface",
    "x": 46.0,
    "y": 228.0,
    "w": 22.0,
    "h": 22.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "perm_ask_control",
    "x": 46.0,
    "y": 228.0,
    "w": 22.0,
    "h": 22.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "perm_ask_label",
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
    "id": "t_opt1",
    "x": 72.8,
    "y": 222.1,
    "w": 123.84,
    "h": 38.39,
    "text": "Ask for approval",
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
    "id": "t_opt1_d",
    "x": 98.31,
    "y": 271.46,
    "w": 227.26,
    "h": 32.9,
    "text": "Server asks before shell, write and network",
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
    "id": "perm_workspace_ring",
    "x": 46.0,
    "y": 348.0,
    "w": 22.0,
    "h": 22.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "perm_workspace",
    "x": 46.0,
    "y": 348.0,
    "w": 22.0,
    "h": 22.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "perm_workspace_surface",
    "x": 46.0,
    "y": 348.0,
    "w": 22.0,
    "h": 22.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "perm_workspace_control",
    "x": 46.0,
    "y": 348.0,
    "w": 22.0,
    "h": 22.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "perm_workspace_label",
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
    "id": "t_opt2",
    "x": 72.8,
    "y": 342.8,
    "w": 196.62,
    "h": 38.39,
    "text": "Auto-approve in workspace",
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
    "id": "t_opt2_d",
    "x": 99.58,
    "y": 397.6,
    "w": 181.3,
    "h": 30.16,
    "text": "Approve actions in this workspace",
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
    "id": "perm_full_ring",
    "x": 46.0,
    "y": 470.0,
    "w": 22.0,
    "h": 22.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "perm_full",
    "x": 46.0,
    "y": 470.0,
    "w": 22.0,
    "h": 22.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "perm_full_surface",
    "x": 46.0,
    "y": 470.0,
    "w": 22.0,
    "h": 22.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "perm_full_control",
    "x": 46.0,
    "y": 470.0,
    "w": 22.0,
    "h": 22.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "perm_full_label",
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
    "id": "t_opt3",
    "x": 95.75,
    "y": 466.15,
    "w": 70.22,
    "h": 30.16,
    "text": "Full access",
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
    "id": "t_opt3_d",
    "x": 99.58,
    "y": 515.51,
    "w": 197.89,
    "h": 30.16,
    "text": "No prompts \u2022 full read/write/network",
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
    "y": 596.0,
    "w": 358.0,
    "h": 1.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_readback",
    "x": 72.77,
    "y": 625.19,
    "w": 237.47,
    "h": 32.9,
    "text": "Server: ask before shell, write and network",
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
    "id": "t_advanced",
    "x": 72.7,
    "y": 687.6,
    "w": 71.6,
    "h": 28.63,
    "text": "Advanced\u2026",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 13.5,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "advanced_link",
    "x": 64.0,
    "y": 680.0,
    "w": 120.0,
    "h": 40.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "advanced_link_surface",
    "x": 64.0,
    "y": 680.0,
    "w": 120.0,
    "h": 40.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "advanced_link_control",
    "x": 64.0,
    "y": 680.0,
    "w": 120.0,
    "h": 40.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "advanced_link_label",
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
