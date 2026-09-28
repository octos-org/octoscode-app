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
    "x": 22.46,
    "y": 46.77,
    "w": 154.7,
    "h": 29.31,
    "text": "OctosCode v",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 19.54,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "new_chat",
    "x": 27.95,
    "y": 129.02,
    "w": 103.12,
    "h": 38.6,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "t02",
    "x": 35.948785546400075,
    "y": 137.02212955863436,
    "w": 87.12,
    "h": 24.41,
    "text": "New chat",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 16.27,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "thread_1",
    "x": 24.22,
    "y": 221.5,
    "w": 325.33,
    "h": 48.5,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "t03",
    "x": 32.222231299192686,
    "y": 229.50000006902022,
    "w": 309.33,
    "h": 33.0,
    "text": "Fix steer queue drop on reconnect",
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
    "id": "thread_2",
    "x": 26.96,
    "y": 305.5,
    "w": 161.48,
    "h": 37.5,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "t04",
    "x": 34.961108470425344,
    "y": 313.4999995637694,
    "w": 145.48,
    "h": 23.22,
    "text": "Add session fork",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15.48,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "thread_3",
    "x": 28.09,
    "y": 386.77,
    "w": 164.87,
    "h": 40.81,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "t05",
    "x": 36.08888806335851,
    "y": 394.7674417807523,
    "w": 148.87,
    "h": 26.8,
    "text": "Review PR #2566",
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
    "id": "thread_4",
    "x": 28.09,
    "y": 473.5,
    "w": 274.26,
    "h": 42.06,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "t06",
    "x": 36.08888937433068,
    "y": 481.499999744909,
    "w": 258.26,
    "h": 28.14,
    "text": "Bump octos-core to a6ea8505",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 18.76,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "thread_5",
    "x": 26.96,
    "y": 557.0,
    "w": 195.32,
    "h": 43.15,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "t07",
    "x": 34.96111294487628,
    "y": 565.0000001958095,
    "w": 179.32,
    "h": 29.33,
    "text": "Why is hydrate slow?",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 19.55,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
