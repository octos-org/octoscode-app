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
    "x": 0,
    "y": 0,
    "w": 406,
    "h": 776,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "tool_1",
    "x": 79.97,
    "y": 69.83,
    "w": 247.19,
    "h": 38.67,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "t01",
    "x": 87.96666879879072,
    "y": 77.82558119961651,
    "w": 231.19,
    "h": 24.49,
    "text": "Read ui_protocol_transport.rs",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 16.33,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t02",
    "x": 86.84,
    "y": 104.9,
    "w": 78.94,
    "h": 19.56,
    "text": "\u2022 412 lines",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 13.04,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "tool_2",
    "x": 79.97,
    "y": 197.28,
    "w": 193.06,
    "h": 40.81,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "t03",
    "x": 87.9666673824301,
    "y": 205.27906969900806,
    "w": 177.06,
    "h": 26.8,
    "text": "Search 'steer_dropped'",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 17.87,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t04",
    "x": 86.62,
    "y": 234.57,
    "w": 88.41,
    "h": 21.99,
    "text": "\u2022 7 matches",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 14.66,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "tool_3",
    "x": 27.44,
    "y": 328.27,
    "w": 286.67,
    "h": 48.33,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "t05",
    "x": 35.444446227929745,
    "y": 336.26666689828295,
    "w": 270.67,
    "h": 33.0,
    "text": ">_ Ran cargo test -p octos-cli",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 22,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t06",
    "x": 87.66,
    "y": 370.55,
    "w": 89.57,
    "h": 21.84,
    "text": "steer_queue",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 14.56,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t07",
    "x": 45.05,
    "y": 442.86,
    "w": 135.46,
    "h": 24.03,
    "text": "running 12 tests",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 16.02,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t08",
    "x": 143.23,
    "y": 488.38,
    "w": 15.79,
    "h": 16.5,
    "text": "\u2022\u2022",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 11,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t09",
    "x": 39.47,
    "y": 515.45,
    "w": 302.24,
    "h": 25.58,
    "text": "test steer_queue::reconnect_ok ... ok",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 17.05,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t10",
    "x": 42.86,
    "y": 568.47,
    "w": 303.37,
    "h": 25.58,
    "text": "test result: ok. 12 passed; 0 failed",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 17.05,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
