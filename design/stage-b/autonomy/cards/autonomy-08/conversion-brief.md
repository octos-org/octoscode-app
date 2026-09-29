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
    "id": "t02",
    "x": 8.53,
    "y": 207.72,
    "w": 160.35,
    "h": 27.43,
    "text": "Resume a session",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 17,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "row_1_sel",
    "x": 16.0,
    "y": 269.68,
    "w": 374.0,
    "h": 98.96,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "row_1_radio",
    "x": 20.0,
    "y": 285.68,
    "w": 18.0,
    "h": 18.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "row_1_title",
    "x": 44.29,
    "y": 283.68,
    "w": 128.08,
    "h": 26.39,
    "text": "Add session fork",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 15,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "row_1_meta",
    "x": 46.06,
    "y": 331.17,
    "w": 168.88,
    "h": 25.47,
    "text": "octos \u2022 2h ago \u2022 14 turns",
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
    "id": "div_0",
    "x": 16.0,
    "y": 372.65,
    "w": 374.0,
    "h": 1.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "row_2_radio",
    "x": 20.0,
    "y": 405.68,
    "w": 18.0,
    "h": 18.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "row_2_title",
    "x": 46.06,
    "y": 403.68,
    "w": 238.82,
    "h": 29.39,
    "text": "Fix steer queue drop on reconnect",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 15,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "row_2_meta",
    "x": 46.06,
    "y": 448.75,
    "w": 168.88,
    "h": 25.47,
    "text": "octos \u2022 1d ago \u2022 23 turns",
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
    "id": "div_1",
    "x": 16.0,
    "y": 490.22,
    "w": 374.0,
    "h": 1.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "row_3_radio",
    "x": 20.0,
    "y": 523.25,
    "w": 18.0,
    "h": 18.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "row_3_title",
    "x": 46.06,
    "y": 521.25,
    "w": 129.65,
    "h": 25.47,
    "text": "Review PR #2566",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 15,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "row_3_meta",
    "x": 46.06,
    "y": 568.28,
    "w": 168.88,
    "h": 23.52,
    "text": "octos \u2022 3d ago \u2022 17 turns",
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
    "id": "div_2",
    "x": 16.0,
    "y": 607.8,
    "w": 374.0,
    "h": 1.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "row_4_radio",
    "x": 20.0,
    "y": 640.83,
    "w": 18.0,
    "h": 18.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "row_4_title",
    "x": 40.94,
    "y": 638.83,
    "w": 163.76,
    "h": 29.39,
    "text": "Why is hydrate slow?",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 15,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "row_4_meta",
    "x": 46.06,
    "y": 685.86,
    "w": 158.65,
    "h": 23.52,
    "text": "octos \u2022 4d ago \u2022 9 turns",
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
    "id": "confirm_strip",
    "x": 16.0,
    "y": 725.37,
    "w": 374.0,
    "h": 52.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_confirm",
    "x": 30.0,
    "y": 739.37,
    "w": 190.0,
    "h": 24.0,
    "text": "Resume \"Add session fork\"?",
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
    "id": "resume_pill",
    "x": 226.0,
    "y": 734.37,
    "w": 74.0,
    "h": 34.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_resume",
    "x": 240.0,
    "y": 741.37,
    "w": 54.0,
    "h": 22.0,
    "text": "Resume",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 14,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_cancel2",
    "x": 314.0,
    "y": 741.37,
    "w": 52.0,
    "h": 22.0,
    "text": "Cancel",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 14,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
