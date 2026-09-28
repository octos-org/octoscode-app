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
    "id": "question_card",
    "x": 16.0,
    "y": 60.0,
    "w": 374.0,
    "h": 660.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "icon_decision",
    "x": 24.0,
    "y": 80.0,
    "w": 26.0,
    "h": 28.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t01",
    "x": 78.94,
    "y": 82.34,
    "w": 246.98,
    "h": 28.5,
    "text": "Octos needs a decision",
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
    "id": "question_text",
    "x": 37.0,
    "y": 150.0,
    "w": 290.0,
    "h": 62.0,
    "text": "Where should queued steers be persisted?",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 17,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "opt_ledger",
    "x": 34.0,
    "y": 248.59,
    "w": 340.0,
    "h": 43.71,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "opt_ledger_radio",
    "x": 38.0,
    "y": 256.59,
    "w": 20.0,
    "h": 20.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "opt_ledger_label",
    "x": 75.1,
    "y": 254.59,
    "w": 181.49,
    "h": 31.71,
    "text": "In the session ledger",
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
    "id": "opt_memory",
    "x": 34.0,
    "y": 349.12,
    "w": 340.0,
    "h": 38.28,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "opt_memory_radio",
    "x": 38.0,
    "y": 357.12,
    "w": 20.0,
    "h": 20.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "opt_memory_label",
    "x": 76.66,
    "y": 355.12,
    "w": 138.78,
    "h": 26.28,
    "text": "In memory only",
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
    "id": "opt_ask",
    "x": 34.0,
    "y": 409.52,
    "w": 340.0,
    "h": 34.78,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "opt_ask_radio",
    "x": 38.0,
    "y": 417.52,
    "w": 20.0,
    "h": 20.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "opt_ask_label",
    "x": 79.96,
    "y": 415.52,
    "w": 124.28,
    "h": 22.78,
    "text": "Ask each time",
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
    "id": "t_reco",
    "x": 76.61,
    "y": 293.91,
    "w": 140.01,
    "h": 25.76,
    "text": "(recommended)",
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
    "id": "note_box",
    "x": 34.0,
    "y": 478.0,
    "w": 338.0,
    "h": 50.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "note_input",
    "x": 48.0,
    "y": 488.0,
    "w": 310.0,
    "h": 30.0,
    "text": "",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 14,
    "weight": 400,
    "role": "input",
    "native_candidates": [
      "TextInput",
      "KitFormField"
    ]
  },
  {
    "id": "submit_answer",
    "x": 24.0,
    "y": 566.0,
    "w": 362.0,
    "h": 72.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "submit_answer_surface",
    "x": 24.0,
    "y": 566.0,
    "w": 362.0,
    "h": 72.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "submit_answer_control",
    "x": 24.0,
    "y": 566.0,
    "w": 362.0,
    "h": 72.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "submit_answer_label",
    "x": 138.0,
    "y": 591.0,
    "w": 220.0,
    "h": 30.0,
    "text": "Submit answer",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 20.0,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "skip",
    "x": 24.0,
    "y": 650.0,
    "w": 362.0,
    "h": 60.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "skip_surface",
    "x": 24.0,
    "y": 650.0,
    "w": 362.0,
    "h": 60.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "skip_control",
    "x": 24.0,
    "y": 650.0,
    "w": 362.0,
    "h": 60.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "skip_label",
    "x": 180.0,
    "y": 673.0,
    "w": 80.0,
    "h": 30.0,
    "text": "Skip",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 20.0,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
