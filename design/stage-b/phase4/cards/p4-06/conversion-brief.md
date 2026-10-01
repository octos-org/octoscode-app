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
    "x": 117.26,
    "y": 154.37,
    "w": 155.69,
    "h": 30.0,
    "text": "Edit provider",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 20,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_name_label",
    "x": 20.3,
    "y": 194.0,
    "w": 42.86,
    "h": 19.5,
    "text": "Name",
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
    "id": "prov_name",
    "x": 28.0,
    "y": 218.0,
    "w": 350.0,
    "h": 44.0,
    "text": "DeepSeek \u2022 Official API",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15,
    "weight": 400,
    "role": "input",
    "native_candidates": [
      "TextInput",
      "KitFormField"
    ]
  },
  {
    "id": "t_url_label",
    "x": 20.17,
    "y": 271.24,
    "w": 70.18,
    "h": 19.5,
    "text": "Base URL",
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
    "id": "prov_url",
    "x": 28.0,
    "y": 296.0,
    "w": 350.0,
    "h": 44.0,
    "text": "https://api.deepseek.com/v1",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15,
    "weight": 400,
    "role": "input",
    "native_candidates": [
      "TextInput",
      "KitFormField"
    ]
  },
  {
    "id": "t_key_label",
    "x": 20.3,
    "y": 348.5,
    "w": 54.13,
    "h": 19.5,
    "text": "API key",
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
    "id": "prov_key",
    "x": 28.0,
    "y": 374.0,
    "w": 350.0,
    "h": 44.0,
    "text": "\u2022\u2022\u2022\u2022\u2022\u2022\u2022\u2022\u2022\u2022",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15,
    "weight": 400,
    "role": "input",
    "native_candidates": [
      "TextInput",
      "KitFormField"
    ]
  },
  {
    "id": "key_eye",
    "x": 350.0,
    "y": 386.0,
    "w": 20.0,
    "h": 20.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "key_eye_control",
    "x": 342.0,
    "y": 378.0,
    "w": 36.0,
    "h": 36.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "t_models_label",
    "x": 19.15,
    "y": 431.93,
    "w": 51.92,
    "h": 22.5,
    "text": "Models",
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
    "id": "chk_0",
    "x": 40.0,
    "y": 467.0,
    "w": 18.0,
    "h": 18.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_model_0",
    "x": 34.96,
    "y": 469.0,
    "w": 198.49,
    "h": 21.0,
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
    "id": "model_0_control",
    "x": 28.0,
    "y": 462.0,
    "w": 350.0,
    "h": 34.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "chk_1",
    "x": 40.0,
    "y": 467.0,
    "w": 18.0,
    "h": 18.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_model_1",
    "x": 34.96,
    "y": 469.0,
    "w": 198.49,
    "h": 21.0,
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
    "id": "model_1_control",
    "x": 28.0,
    "y": 503.0,
    "w": 350.0,
    "h": 34.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "chk_2",
    "x": 40.0,
    "y": 549.93,
    "w": 18.0,
    "h": 18.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_model_2",
    "x": 34.83,
    "y": 551.93,
    "w": 107.4,
    "h": 21.0,
    "text": "deepseek-chat",
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
    "id": "model_2_control",
    "x": 28.0,
    "y": 545.0,
    "w": 350.0,
    "h": 34.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "prov_cancel",
    "x": 60.0,
    "y": 606.0,
    "w": 120.0,
    "h": 44.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "prov_cancel_surface",
    "x": 60.0,
    "y": 606.0,
    "w": 120.0,
    "h": 44.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "prov_cancel_control",
    "x": 60.0,
    "y": 606.0,
    "w": 120.0,
    "h": 44.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "prov_cancel_label",
    "x": 74.0,
    "y": 618.0,
    "w": 57.0,
    "h": 22.5,
    "text": "Cancel",
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
    "id": "prov_save",
    "x": 226.0,
    "y": 606.0,
    "w": 120.0,
    "h": 44.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "prov_save_surface",
    "x": 226.0,
    "y": 606.0,
    "w": 120.0,
    "h": 44.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "prov_save_control",
    "x": 226.0,
    "y": 606.0,
    "w": 120.0,
    "h": 44.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "prov_save_label",
    "x": 272.0,
    "y": 618.0,
    "w": 42.0,
    "h": 22.5,
    "text": "Save",
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
