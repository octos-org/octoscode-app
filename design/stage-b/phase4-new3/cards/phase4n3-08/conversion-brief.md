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
    "id": "strip",
    "x": 0,
    "y": 0,
    "w": 406,
    "h": 64,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "strip_model_t",
    "x": 16,
    "y": 24,
    "w": 110,
    "h": 16,
    "text": "glm-4.6",
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
    "id": "strip_model",
    "x": 0,
    "y": 0,
    "w": 135,
    "h": 64,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "strip_d1",
    "x": 135,
    "y": 12,
    "w": 1,
    "h": 40,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "strip_activity_t",
    "x": 143,
    "y": 24,
    "w": 118,
    "h": 16,
    "text": "Working\u2026",
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
    "id": "strip_activity",
    "x": 136,
    "y": 0,
    "w": 133,
    "h": 64,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "strip_d2",
    "x": 270,
    "y": 12,
    "w": 1,
    "h": 40,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "strip_perm_t",
    "x": 286,
    "y": 24,
    "w": 100,
    "h": 16,
    "text": "Write",
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
    "id": "strip_permissions",
    "x": 271,
    "y": 0,
    "w": 135,
    "h": 64,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "t_legend",
    "x": 16,
    "y": 80,
    "w": 240,
    "h": 14,
    "text": "Model, permissions, sandbox",
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
    "id": "state_0",
    "x": 16,
    "y": 108,
    "w": 90.4,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "state_0_t",
    "x": 24,
    "y": 111,
    "w": 74.4,
    "h": 14,
    "text": "Reconnecting",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 10,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "state_1",
    "x": 16,
    "y": 142,
    "w": 102.8,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "state_1_t",
    "x": 24,
    "y": 145,
    "w": 86.8,
    "h": 14,
    "text": "Resuming chat\u2026",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 10,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "state_2",
    "x": 16,
    "y": 176,
    "w": 146.2,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "state_2_t",
    "x": 24,
    "y": 179,
    "w": 130.2,
    "h": 14,
    "text": "Handing back control\u2026",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 10,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "info",
    "x": 16,
    "y": 232,
    "w": 342,
    "h": 40,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "info_dot",
    "x": 28,
    "y": 161,
    "w": 10,
    "h": 10,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "info_t",
    "x": 48,
    "y": 156,
    "w": 300,
    "h": 16,
    "text": "Another app is using this session",
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
