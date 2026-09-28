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
    "x": 78.94,
    "y": 82.34,
    "w": 246.98,
    "h": 29.34,
    "text": "Octos needs a decision",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 19.56,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t02",
    "x": 37.22,
    "y": 154.5,
    "w": 257.13,
    "h": 30.48,
    "text": "Where should queued steers",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 20.32,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t03",
    "x": 36.09,
    "y": 192.87,
    "w": 121.8,
    "h": 30.45,
    "text": "be persisted?",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 20.3,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t04",
    "x": 75.1,
    "y": 254.59,
    "w": 181.49,
    "h": 33.0,
    "text": "In the session ledger",
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
    "id": "t05",
    "x": 76.61,
    "y": 293.91,
    "w": 140.01,
    "h": 27.83,
    "text": "(recommended)",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 18.55,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t06",
    "x": 76.66,
    "y": 355.12,
    "w": 138.78,
    "h": 28.38,
    "text": "In memory only",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 18.92,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t07",
    "x": 79.96,
    "y": 415.52,
    "w": 124.28,
    "h": 24.6,
    "text": "Ask each time",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 16.4,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "note_input",
    "x": 31.26,
    "y": 485.14,
    "w": 200,
    "h": 40.33,
    "text": "",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 17.52,
    "weight": 400,
    "role": "input",
    "native_candidates": [
      "TextInput",
      "KitFormField"
    ]
  },
  {
    "id": "t08",
    "x": 39.26405983846847,
    "y": 493.1372056586084,
    "w": 92.89,
    "h": 26.28,
    "text": "Add a note",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 17.52,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "submit_answer",
    "x": 129.55,
    "y": 582.77,
    "w": 151.42,
    "h": 40.19,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "t09",
    "x": 137.54531165402653,
    "y": 590.7717433066812,
    "w": 135.42,
    "h": 26.13,
    "text": "Submit answer",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 17.42,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "skip",
    "x": 172.44,
    "y": 665.36,
    "w": 58.86,
    "h": 45.33,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "t10",
    "x": 180.44444420375925,
    "y": 673.3604656987765,
    "w": 42.86,
    "h": 31.66,
    "text": "Skip",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 21.11,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
