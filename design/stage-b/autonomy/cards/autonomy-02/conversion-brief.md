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
    "id": "review_run",
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
    "x": 23.77,
    "y": 19.68,
    "w": 100.86,
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
    "x": 23.83,
    "y": 86.53,
    "w": 114.4,
    "h": 28.5,
    "text": "Code review",
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
    "id": "start_review",
    "x": 262.23,
    "y": 78.3,
    "w": 118.66,
    "h": 38.56,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "start_review_surface",
    "x": 262.23,
    "y": 78.3,
    "w": 118.66,
    "h": 38.56,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "start_review_control",
    "x": 262.23,
    "y": 78.3,
    "w": 118.66,
    "h": 38.56,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "start_review_label",
    "x": 276.23,
    "y": 86.3,
    "w": 90.66,
    "h": 22.56,
    "text": "Start review",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 15.04,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "run_status_card",
    "x": 18.0,
    "y": 157.48,
    "w": 370.0,
    "h": 89.31,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_status",
    "x": 80.18,
    "y": 173.48,
    "w": 223.47,
    "h": 21.68,
    "text": "Reviewing 3 files \u00b7 2 specialists",
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
    "id": "t_status_sub",
    "x": 80.18,
    "y": 210.65,
    "w": 254.18,
    "h": 20.14,
    "text": "Analyzing changes and code quality...",
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
    "id": "finding_high",
    "x": 18.0,
    "y": 300.43,
    "w": 370.0,
    "h": 167.21,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "finding_high_badge",
    "x": 30.94,
    "y": 310.43,
    "w": 55.82,
    "h": 29.68,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "finding_high_badge_label",
    "x": 40.94,
    "y": 314.43,
    "w": 35.82,
    "h": 21.68,
    "text": "High",
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
    "id": "finding_high_path",
    "x": 32.41,
    "y": 359.35,
    "w": 339.47,
    "h": 23.23,
    "text": "crates/octos-cli/src/api/ui_protocol_transport.rs",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 12,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "finding_high_text",
    "x": 32.41,
    "y": 399.62,
    "w": 327.53,
    "h": 50.02,
    "text": "Potential message loss when reconnecting: pending queue was dropped.",
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
    "id": "finding_low",
    "x": 18.0,
    "y": 518.82,
    "w": 370.0,
    "h": 167.19,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "finding_low_badge",
    "x": 27.53,
    "y": 528.82,
    "w": 54.12,
    "h": 25.04,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "finding_low_badge_label",
    "x": 37.53,
    "y": 532.82,
    "w": 34.12,
    "h": 19.5,
    "text": "Low",
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
    "id": "finding_low_path",
    "x": 32.41,
    "y": 579.29,
    "w": 257.59,
    "h": 20.14,
    "text": "crates/octos-core/src/ui_protocol.rs",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 12,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "finding_low_text",
    "x": 32.41,
    "y": 618.01,
    "w": 313.88,
    "h": 50.0,
    "text": "Missing documentation for new preserve_pending behavior.",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 14,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
