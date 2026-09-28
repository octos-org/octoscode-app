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
    "id": "goal_strip",
    "x": 16.0,
    "y": 28.0,
    "w": 374.0,
    "h": 40.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t01",
    "x": 33.83,
    "y": 36.0,
    "w": 254.88,
    "h": 21.52,
    "text": "Goal \u2022 Fix steer queue on reconnect \u2022 18m",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 14.35,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "icon_pause",
    "x": 330.0,
    "y": 32.0,
    "w": 16.0,
    "h": 18.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t02",
    "x": 33.73,
    "y": 151.77,
    "w": 103.95,
    "h": 22.42,
    "text": "Plan \u2022 3 of 5",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 14.94,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "plan_steps",
    "x": 24.0,
    "y": 200.0,
    "w": 368.0,
    "h": 420.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "step_0_label",
    "x": 82.33,
    "y": 235.5,
    "w": 198.49,
    "h": 24.0,
    "text": "Reproduce reconnect drop",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 16.0,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "step_1_label",
    "x": 85.71,
    "y": 321.45,
    "w": 192.85,
    "h": 23.69,
    "text": "Trace steer queue lifecycle",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15.79,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "step_2_label",
    "x": 85.71,
    "y": 407.0,
    "w": 124.06,
    "h": 25.01,
    "text": "Write failing test",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 16.67,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "step_3_label",
    "x": 77.82,
    "y": 494.0,
    "w": 197.36,
    "h": 23.71,
    "text": "Implement durable queue",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 15.81,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "step_4_label",
    "x": 87.97,
    "y": 579.5,
    "w": 166.91,
    "h": 23.0,
    "text": "Run full suite and push",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15.33,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "icon_step0",
    "x": 42.0,
    "y": 228.0,
    "w": 24.0,
    "h": 24.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "icon_step1",
    "x": 42.0,
    "y": 314.0,
    "w": 24.0,
    "h": 24.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "icon_step2",
    "x": 42.0,
    "y": 400.0,
    "w": 24.0,
    "h": 24.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "icon_step3",
    "x": 42.0,
    "y": 486.0,
    "w": 24.0,
    "h": 24.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "icon_step4",
    "x": 42.0,
    "y": 572.0,
    "w": 24.0,
    "h": 24.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  }
]
```
