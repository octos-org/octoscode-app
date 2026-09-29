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
    "id": "goal_screen",
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
    "x": 20.12,
    "y": 20.18,
    "w": 106.07,
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
    "id": "goal_card",
    "x": 16.0,
    "y": 182.21,
    "w": 374.0,
    "h": 449.31,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_goal_title",
    "x": 31.09,
    "y": 133.47,
    "w": 54.86,
    "h": 26.38,
    "text": "Goal",
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
    "id": "t_goal",
    "x": 31.09,
    "y": 200.21,
    "w": 245.06,
    "h": 24.83,
    "text": "Fix steer queue on reconnect",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 15,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "goal_badge",
    "x": 33.68,
    "y": 251.66,
    "w": 71.62,
    "h": 27.46,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "goal_badge_label",
    "x": 43.68,
    "y": 255.66,
    "w": 51.62,
    "h": 19.5,
    "text": "Active",
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
    "id": "t_budget",
    "x": 30.91,
    "y": 351.71,
    "w": 111.91,
    "h": 26.01,
    "text": "Token budget",
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
    "id": "t_budget_val",
    "x": 272.5,
    "y": 350.75,
    "w": 98.76,
    "h": 21.0,
    "text": "41k of 100k",
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
    "id": "bar_track",
    "x": 31.09,
    "y": 392.93,
    "w": 330.0,
    "h": 8.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "bar_fill",
    "x": 31.09,
    "y": 392.93,
    "w": 135.3,
    "h": 8.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_elapsed",
    "x": 29.26,
    "y": 460.94,
    "w": 67.67,
    "h": 24.83,
    "text": "Elapsed",
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
    "id": "t_elapsed_val",
    "x": 331.02,
    "y": 460.94,
    "w": 40.23,
    "h": 21.0,
    "text": "18m",
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
    "id": "pause_btn",
    "x": 30.0,
    "y": 559.79,
    "w": 143.0,
    "h": 53.73,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "pause_btn_surface",
    "x": 30.0,
    "y": 559.79,
    "w": 143.0,
    "h": 53.73,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "pause_btn_control",
    "x": 30.0,
    "y": 559.79,
    "w": 143.0,
    "h": 53.73,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "pause_btn_label",
    "x": 82.3,
    "y": 575.79,
    "w": 53.04,
    "h": 21.73,
    "text": "Pause",
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
    "id": "stop_btn",
    "x": 193.0,
    "y": 559.79,
    "w": 142.0,
    "h": 53.73,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "stop_btn_surface",
    "x": 193.0,
    "y": 559.79,
    "w": 142.0,
    "h": 53.73,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "stop_btn_control",
    "x": 193.0,
    "y": 559.79,
    "w": 142.0,
    "h": 53.73,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "stop_btn_label",
    "x": 257.86,
    "y": 577.34,
    "w": 45.72,
    "h": 26.38,
    "text": "Stop",
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
    "id": "clear_goal",
    "x": 136.06,
    "y": 648.75,
    "w": 111.93,
    "h": 37.21,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "clear_goal_surface",
    "x": 136.06,
    "y": 648.75,
    "w": 111.93,
    "h": 37.21,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "clear_goal_control",
    "x": 136.06,
    "y": 648.75,
    "w": 111.93,
    "h": 37.21,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "clear_goal_label",
    "x": 148.06,
    "y": 654.75,
    "w": 87.93,
    "h": 25.21,
    "text": "Clear goal",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 16.81,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
