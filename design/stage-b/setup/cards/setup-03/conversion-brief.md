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
    "x": 57.52,
    "y": 94.5,
    "w": 241.34,
    "h": 33.0,
    "text": "Set up a local profile",
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
    "id": "t_pname",
    "x": 31.52,
    "y": 156.45,
    "w": 98.24,
    "h": 21.0,
    "text": "Profile name",
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
    "id": "profile",
    "x": 46.0,
    "y": 193.16,
    "w": 314.0,
    "h": 44.55,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "profile_field",
    "x": 60.0,
    "y": 205.16,
    "w": 286.0,
    "h": 22.5,
    "text": "octos-dev",
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
    "id": "t_provider",
    "x": 31.4,
    "y": 280.19,
    "w": 66.89,
    "h": 21.0,
    "text": "Provider",
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
    "id": "provider_deepseek",
    "x": 46.0,
    "y": 320.48,
    "w": 314.0,
    "h": 41.52,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "radio_deepseek",
    "x": 60.0,
    "y": 331.24,
    "w": 20.0,
    "h": 20.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_prov_deepseek",
    "x": 72.18,
    "y": 330.48,
    "w": 184.96,
    "h": 21.52,
    "text": "DeepSeek \u2022 Official API",
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
    "id": "provider_kimi",
    "x": 46.0,
    "y": 371.49,
    "w": 314.0,
    "h": 44.29,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "radio_kimi",
    "x": 60.0,
    "y": 383.64,
    "w": 20.0,
    "h": 20.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_prov_kimi",
    "x": 72.04,
    "y": 381.49,
    "w": 134.49,
    "h": 24.29,
    "text": "Kimi Coding Plan",
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
    "id": "provider_glm",
    "x": 46.0,
    "y": 425.9,
    "w": 314.0,
    "h": 43.75,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "radio_glm",
    "x": 60.0,
    "y": 437.78,
    "w": 20.0,
    "h": 20.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_prov_glm",
    "x": 72.08,
    "y": 435.9,
    "w": 137.78,
    "h": 23.75,
    "text": "GLM Coding Plan",
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
    "id": "t_apikey",
    "x": 30.45,
    "y": 498.5,
    "w": 62.03,
    "h": 22.59,
    "text": "API key",
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
    "id": "apikey",
    "x": 46.0,
    "y": 541.8,
    "w": 314.0,
    "h": 33.02,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "apikey_field",
    "x": 60.0,
    "y": 553.8,
    "w": 286.0,
    "h": 22.5,
    "text": "",
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
    "id": "apikey_eye",
    "x": 328.0,
    "y": 548.31,
    "w": 20.0,
    "h": 20.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_keyhint",
    "x": 31.58,
    "y": 603.43,
    "w": 252.62,
    "h": 21.57,
    "text": "Your key never leaves this machine",
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
    "id": "create_profile",
    "x": 104.18,
    "y": 680.28,
    "w": 160.42,
    "h": 45.94,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "create_profile_surface",
    "x": 104.18,
    "y": 680.28,
    "w": 160.42,
    "h": 45.94,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "create_profile_control",
    "x": 104.18,
    "y": 680.28,
    "w": 160.42,
    "h": 45.94,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "create_profile_label",
    "x": 125.18,
    "y": 690.28,
    "w": 118.42,
    "h": 25.94,
    "text": "Create profile",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 17.29,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
