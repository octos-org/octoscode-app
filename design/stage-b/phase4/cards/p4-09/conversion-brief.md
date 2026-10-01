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
    "x": 60.9,
    "y": 109.41,
    "w": 268.41,
    "h": 30.0,
    "text": "Choose workspace folder",
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
    "id": "t_crumbs",
    "x": 24.6,
    "y": 159.0,
    "w": 100.79,
    "h": 21.5,
    "text": "/ \u203a private",
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
    "id": "cal_refused",
    "x": 28.0,
    "y": 208.38,
    "w": 350.0,
    "h": 123.62,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_cal1",
    "x": 92.37,
    "y": 224.38,
    "w": 239.38,
    "h": 21.57,
    "text": "The server won't list this folder.",
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
    "x": 93.61,
    "y": 268.44,
    "w": 249.24,
    "h": 20.3,
    "text": "Pick another folder or type a path",
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
    "id": "t_cal3",
    "x": 92.48,
    "y": 300.0,
    "w": 119.54,
    "h": 19.5,
    "text": "you can access.",
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
    "id": "browser_back",
    "x": 28.0,
    "y": 382.0,
    "w": 350.0,
    "h": 44.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "browser_back_surface",
    "x": 28.0,
    "y": 382.0,
    "w": 350.0,
    "h": 44.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "browser_back_control",
    "x": 28.0,
    "y": 382.0,
    "w": 350.0,
    "h": 44.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "browser_back_label",
    "x": 128.0,
    "y": 394.0,
    "w": 139.0,
    "h": 22.5,
    "text": "Back to /home/user",
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
    "id": "browser_path_wrap",
    "x": 28.0,
    "y": 488.0,
    "w": 350.0,
    "h": 48.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "browser_path_field",
    "x": 28.0,
    "y": 488.0,
    "w": 350.0,
    "h": 48.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "browser_path",
    "x": 42.0,
    "y": 488.0,
    "w": 322.0,
    "h": 48.0,
    "text": "/private",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15,
    "weight": 400,
    "role": "input",
    "native_candidates": [
      "TextInput",
      "KitFormField"
    ]
  }
]
```
