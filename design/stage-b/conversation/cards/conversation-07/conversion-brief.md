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
    "id": "edited_files_card",
    "x": 16.0,
    "y": 60.0,
    "w": 374.0,
    "h": 580.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t01",
    "x": 19.97,
    "y": 78.23,
    "w": 129.23,
    "h": 28.51,
    "text": "Edited 3 files",
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
    "id": "t02",
    "x": 16.92,
    "y": 126.33,
    "w": 69.92,
    "h": 23.69,
    "text": "+62 -5",
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
    "id": "icon_undo",
    "x": 231.0,
    "y": 86.0,
    "w": 18.0,
    "h": 20.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_undo",
    "x": 231.19,
    "y": 86.85,
    "w": 64.28,
    "h": 22.65,
    "text": "Undo 9",
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
    "id": "review",
    "x": 316.0,
    "y": 78.0,
    "w": 74.0,
    "h": 40.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "review_surface",
    "x": 316.0,
    "y": 78.0,
    "w": 74.0,
    "h": 40.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "review_control",
    "x": 316.0,
    "y": 78.0,
    "w": 74.0,
    "h": 40.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "review_label",
    "x": 319.0,
    "y": 88.0,
    "w": 80.0,
    "h": 24.0,
    "text": "Review",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 16.0,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "file_1",
    "x": 16.0,
    "y": 201.43,
    "w": 374.0,
    "h": 96.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "file_1_icon",
    "x": 32.0,
    "y": 217.43,
    "w": 20.0,
    "h": 22.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "file_1_dir",
    "x": 73.22,
    "y": 215.43,
    "w": 146.61,
    "h": 19.17,
    "text": "crates/octos-cli/src/",
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
    "id": "file_1_name",
    "x": 72.09,
    "y": 250.4,
    "w": 206.38,
    "h": 27.07,
    "text": "api/ui_protocol_transport.rs",
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
    "id": "file_1_stat",
    "x": 299.99,
    "y": 234.5,
    "w": 66.54,
    "h": 24.0,
    "text": "+31 -4",
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
    "id": "file_2",
    "x": 16.0,
    "y": 331.14,
    "w": 374.0,
    "h": 96.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "file_2_icon",
    "x": 32.0,
    "y": 347.14,
    "w": 20.0,
    "h": 22.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "file_2_dir",
    "x": 73.22,
    "y": 345.14,
    "w": 163.53,
    "h": 19.17,
    "text": "crates/octos-core/src/",
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
    "id": "file_2_name",
    "x": 73.16,
    "y": 379.87,
    "w": 102.74,
    "h": 24.16,
    "text": "ui_protocol.rs",
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
    "id": "file_2_stat",
    "x": 305.63,
    "y": 363.0,
    "w": 60.9,
    "h": 21.62,
    "text": "+9 - 1",
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
    "id": "file_3",
    "x": 16.0,
    "y": 458.59,
    "w": 374.0,
    "h": 96.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "file_3_icon",
    "x": 32.0,
    "y": 474.59,
    "w": 20.0,
    "h": 22.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "file_3_dir",
    "x": 72.09,
    "y": 472.59,
    "w": 161.27,
    "h": 20.3,
    "text": "crates/octos-cli/tests/",
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
    "id": "file_3_name",
    "x": 71.84,
    "y": 510.36,
    "w": 112.06,
    "h": 25.12,
    "text": "steer_queue.rs",
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
    "id": "file_3_stat",
    "x": 290.97,
    "y": 490.5,
    "w": 75.56,
    "h": 25.0,
    "text": "+22 -0",
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
    "id": "icon_show",
    "x": 33.0,
    "y": 605.0,
    "w": 16.0,
    "h": 18.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_show",
    "x": 32.62,
    "y": 605.37,
    "w": 82.5,
    "h": 23.18,
    "text": "Show diff",
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
