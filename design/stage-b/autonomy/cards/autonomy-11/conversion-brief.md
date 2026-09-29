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
    "x": 33.01,
    "y": 115.55,
    "w": 90.8,
    "h": 28.5,
    "text": "OctosCode",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 19,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "user_bubble",
    "x": 73.02,
    "y": 173.54,
    "w": 310.39,
    "h": 71.84,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_q1",
    "x": 87.12,
    "y": 183.54,
    "w": 282.28,
    "h": 24.15,
    "text": "Can we add a test for preserve_pending",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 16.1,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_q2",
    "x": 87.02,
    "y": 213.81,
    "w": 73.39,
    "h": 21.57,
    "text": "behavior?",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 14.38,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_tool",
    "x": 34.52,
    "y": 281.49,
    "w": 282.94,
    "h": 26.94,
    "text": "\u270e Edit crates/octos-cli/tests/steer_queue.rs",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 13,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "answer_surface",
    "x": 20.0,
    "y": 344.0,
    "w": 366.0,
    "h": 186.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "answer_md",
    "x": 30.0,
    "y": 352.0,
    "w": 340.0,
    "h": 170.0,
    "text": "I added a test that simulates a reconnect and verifies pending messages are preserved. The test asserts metrics.steer_preserved increments and metrics.steer_dropped remains unchanged.",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 16,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "composer",
    "x": 16.0,
    "y": 616.0,
    "w": 374.0,
    "h": 130.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "composer_input",
    "x": 30.0,
    "y": 626.0,
    "w": 300.0,
    "h": 40.0,
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15,
    "role": "input",
    "native_candidates": [
      "TextInput",
      "KitFormField"
    ]
  },
  {
    "id": "icon_plus",
    "x": 30.0,
    "y": 696.0,
    "w": 18.0,
    "h": 24.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "approval_pill",
    "x": 76.0,
    "y": 700.0,
    "w": 121.45,
    "h": 33.45,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_approval",
    "x": 88.8,
    "y": 704.9,
    "w": 99.45,
    "h": 21.45,
    "text": "Ask for approval",
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
    "id": "t_model",
    "x": 216.07,
    "y": 705.16,
    "w": 69.7,
    "h": 19.5,
    "text": "v4-flash \u25be",
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
    "id": "send_btn",
    "x": 344.0,
    "y": 690.0,
    "w": 36.0,
    "h": 36.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "icon_send",
    "x": 354.0,
    "y": 700.0,
    "w": 16.0,
    "h": 16.0,
    "role": "unknown",
    "native_candidates": []
  }
]
```
