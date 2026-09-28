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
    "id": "t01",
    "x": 77.82,
    "y": 98.0,
    "w": 210.89,
    "h": 27.2,
    "text": "Run this command?",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 18.13,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t02",
    "x": 29.0,
    "y": 187.5,
    "w": 328.67,
    "h": 32.51,
    "text": "git push origin feat/steer-queue",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 21.67,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t03",
    "x": 21.43,
    "y": 276.34,
    "w": 297.73,
    "h": 21.66,
    "text": "Reason: Push the fix branch so Cl",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 14.44,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t04",
    "x": 19.14,
    "y": 317.93,
    "w": 66.6,
    "h": 16.07,
    "text": "can run",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 10.71,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t05",
    "x": 136.31,
    "y": 389.55,
    "w": 125.48,
    "h": 28.48,
    "text": "Approve once",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 18.99,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t06",
    "x": 109.48,
    "y": 471.66,
    "w": 180.59,
    "h": 33.14,
    "text": "Approve for session",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 22.09,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t07",
    "x": 170.29,
    "y": 552.67,
    "w": 47.37,
    "h": 27.08,
    "text": "Deny",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 18.05,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t08",
    "x": 153.38,
    "y": 653.0,
    "w": 78.94,
    "h": 21.5,
    "text": "Y /S / N",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 14.33,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
