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
    "id": "prov_back_chev",
    "x": 16.0,
    "y": 82.0,
    "w": 14.0,
    "h": 22.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "prov_back_control",
    "x": 6.0,
    "y": 73.0,
    "w": 34.0,
    "h": 40.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "t_title",
    "x": 128.42,
    "y": 99.47,
    "w": 144.66,
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
    "x": 21.43,
    "y": 128.58,
    "w": 38.34,
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
    "x": 21.34,
    "y": 192.43,
    "w": 63.32,
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
    "y": 254.91,
    "w": 50.75,
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
    "y": 372.0,
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
    "id": "cal_red",
    "x": 28.0,
    "y": 307.45,
    "w": 350.0,
    "h": 65.22,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_cal1",
    "x": 23.68,
    "y": 321.45,
    "w": 243.6,
    "h": 21.0,
    "text": "The provider rejected this key (401).",
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
    "id": "t_cal2",
    "x": 24.81,
    "y": 342.88,
    "w": 119.54,
    "h": 19.5,
    "text": "Your draft is kept.",
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
    "id": "t_models_label",
    "x": 21.35,
    "y": 367.43,
    "w": 49.78,
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
    "y": 399.5,
    "w": 18.0,
    "h": 18.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_model_0",
    "x": 33.83,
    "y": 401.5,
    "w": 193.98,
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
    "y": 394.0,
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
    "y": 399.5,
    "w": 18.0,
    "h": 18.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_model_1",
    "x": 33.83,
    "y": 401.5,
    "w": 193.98,
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
    "y": 430.0,
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
    "y": 470.59,
    "w": 18.0,
    "h": 18.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_model_2",
    "x": 33.83,
    "y": 472.59,
    "w": 103.76,
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
    "y": 466.0,
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
    "x": 40.0,
    "y": 518.0,
    "w": 120.0,
    "h": 40.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "prov_cancel_surface",
    "x": 40.0,
    "y": 518.0,
    "w": 120.0,
    "h": 40.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "prov_cancel_control",
    "x": 40.0,
    "y": 518.0,
    "w": 120.0,
    "h": 40.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "prov_cancel_label",
    "x": 71.0,
    "y": 528.0,
    "w": 60.0,
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
    "id": "prov_retry",
    "x": 216.0,
    "y": 518.0,
    "w": 150.0,
    "h": 40.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "prov_retry_surface",
    "x": 216.0,
    "y": 518.0,
    "w": 150.0,
    "h": 40.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "prov_retry_control",
    "x": 216.0,
    "y": 518.0,
    "w": 150.0,
    "h": 40.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "prov_retry_label",
    "x": 246.0,
    "y": 528.0,
    "w": 64.0,
    "h": 22.5,
    "text": "Try again",
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
