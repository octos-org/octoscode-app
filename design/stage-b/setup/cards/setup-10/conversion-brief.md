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
    "id": "t_title",
    "x": 39.47,
    "y": 36.09,
    "w": 63.16,
    "h": 28.5,
    "text": "Skills",
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
    "id": "search_box",
    "x": 24.0,
    "y": 86.0,
    "w": 358.0,
    "h": 50.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "icon_search",
    "x": 42.0,
    "y": 100.0,
    "w": 20.0,
    "h": 20.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_search",
    "x": 83.41,
    "y": 101.23,
    "w": 127.54,
    "h": 23.12,
    "text": "Search registry",
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
    "id": "t_inst_head",
    "x": 32.71,
    "y": 170.31,
    "w": 72.18,
    "h": 22.5,
    "text": "Installed",
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
    "id": "t_name3",
    "x": 49.59,
    "y": 226.55,
    "w": 100.44,
    "h": 21.0,
    "text": "rust-review",
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
    "id": "t_ver6",
    "x": 192.85,
    "y": 224.45,
    "w": 45.11,
    "h": 24.81,
    "text": "1.2.0",
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
    "id": "btn_0_remove",
    "x": 287.0,
    "y": 219.5,
    "w": 84.0,
    "h": 34.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "btn_0_remove_surface",
    "x": 287.0,
    "y": 219.5,
    "w": 84.0,
    "h": 34.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "btn_0_remove_control",
    "x": 287.0,
    "y": 219.5,
    "w": 84.0,
    "h": 34.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "btn_0_remove_label",
    "x": 295.38,
    "y": 227.5,
    "w": 58.84,
    "h": 17.59,
    "text": "Remove",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 11.73,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_name4",
    "x": 49.62,
    "y": 289.87,
    "w": 86.84,
    "h": 23.69,
    "text": "git-helper",
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
    "id": "t_ver7",
    "x": 193.98,
    "y": 287.62,
    "w": 43.98,
    "h": 25.94,
    "text": "0.9.1",
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
    "id": "btn_1_remove",
    "x": 287.0,
    "y": 283.0,
    "w": 84.0,
    "h": 34.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "btn_1_remove_surface",
    "x": 287.0,
    "y": 283.0,
    "w": 84.0,
    "h": 34.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "btn_1_remove_control",
    "x": 287.0,
    "y": 283.0,
    "w": 84.0,
    "h": 34.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "btn_1_remove_label",
    "x": 295.48,
    "y": 291.0,
    "w": 59.77,
    "h": 18.05,
    "text": "Remove",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 12.03,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_name5",
    "x": 50.75,
    "y": 354.16,
    "w": 100.37,
    "h": 21.0,
    "text": "docs-writer",
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
    "id": "t_ver8",
    "x": 193.98,
    "y": 351.91,
    "w": 43.98,
    "h": 25.94,
    "text": "2.0.0",
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
    "id": "btn_2_remove",
    "x": 287.0,
    "y": 347.09,
    "w": 84.0,
    "h": 34.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "btn_2_remove_surface",
    "x": 287.0,
    "y": 347.09,
    "w": 84.0,
    "h": 34.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "btn_2_remove_control",
    "x": 287.0,
    "y": 347.09,
    "w": 84.0,
    "h": 34.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "btn_2_remove_label",
    "x": 295.42,
    "y": 355.09,
    "w": 58.77,
    "h": 18.46,
    "text": "Remove",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 12.3,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_reg_head",
    "x": 33.83,
    "y": 427.48,
    "w": 71.05,
    "h": 23.69,
    "text": "Registry",
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
    "id": "t_name10",
    "x": 49.6,
    "y": 488.26,
    "w": 93.65,
    "h": 21.0,
    "text": "code-linter",
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
    "id": "t_ver11",
    "x": 190.59,
    "y": 485.0,
    "w": 43.98,
    "h": 25.94,
    "text": "1.1.0",
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
    "id": "btn_3_install",
    "x": 280.0,
    "y": 481.51,
    "w": 92.0,
    "h": 34.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "btn_3_install_surface",
    "x": 280.0,
    "y": 481.51,
    "w": 92.0,
    "h": 34.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "btn_3_install_control",
    "x": 280.0,
    "y": 481.51,
    "w": 92.0,
    "h": 34.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "btn_3_install_label",
    "x": 286.46,
    "y": 489.51,
    "w": 53.01,
    "h": 19.17,
    "text": "Install",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 12.78,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_name12",
    "x": 49.33,
    "y": 556.03,
    "w": 81.78,
    "h": 23.75,
    "text": "api-client",
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
    "id": "t_ver13",
    "x": 189.47,
    "y": 556.06,
    "w": 46.24,
    "h": 23.69,
    "text": "0.4.2",
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
    "id": "btn_4_install",
    "x": 280.0,
    "y": 551.44,
    "w": 92.0,
    "h": 34.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "btn_4_install_surface",
    "x": 280.0,
    "y": 551.44,
    "w": 92.0,
    "h": 34.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "btn_4_install_control",
    "x": 280.0,
    "y": 551.44,
    "w": 92.0,
    "h": 34.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "btn_4_install_label",
    "x": 288.71,
    "y": 559.44,
    "w": 49.62,
    "h": 18.05,
    "text": "Install",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 12.03,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
