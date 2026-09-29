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
    "id": "scrim",
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
    "id": "modal",
    "x": 40.0,
    "y": 112.0,
    "w": 326.0,
    "h": 546.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "icon_close",
    "x": 336.08,
    "y": 130.84,
    "w": 19.17,
    "h": 20.3,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_query",
    "x": 50.75,
    "y": 129.5,
    "w": 59.77,
    "h": 25.02,
    "text": "/ mo",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 15,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "row_sel",
    "x": 48.0,
    "y": 194.45,
    "w": 310.0,
    "h": 52.1,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_cmd0",
    "x": 50.38,
    "y": 208.45,
    "w": 74.04,
    "h": 24.1,
    "text": "/ model",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 14,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_desc0",
    "x": 251.42,
    "y": 210.53,
    "w": 105.02,
    "h": 19.96,
    "text": "Switch model",
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
    "id": "t_cmd1",
    "x": 51.84,
    "y": 275.02,
    "w": 93.69,
    "h": 21.0,
    "text": "/monitor",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 14,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_desc1",
    "x": 246.98,
    "y": 276.34,
    "w": 108.27,
    "h": 19.5,
    "text": "Add a monitor",
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
    "id": "t_cmd2",
    "x": 51.84,
    "y": 340.52,
    "w": 62.11,
    "h": 22.78,
    "text": "/ mode",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 14,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_desc2",
    "x": 201.87,
    "y": 341.76,
    "w": 152.25,
    "h": 22.74,
    "text": "Change permissions",
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
    "id": "t_cmd3",
    "x": 53.01,
    "y": 407.0,
    "w": 92.48,
    "h": 24.0,
    "text": "/ compact",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 14,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_desc3",
    "x": 226.56,
    "y": 407.6,
    "w": 128.8,
    "h": 23.96,
    "text": "Compact context",
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
    "id": "t_cmd4",
    "x": 50.75,
    "y": 472.5,
    "w": 49.62,
    "h": 22.65,
    "text": "/ btw",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 14,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_desc4",
    "x": 208.64,
    "y": 474.85,
    "w": 145.48,
    "h": 21.65,
    "text": "Ask a side question",
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
    "id": "t_cmd5",
    "x": 53.01,
    "y": 538.0,
    "w": 81.2,
    "h": 21.5,
    "text": "/ resume",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 14,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_desc5",
    "x": 217.66,
    "y": 540.27,
    "w": 135.33,
    "h": 19.5,
    "text": "Resume a session",
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
    "id": "t_hints",
    "x": 72.18,
    "y": 620.35,
    "w": 250.37,
    "h": 18.0,
    "text": "\u2191\u2193 to move \u00b7 \u21b5 to run \u00b7 esc",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 12,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
