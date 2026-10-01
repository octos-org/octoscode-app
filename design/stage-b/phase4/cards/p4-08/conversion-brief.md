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
    "x": 56.39,
    "y": 108.28,
    "w": 287.58,
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
    "x": 15.79,
    "y": 154.5,
    "w": 235.71,
    "h": 19.5,
    "text": "/ > Users \u203a dev \u203a code",
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
    "id": "row_0",
    "x": 28.0,
    "y": 196.0,
    "w": 350.0,
    "h": 48.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "fld_0",
    "x": 46.0,
    "y": 211.0,
    "w": 18.0,
    "h": 18.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_row_0",
    "x": 82.3,
    "y": 213.07,
    "w": 50.81,
    "h": 21.0,
    "text": "octos",
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
    "id": "row_0_control",
    "x": 28.0,
    "y": 196.0,
    "w": 350.0,
    "h": 48.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "row_1",
    "x": 28.0,
    "y": 248.0,
    "w": 350.0,
    "h": 48.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "fld_1",
    "x": 46.0,
    "y": 263.0,
    "w": 18.0,
    "h": 18.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_row_1",
    "x": 86.35,
    "y": 265.22,
    "w": 125.18,
    "h": 25.47,
    "text": "octoscode-app",
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
    "id": "row_1_control",
    "x": 28.0,
    "y": 248.0,
    "w": 350.0,
    "h": 48.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "row_2",
    "x": 28.0,
    "y": 308.0,
    "w": 350.0,
    "h": 48.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "fld_2",
    "x": 46.0,
    "y": 323.0,
    "w": 18.0,
    "h": 18.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_row_2",
    "x": 86.79,
    "y": 324.71,
    "w": 47.45,
    "h": 21.0,
    "text": "notes",
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
    "id": "row_2_control",
    "x": 28.0,
    "y": 308.0,
    "w": 350.0,
    "h": 48.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "row_3",
    "x": 28.0,
    "y": 364.0,
    "w": 350.0,
    "h": 48.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "fld_3",
    "x": 46.0,
    "y": 379.0,
    "w": 18.0,
    "h": 18.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_row_3",
    "x": 85.67,
    "y": 381.08,
    "w": 63.24,
    "h": 21.0,
    "text": "scratch",
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
    "id": "row_3_control",
    "x": 28.0,
    "y": 364.0,
    "w": 350.0,
    "h": 48.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "t_hidden",
    "x": 25.94,
    "y": 433.0,
    "w": 178.19,
    "h": 19.5,
    "text": "3 hidden by the server",
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
    "id": "browser_path",
    "x": 28.0,
    "y": 490.0,
    "w": 350.0,
    "h": 48.0,
    "text": "/Users/dev/code/octoscode-app",
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
    "id": "browser_use",
    "x": 28.0,
    "y": 560.0,
    "w": 350.0,
    "h": 48.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "browser_use_surface",
    "x": 28.0,
    "y": 560.0,
    "w": 350.0,
    "h": 48.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "browser_use_control",
    "x": 28.0,
    "y": 560.0,
    "w": 350.0,
    "h": 48.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "browser_use_label",
    "x": 126.0,
    "y": 574.0,
    "w": 132.0,
    "h": 22.5,
    "text": "Use this folder",
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
