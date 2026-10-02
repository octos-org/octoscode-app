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
    "id": "strip",
    "x": 0.0,
    "y": 0.0,
    "w": 406.0,
    "h": 128.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_strip1",
    "x": 17.14,
    "y": 45.65,
    "w": 321.64,
    "h": 32.22,
    "text": "New chat defaults \u2022 Ask for approval \u2022 Workspace write \u2022",
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
    "id": "t_strip2",
    "x": 15.82,
    "y": 88.61,
    "w": 154.23,
    "h": 32.22,
    "text": "v4-flash v \u2022 Thinking: On",
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
    "id": "t_change",
    "x": 335.8,
    "y": 92.6,
    "w": 45.59,
    "h": 26.74,
    "text": "Change",
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
    "id": "defaults_change",
    "x": 326.0,
    "y": 84.0,
    "w": 70.0,
    "h": 42.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "defaults_change_surface",
    "x": 326.0,
    "y": 84.0,
    "w": 70.0,
    "h": 42.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "defaults_change_control",
    "x": 326.0,
    "y": 84.0,
    "w": 70.0,
    "h": 42.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "defaults_change_label",
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
    "id": "t_placeholder",
    "x": 112.05,
    "y": 354.44,
    "w": 160.82,
    "h": 42.96,
    "text": "Ask Octos anything",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 16,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_placeholder_sub",
    "x": 92.27,
    "y": 416.19,
    "w": 204.32,
    "h": 26.85,
    "text": "Start a new conversation with Octos.",
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
    "id": "composer",
    "x": 20.0,
    "y": 580.0,
    "w": 366.0,
    "h": 84.0,
    "text": "",
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
