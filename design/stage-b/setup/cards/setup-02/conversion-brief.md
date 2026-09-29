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
    "x": 82.33,
    "y": 102.5,
    "w": 225.56,
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
    "x": 48.49,
    "y": 177.08,
    "w": 53.01,
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
    "h": 45.43,
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
    "x": 48.47,
    "y": 300.98,
    "w": 106.07,
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
    "y": 344.42,
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
    "y": 356.42,
    "w": 286.0,
    "h": 22.5,
    "text": "\u2022",
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
    "id": "token_eye",
    "x": 328.0,
    "y": 351.49,
    "w": 20.0,
    "h": 20.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "error_band",
    "x": 46.0,
    "y": 396.92,
    "w": 314.0,
    "h": 37.58,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_error",
    "x": 47.37,
    "y": 404.92,
    "w": 207.51,
    "h": 21.58,
    "text": "Token rejected by the server",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 13,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_last",
    "x": 48.49,
    "y": 465.83,
    "w": 207.51,
    "h": 22.67,
    "text": "Last tried 9:41 PM",
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
    "id": "retry",
    "x": 198.49,
    "y": 458.83,
    "w": 62.0,
    "h": 36.67,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "retry_surface",
    "x": 198.49,
    "y": 458.83,
    "w": 62.0,
    "h": 36.67,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "retry_control",
    "x": 198.49,
    "y": 458.83,
    "w": 62.0,
    "h": 36.67,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "retry_label",
    "x": 208.49,
    "y": 465.83,
    "w": 42.0,
    "h": 22.68,
    "text": "Retry",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15.12,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "connect",
    "x": 138.79,
    "y": 564.94,
    "w": 123.9,
    "h": 44.27,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "connect_surface",
    "x": 138.79,
    "y": 564.94,
    "w": 123.9,
    "h": 44.27,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "connect_control",
    "x": 138.79,
    "y": 564.94,
    "w": 123.9,
    "h": 44.27,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "connect_label",
    "x": 159.79,
    "y": 574.94,
    "w": 81.9,
    "h": 24.27,
    "text": "Connect",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 16.18,
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
    "w": 168.04,
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
