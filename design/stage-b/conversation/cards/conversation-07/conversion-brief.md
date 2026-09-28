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
    "id": "t01",
    "x": 19.97,
    "y": 78.23,
    "w": 129.23,
    "h": 30.8,
    "text": "Edited 3 files",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 20.53,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t02",
    "x": 16.92,
    "y": 126.33,
    "w": 69.92,
    "h": 25.58,
    "text": "+62 -5",
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
    "id": "undo",
    "x": 223.19,
    "y": 78.85,
    "w": 80.28,
    "h": 38.65,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "t03",
    "x": 231.19444490551686,
    "y": 86.84883695090846,
    "w": 64.28,
    "h": 24.46,
    "text": "Undo 9",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 16.31,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "review",
    "x": 311.05,
    "y": 79.67,
    "w": 73.75,
    "h": 38.05,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "t04",
    "x": 319.0467996901899,
    "y": 87.66612256514853,
    "w": 57.75,
    "h": 23.82,
    "text": "Review",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15.88,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t05",
    "x": 37.22,
    "y": 215.43,
    "w": 146.61,
    "h": 20.71,
    "text": "crates/octos-cli/src/",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 13.81,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t06",
    "x": 36.09,
    "y": 250.4,
    "w": 206.38,
    "h": 29.23,
    "text": "api/ui_protocol_transport.rs",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 19.49,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t07",
    "x": 299.99,
    "y": 234.5,
    "w": 66.54,
    "h": 25.92,
    "text": "+31 -4",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 17.28,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t08",
    "x": 37.22,
    "y": 345.14,
    "w": 163.53,
    "h": 20.71,
    "text": "crates/octos-core/src/",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 13.81,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t09",
    "x": 37.16,
    "y": 379.87,
    "w": 102.74,
    "h": 26.1,
    "text": "ui_protocol.rs",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 17.4,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t10",
    "x": 305.63,
    "y": 363.0,
    "w": 60.9,
    "h": 23.34,
    "text": "+9 - 1",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15.56,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t11",
    "x": 36.09,
    "y": 472.59,
    "w": 161.27,
    "h": 21.93,
    "text": "crates/octos-cli/tests/",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 14.62,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t12",
    "x": 35.84,
    "y": 510.36,
    "w": 112.06,
    "h": 27.13,
    "text": "steer_queue.rs",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 18.09,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t13",
    "x": 290.97,
    "y": 490.5,
    "w": 75.56,
    "h": 27.0,
    "text": "+22 -0",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 18.0,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t14",
    "x": 32.62,
    "y": 605.37,
    "w": 82.5,
    "h": 25.04,
    "text": "Show diff",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 16.69,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
