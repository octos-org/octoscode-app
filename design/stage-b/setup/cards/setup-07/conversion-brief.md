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
    "id": "card_deepseek",
    "x": 20.0,
    "y": 110.0,
    "w": 324.0,
    "h": 290.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "card_kimi",
    "x": 20.0,
    "y": 441.0,
    "w": 324.0,
    "h": 99.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "card_glm",
    "x": 20.0,
    "y": 564.0,
    "w": 324.0,
    "h": 98.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "vline",
    "x": 364.0,
    "y": 110.0,
    "w": 1.0,
    "h": 590.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_title",
    "x": 25.9,
    "y": 61.92,
    "w": 77.88,
    "h": 28.5,
    "text": "Models",
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
    "id": "t_ds_head",
    "x": 36.09,
    "y": 138.5,
    "w": 203.0,
    "h": 25.05,
    "text": "DeepSeek \u2022 Official API",
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
    "id": "t_ds_count",
    "x": 34.87,
    "y": 174.47,
    "w": 72.35,
    "h": 19.5,
    "text": "3 models",
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
    "id": "inner_card",
    "x": 34.5,
    "y": 214.0,
    "w": 296.5,
    "h": 123.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "inner_div",
    "x": 34.5,
    "y": 276.5,
    "w": 296.5,
    "h": 1.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "dot_ds",
    "x": 280.5,
    "y": 142.0,
    "w": 9.0,
    "h": 10.5,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "icon_chev_ds",
    "x": 313.5,
    "y": 143.0,
    "w": 12.5,
    "h": 8.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_flash",
    "x": 47.37,
    "y": 236.86,
    "w": 200.74,
    "h": 21.64,
    "text": "deepseek-v4-flash (default)",
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
    "id": "t_pro",
    "x": 46.14,
    "y": 298.24,
    "w": 126.51,
    "h": 21.61,
    "text": "deepseek-v4-pro",
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
    "id": "icon_check",
    "x": 292.0,
    "y": 236.0,
    "w": 28.0,
    "h": 26.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "btn_test",
    "x": 34.5,
    "y": 356.0,
    "w": 140.0,
    "h": 40.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "btn_test_surface",
    "x": 34.5,
    "y": 356.0,
    "w": 140.0,
    "h": 40.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "btn_test_control",
    "x": 34.5,
    "y": 356.0,
    "w": 140.0,
    "h": 40.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "btn_test_label",
    "x": 65.0,
    "y": 369.0,
    "w": 76.0,
    "h": 20.0,
    "text": "Test route",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 13.33,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "btn_discover",
    "x": 192.0,
    "y": 356.0,
    "w": 138.5,
    "h": 40.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "btn_discover_surface",
    "x": 192.0,
    "y": 356.0,
    "w": 138.5,
    "h": 40.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "btn_discover_control",
    "x": 192.0,
    "y": 356.0,
    "w": 138.5,
    "h": 40.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "btn_discover_label",
    "x": 208.0,
    "y": 369.0,
    "w": 112.0,
    "h": 20.0,
    "text": "Discover models",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 13.33,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_kimi_head",
    "x": 34.87,
    "y": 465.31,
    "w": 142.28,
    "h": 25.84,
    "text": "Kimi Coding Plan",
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
    "id": "t_kimi_count",
    "x": 34.84,
    "y": 500.28,
    "w": 69.04,
    "h": 19.5,
    "text": "3 models",
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
    "id": "dot_kimi",
    "x": 280.5,
    "y": 476.5,
    "w": 9.0,
    "h": 10.5,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "icon_chev_kimi",
    "x": 314.0,
    "y": 484.5,
    "w": 11.5,
    "h": 7.5,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_glm_head",
    "x": 34.85,
    "y": 588.09,
    "w": 144.57,
    "h": 25.03,
    "text": "GLM Coding Plan",
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
    "id": "t_glm_count",
    "x": 34.76,
    "y": 624.0,
    "w": 68.08,
    "h": 19.5,
    "text": "3 models",
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
    "id": "dot_glm",
    "x": 280.5,
    "y": 601.5,
    "w": 9.0,
    "h": 10.5,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "icon_chev_glm",
    "x": 314.0,
    "y": 608.5,
    "w": 11.5,
    "h": 7.5,
    "role": "unknown",
    "native_candidates": []
  }
]
```
