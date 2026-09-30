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
    "id": "banner",
    "x": 0.0,
    "y": 0.0,
    "w": 406.0,
    "h": 58.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_banner",
    "x": 46.2,
    "y": 29.3,
    "w": 222.0,
    "h": 25.5,
    "text": "Reconnecting\u2026 attempt 2 \u00b7",
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
    "id": "t_retry",
    "x": 276.5,
    "y": 29.0,
    "w": 77.0,
    "h": 25.0,
    "text": "Retry now",
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
    "id": "skel0",
    "x": 30.0,
    "y": 98.0,
    "w": 70.0,
    "h": 50.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "skel1",
    "x": 100.0,
    "y": 104.0,
    "w": 255.0,
    "h": 44.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "skel2",
    "x": 30.0,
    "y": 180.0,
    "w": 70.0,
    "h": 56.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "skel3",
    "x": 100.0,
    "y": 184.0,
    "w": 255.0,
    "h": 52.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "skel4",
    "x": 30.0,
    "y": 270.0,
    "w": 70.0,
    "h": 49.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "skel5",
    "x": 100.0,
    "y": 275.0,
    "w": 176.0,
    "h": 44.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "divider",
    "x": 24.0,
    "y": 392.0,
    "w": 358.0,
    "h": 2.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "icon_spin",
    "x": 178.0,
    "y": 441.0,
    "w": 30.0,
    "h": 36.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_loading",
    "x": 112.78,
    "y": 510.94,
    "w": 160.14,
    "h": 25.06,
    "text": "Loading session...",
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
    "id": "t_cancel",
    "x": 163.53,
    "y": 571.85,
    "w": 57.52,
    "h": 21.0,
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
