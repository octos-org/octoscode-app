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
    "id": "outer_card",
    "x": 10.0,
    "y": 10.0,
    "w": 386.0,
    "h": 756.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_title",
    "x": 78.94,
    "y": 102.5,
    "w": 226.68,
    "h": 30.0,
    "text": "Connect to Octos",
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
    "id": "t_server",
    "x": 47.3,
    "y": 175.76,
    "w": 52.01,
    "h": 21.0,
    "text": "Server",
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
    "id": "server",
    "x": 46.0,
    "y": 213.58,
    "w": 314.0,
    "h": 44.3,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "server_field",
    "x": 60.0,
    "y": 225.58,
    "w": 286.0,
    "h": 22.5,
    "text": "http://127.0.0.1:50190",
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
    "id": "t_token",
    "x": 48.45,
    "y": 299.77,
    "w": 104.97,
    "h": 21.0,
    "text": "Access token",
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
    "id": "token",
    "x": 46.0,
    "y": 343.29,
    "w": 314.0,
    "h": 34.15,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "token_field",
    "x": 60.0,
    "y": 355.29,
    "w": 286.0,
    "h": 22.5,
    "text": "\u2022\u2022\u2022\u2022\u2022\u2022\u2022\u2022\u2022\u2022\u2022\u2022\u2022\u2022\u2022\u2022\u2022\u2022\u2022",
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
    "id": "t_hint",
    "x": 47.14,
    "y": 403.53,
    "w": 184.31,
    "h": 21.82,
    "text": "Stored for this server only",
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
    "id": "connect",
    "x": 137.78,
    "y": 565.43,
    "w": 124.79,
    "h": 43.3,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "connect_surface",
    "x": 137.78,
    "y": 565.43,
    "w": 124.79,
    "h": 43.3,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "connect_control",
    "x": 137.78,
    "y": 565.43,
    "w": 124.79,
    "h": 43.3,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "connect_label",
    "x": 158.78,
    "y": 575.43,
    "w": 82.79,
    "h": 23.3,
    "text": "Connect",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 15.53,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_local",
    "x": 118.42,
    "y": 662.0,
    "w": 166.91,
    "h": 19.5,
    "text": "Use local solo server",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 13,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
