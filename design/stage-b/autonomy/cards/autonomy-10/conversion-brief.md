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
    "id": "user_bubble",
    "x": 113.32,
    "y": 157.78,
    "w": 271.22,
    "h": 78.05,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_q1",
    "x": 127.65,
    "y": 167.78,
    "w": 242.89,
    "h": 24.2,
    "text": "Why is the steer queue dropping",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 16.13,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_q2",
    "x": 127.32,
    "y": 201.37,
    "w": 185.25,
    "h": 24.47,
    "text": "messages on reconnect?",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 16.31,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "tool_card",
    "x": 14.0,
    "y": 256.2,
    "w": 374.0,
    "h": 53.88,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "icon_tool",
    "x": 24.0,
    "y": 266.2,
    "w": 18.0,
    "h": 18.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_tool",
    "x": 52.0,
    "y": 266.2,
    "w": 280.0,
    "h": 33.88,
    "text": "Read crates/octos-core/src/ui_protocol.rs",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 13,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "icon_tool_done",
    "x": 356.0,
    "y": 267.2,
    "w": 16.0,
    "h": 16.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "aside_card",
    "x": 16.0,
    "y": 340.0,
    "w": 374.0,
    "h": 250.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_aside",
    "x": 28.37,
    "y": 359.77,
    "w": 109.92,
    "h": 19.5,
    "text": "Aside \u00b7 /btw",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 13,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_dismiss",
    "x": 297.85,
    "y": 359.77,
    "w": 86.87,
    "h": 18.0,
    "text": "Dismiss aside",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 12,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "icon_dismiss",
    "x": 390.72,
    "y": 360.77,
    "w": 14.0,
    "h": 14.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_qtitle",
    "x": 28.37,
    "y": 406.55,
    "w": 248.21,
    "h": 22.59,
    "text": "What does steer_dropped mean?",
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
    "id": "aside_answer",
    "x": 26.0,
    "y": 452.0,
    "w": 330.0,
    "h": 138.0,
    "text": "It's a metric that increments when messages are dropped from the steer queue due to a reconnect or protocol error. It helps track message loss.",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 17,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "composer",
    "x": 16.0,
    "y": 622.0,
    "w": 374.0,
    "h": 130.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "composer_input",
    "x": 26.0,
    "y": 632.0,
    "w": 300.0,
    "h": 40.0,
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15,
    "role": "input",
    "native_candidates": [
      "TextInput",
      "KitFormField"
    ]
  },
  {
    "id": "icon_plus",
    "x": 26.0,
    "y": 692.0,
    "w": 18.0,
    "h": 24.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "approval_pill",
    "x": 64.0,
    "y": 696.0,
    "w": 125.59,
    "h": 36.32,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_approval",
    "x": 74.08,
    "y": 700.11,
    "w": 103.59,
    "h": 24.32,
    "text": "Ask for approval",
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
    "id": "t_model",
    "x": 203.89,
    "y": 701.79,
    "w": 72.69,
    "h": 19.5,
    "text": "v4-flash \u25be",
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
    "id": "send_btn",
    "x": 344.0,
    "y": 686.0,
    "w": 36.0,
    "h": 36.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "icon_send",
    "x": 354.0,
    "y": 696.0,
    "w": 16.0,
    "h": 16.0,
    "role": "unknown",
    "native_candidates": []
  }
]
```
