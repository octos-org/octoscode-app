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
    "x": 108.27,
    "y": 93.5,
    "w": 187.21,
    "h": 30.0,
    "text": "Pair with Octos",
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
    "id": "cal_red",
    "x": 28.0,
    "y": 159.95,
    "w": 350.0,
    "h": 86.14,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_cal1",
    "x": 55.26,
    "y": 175.95,
    "w": 276.31,
    "h": 21.0,
    "text": "This pairing link was already used.",
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
    "x": 55.26,
    "y": 212.0,
    "w": 213.15,
    "h": 19.5,
    "text": "Ask Octos for a new code.",
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
    "id": "t_srv_label",
    "x": 25.89,
    "y": 289.71,
    "w": 53.1,
    "h": 19.5,
    "text": "Server",
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
    "id": "connect_server_wrap",
    "x": 28.0,
    "y": 322.0,
    "w": 350.0,
    "h": 48.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "connect_server_field",
    "x": 28.0,
    "y": 322.0,
    "w": 350.0,
    "h": 48.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "connect_server",
    "x": 42.0,
    "y": 322.0,
    "w": 322.0,
    "h": 48.0,
    "text": "http://192.168.1.20:50190",
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
    "id": "t_tok_label",
    "x": 25.94,
    "y": 404.92,
    "w": 104.88,
    "h": 19.5,
    "text": "Access token",
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
    "id": "connect_token_wrap",
    "x": 28.0,
    "y": 436.0,
    "w": 350.0,
    "h": 48.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "connect_token_field",
    "x": 28.0,
    "y": 436.0,
    "w": 350.0,
    "h": 48.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "connect_token",
    "x": 42.0,
    "y": 436.0,
    "w": 322.0,
    "h": 48.0,
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
    "id": "connect_submit",
    "x": 128.0,
    "y": 600.0,
    "w": 150.0,
    "h": 48.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "connect_submit_surface",
    "x": 128.0,
    "y": 600.0,
    "w": 150.0,
    "h": 48.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "connect_submit_control",
    "x": 128.0,
    "y": 600.0,
    "w": 150.0,
    "h": 48.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "connect_submit_label",
    "x": 160.0,
    "y": 614.0,
    "w": 72.0,
    "h": 22.5,
    "text": "Connect",
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
