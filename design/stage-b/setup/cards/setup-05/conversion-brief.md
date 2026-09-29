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
    "x": 82.24,
    "y": 80.66,
    "w": 181.74,
    "h": 30.0,
    "text": "Session settings",
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
    "id": "t_close",
    "x": 341.72,
    "y": 75.57,
    "w": 20.3,
    "h": 24.0,
    "text": "\u2715",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 16,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_model",
    "x": 42.86,
    "y": 155.65,
    "w": 53.01,
    "h": 21.43,
    "text": "Model",
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
    "id": "model",
    "x": 46.0,
    "y": 201.0,
    "w": 314.0,
    "h": 50.12,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "model_field",
    "x": 60.0,
    "y": 213.0,
    "w": 286.0,
    "h": 26.12,
    "text": "deepseek-v4-flash",
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
    "id": "model_chev",
    "x": 330.0,
    "y": 216.06,
    "w": 20.0,
    "h": 20.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_saved",
    "x": 43.98,
    "y": 274.0,
    "w": 151.12,
    "h": 21.51,
    "text": "Saved for this profile",
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
    "id": "t_perm",
    "x": 43.93,
    "y": 376.41,
    "w": 100.49,
    "h": 21.0,
    "text": "Permissions",
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
    "id": "segments",
    "x": 46.0,
    "y": 426.21,
    "w": 314.0,
    "h": 38.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "seg_sel",
    "x": 48.54,
    "y": 430.21,
    "w": 104.79,
    "h": 30.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_seg0",
    "x": 58.54,
    "y": 437.21,
    "w": 84.79,
    "h": 22.27,
    "text": "On request",
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
    "id": "t_seg1",
    "x": 171.39,
    "y": 437.49,
    "w": 73.38,
    "h": 19.5,
    "text": "On failure",
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
    "id": "t_seg2",
    "x": 290.92,
    "y": 437.52,
    "w": 45.2,
    "h": 19.5,
    "text": "Never",
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
    "id": "segdiv1",
    "x": 157.36,
    "y": 430.21,
    "w": 1.0,
    "h": 30.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "segdiv2",
    "x": 267.84,
    "y": 430.21,
    "w": 1.0,
    "h": 30.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_sandbox",
    "x": 43.86,
    "y": 566.87,
    "w": 73.55,
    "h": 21.0,
    "text": "Sandbox",
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
    "id": "sandbox_box",
    "x": 46.0,
    "y": 617.0,
    "w": 314.0,
    "h": 42.67,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_sandbox_v",
    "x": 58.64,
    "y": 627.0,
    "w": 294.35,
    "h": 22.67,
    "text": "Enabled \u00b7 Network off \u00b7 Workspace write",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 13,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
