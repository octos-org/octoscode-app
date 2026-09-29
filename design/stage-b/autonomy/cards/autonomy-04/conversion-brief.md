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
    "id": "loops_screen",
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
    "x": 22.31,
    "y": 18.3,
    "w": 92.85,
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
    "id": "t_title",
    "x": 18.49,
    "y": 87.18,
    "w": 63.71,
    "h": 31.75,
    "text": "Loops",
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
    "id": "new_loop",
    "x": 271.76,
    "y": 83.34,
    "w": 119.77,
    "h": 39.26,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "new_loop_surface",
    "x": 271.76,
    "y": 83.34,
    "w": 119.77,
    "h": 39.26,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "new_loop_control",
    "x": 271.76,
    "y": 83.34,
    "w": 119.77,
    "h": 39.26,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "new_loop_icon",
    "x": 281.76,
    "y": 93.97,
    "w": 18.0,
    "h": 18.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "new_loop_label",
    "x": 283.76,
    "y": 89.34,
    "w": 95.77,
    "h": 27.27,
    "text": "+ New loop",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 18.18,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "loop_1",
    "x": 16.0,
    "y": 155.24,
    "w": 374.0,
    "h": 121.82,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "loop_1_dot",
    "x": 204.5,
    "y": 167.24,
    "w": 20.0,
    "h": 26.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "loop_1_name",
    "x": 23.85,
    "y": 171.24,
    "w": 110.55,
    "h": 22.5,
    "text": "Run CI smoke",
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
    "id": "loop_1_cad",
    "x": 28.75,
    "y": 215.22,
    "w": 83.16,
    "h": 21.83,
    "text": "every 15 min",
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
    "id": "loop_1_pause",
    "x": 257.2,
    "y": 251.06,
    "w": 18.0,
    "h": 20.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "loop_1_play",
    "x": 306.9,
    "y": 251.06,
    "w": 18.0,
    "h": 20.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "loop_1_trash",
    "x": 356.2,
    "y": 251.06,
    "w": 18.0,
    "h": 20.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "loop_2",
    "x": 16.0,
    "y": 278.29,
    "w": 374.0,
    "h": 121.06,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "loop_2_dot",
    "x": 204.5,
    "y": 290.29,
    "w": 20.0,
    "h": 26.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "loop_2_name",
    "x": 22.38,
    "y": 294.29,
    "w": 87.91,
    "h": 23.23,
    "text": "Sync main",
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
    "id": "loop_2_cad",
    "x": 22.38,
    "y": 336.11,
    "w": 94.31,
    "h": 23.23,
    "text": "every 30 min",
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
    "id": "loop_2_pause",
    "x": 257.2,
    "y": 373.35,
    "w": 18.0,
    "h": 20.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "loop_2_play",
    "x": 306.9,
    "y": 373.35,
    "w": 18.0,
    "h": 20.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "loop_2_trash",
    "x": 356.2,
    "y": 373.35,
    "w": 18.0,
    "h": 20.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "loop_3",
    "x": 16.0,
    "y": 400.43,
    "w": 374.0,
    "h": 121.28,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "loop_3_dot",
    "x": 204.5,
    "y": 412.43,
    "w": 20.0,
    "h": 26.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "loop_3_name",
    "x": 22.33,
    "y": 416.43,
    "w": 118.38,
    "h": 25.23,
    "text": "Nightly review",
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
    "id": "loop_3_cad",
    "x": 22.38,
    "y": 460.02,
    "w": 118.28,
    "h": 21.68,
    "text": "every day \u00b7 01:00",
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
    "id": "loop_3_play",
    "x": 306.9,
    "y": 495.71,
    "w": 18.0,
    "h": 20.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "loop_3_trash",
    "x": 356.2,
    "y": 495.71,
    "w": 18.0,
    "h": 20.0,
    "role": "unknown",
    "native_candidates": []
  }
]
```
