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
    "id": "fleet_card",
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
    "id": "t_title",
    "x": 20,
    "y": 20,
    "w": 160,
    "h": 24.4,
    "text": "Fleet",
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
    "id": "t_empty",
    "x": 246,
    "y": 26,
    "w": 140,
    "h": 16,
    "text": "No peers yet",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 12,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "loading_models",
    "x": 20,
    "y": 52,
    "w": 200,
    "h": 14,
    "text": "Loading models\u2026",
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
    "id": "peer_1",
    "x": 20,
    "y": 80,
    "w": 366,
    "h": 296,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "peer_1_avatar",
    "x": 36,
    "y": 96,
    "w": 32,
    "h": 32,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "peer_1_av",
    "x": 36,
    "y": 104,
    "w": 32,
    "h": 18,
    "text": "A",
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
    "id": "peer_1_model",
    "x": 78,
    "y": 96,
    "w": 200,
    "h": 16,
    "text": "anthropic/claude-3.5-sonnet",
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
    "id": "peer_1_status",
    "x": 78,
    "y": 118,
    "w": 71.8,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "peer_1_status_t",
    "x": 86,
    "y": 121,
    "w": 55.8,
    "h": 14,
    "text": "Available",
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
    "id": "peer_1_adv_row",
    "x": 300,
    "y": 98,
    "w": 70,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "peer_1_adv_t",
    "x": 300,
    "y": 98,
    "w": 70,
    "h": 17,
    "text": "Advanced",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 12,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "peer_1_adv",
    "x": 300,
    "y": 98,
    "w": 70,
    "h": 20,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "peer_1_brief",
    "x": 78,
    "y": 146,
    "w": 290,
    "h": 34,
    "text": "Strong coding and reasoning model with tool use.",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 12,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "lbl_model",
    "x": 36,
    "y": 190,
    "w": 120,
    "h": 14,
    "text": "Model",
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
    "id": "sel_model_row",
    "x": 36,
    "y": 208,
    "w": 334,
    "h": 38,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "sel_model_bg",
    "x": 36,
    "y": 208,
    "w": 334,
    "h": 38,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "sel_model_text",
    "x": 48,
    "y": 218.0,
    "w": 310,
    "h": 18,
    "text": "anthropic/claude-3.5-sonnet",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 12,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "sel_model",
    "x": 36,
    "y": 208,
    "w": 334,
    "h": 38,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "lbl_brief",
    "x": 36,
    "y": 254,
    "w": 120,
    "h": 14,
    "text": "Brief",
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
    "id": "brief_box",
    "x": 36,
    "y": 272,
    "w": 334,
    "h": 56,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "brief_text",
    "x": 48,
    "y": 282,
    "w": 310,
    "h": 36,
    "text": "Focus on code quality, tests, and minimal diffs.",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 12,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "brief_input",
    "x": 36,
    "y": 272,
    "w": 334,
    "h": 56,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "btn_start_row",
    "x": 282,
    "y": 340,
    "w": 88,
    "h": 34,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "btn_start_bg",
    "x": 282,
    "y": 340,
    "w": 88,
    "h": 34,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "btn_start_t",
    "x": 282,
    "y": 348.5,
    "w": 88,
    "h": 17.08,
    "text": "Start",
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
    "id": "btn_start",
    "x": 282,
    "y": 340,
    "w": 88,
    "h": 34,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "peer_2",
    "x": 20,
    "y": 392,
    "w": 366,
    "h": 150,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "peer_2_avatar",
    "x": 36,
    "y": 408,
    "w": 32,
    "h": 32,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "peer_2_av",
    "x": 36,
    "y": 416,
    "w": 32,
    "h": 18,
    "text": "W",
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
    "id": "peer_2_model",
    "x": 78,
    "y": 408,
    "w": 140,
    "h": 16,
    "text": "gpt-4o",
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
    "id": "peer_2_status",
    "x": 220,
    "y": 410,
    "w": 109.0,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "peer_2_status_t",
    "x": 228,
    "y": 413,
    "w": 93.0,
    "h": 14,
    "text": "Waiting for you",
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
    "id": "in_steer_row",
    "x": 36,
    "y": 448,
    "w": 334,
    "h": 38,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "in_steer_bg",
    "x": 36,
    "y": 448,
    "w": 334,
    "h": 38,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "in_steer_text",
    "x": 48,
    "y": 458.0,
    "w": 310,
    "h": 18,
    "text": "Enter steering text",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 12,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "in_steer",
    "x": 36,
    "y": 448,
    "w": 334,
    "h": 38,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "btn_dismiss_row",
    "x": 36,
    "y": 498,
    "w": 70,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "btn_dismiss_t",
    "x": 36,
    "y": 498,
    "w": 70,
    "h": 17,
    "text": "Dismiss",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 12,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "btn_dismiss",
    "x": 36,
    "y": 498,
    "w": 70,
    "h": 20,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "peer_2_rule",
    "x": 120,
    "y": 500,
    "w": 250,
    "h": 1,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "peer_2_note",
    "x": 36,
    "y": 512,
    "w": 334,
    "h": 14,
    "text": "Only while working",
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
    "id": "t_noproject",
    "x": 123,
    "y": 560,
    "w": 160,
    "h": 16,
    "text": "Open a project first",
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
