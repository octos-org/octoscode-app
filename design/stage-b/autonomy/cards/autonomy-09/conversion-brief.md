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
    "id": "att_1",
    "x": 16.0,
    "y": 190.0,
    "w": 178.0,
    "h": 240.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "att_1_img",
    "x": 93.0,
    "y": 280.0,
    "w": 24.0,
    "h": 24.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "att_1_close_bg",
    "x": 162.0,
    "y": 196.0,
    "w": 24.0,
    "h": 24.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "att_1_close",
    "x": 167.0,
    "y": 201.0,
    "w": 14.0,
    "h": 14.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "att_2",
    "x": 212.0,
    "y": 190.0,
    "w": 178.0,
    "h": 240.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "att_2_img",
    "x": 289.0,
    "y": 280.0,
    "w": 24.0,
    "h": 24.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "att_2_close_bg",
    "x": 358.0,
    "y": 196.0,
    "w": 24.0,
    "h": 24.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "att_2_close",
    "x": 363.0,
    "y": 201.0,
    "w": 14.0,
    "h": 14.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "att2_ring",
    "x": 274.0,
    "y": 282.0,
    "w": 52.0,
    "h": 52.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "att2_pct",
    "x": 274.0,
    "y": 296.0,
    "w": 52.0,
    "h": 24.0,
    "text": "68%",
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
    "id": "t_sz1",
    "x": 13.76,
    "y": 435.53,
    "w": 56.77,
    "h": 25.92,
    "text": "1.2 MB",
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
    "id": "t_sz2",
    "x": 208.16,
    "y": 435.53,
    "w": 56.77,
    "h": 24.2,
    "text": "1.2 MB",
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
    "id": "t_caption",
    "x": 15.48,
    "y": 501.2,
    "w": 196.12,
    "h": 24.2,
    "text": "2 of 4 images \u2022 20 MB max",
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
    "x": 28.0,
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
    "x": 26.0,
    "y": 726.0,
    "w": 18.0,
    "h": 24.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "approval_pill",
    "x": 62.0,
    "y": 738.0,
    "w": 125.22,
    "h": 34.47,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_approval",
    "x": 72.25,
    "y": 743.16,
    "w": 103.22,
    "h": 22.47,
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
    "x": 204.53,
    "y": 744.03,
    "w": 72.64,
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
    "id": "icon_mic",
    "x": 306.0,
    "y": 726.0,
    "w": 16.0,
    "h": 26.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "send_btn",
    "x": 344.0,
    "y": 724.0,
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
    "y": 734.0,
    "w": 16.0,
    "h": 16.0,
    "role": "unknown",
    "native_candidates": []
  }
]
```
