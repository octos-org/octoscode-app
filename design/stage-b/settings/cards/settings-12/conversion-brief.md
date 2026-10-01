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
    "h": 132.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "info_i",
    "x": 24.0,
    "y": 40.0,
    "w": 20.0,
    "h": 20.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_ban1",
    "x": 46.09,
    "y": 48.17,
    "w": 196.14,
    "h": 32.11,
    "text": "This session is open in OctoSense [remote].",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 13.5,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_ban2",
    "x": 52.96,
    "y": 90.98,
    "w": 163.77,
    "h": 29.43,
    "text": "You can read along; take over to send.",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 12.5,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "takeover_fill",
    "x": 312.0,
    "y": 56.0,
    "w": 82.0,
    "h": 44.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "takeover_label",
    "x": 325.6,
    "y": 63.9,
    "w": 50.1,
    "h": 32.7,
    "text": "Take over",
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
    "id": "take_over",
    "x": 312.0,
    "y": 56.0,
    "w": 82.0,
    "h": 44.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "take_over_surface",
    "x": 312.0,
    "y": 56.0,
    "w": 82.0,
    "h": 44.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "take_over_control",
    "x": 312.0,
    "y": 56.0,
    "w": 82.0,
    "h": 44.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "take_over_label",
    "x": 0.0,
    "y": 0.0,
    "w": 8.0,
    "h": 15.0,
    "text": "",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 10.0,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "ban_div",
    "x": 0.0,
    "y": 132.0,
    "w": 406.0,
    "h": 1.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_thread1",
    "x": 15.69,
    "y": 184.63,
    "w": 166.71,
    "h": 34.79,
    "text": "Fix steer queue drop on reconnect",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 14.5,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_thread2",
    "x": 15.6,
    "y": 266.24,
    "w": 51.18,
    "h": 29.44,
    "text": "OctosCode",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 12.5,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_b1",
    "x": 16.67,
    "y": 318.43,
    "w": 187.31,
    "h": 34.79,
    "text": "\u2022 I found the root cause in steer/queue.py.",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 13.5,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_b2",
    "x": 18.63,
    "y": 379.97,
    "w": 125.53,
    "h": 32.11,
    "text": "\u2022 Examining steer/queue.py",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 13.5,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_b3",
    "x": 18.63,
    "y": 436.17,
    "w": 187.31,
    "h": 32.11,
    "text": "\u2022 Reconnecting logic added to prevent drops.",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 13.5,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "chip_patch",
    "x": 20.5,
    "y": 510.0,
    "w": 52.0,
    "h": 36.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "chip_patch_t",
    "x": 24.5,
    "y": 513.0,
    "w": 44.0,
    "h": 30.0,
    "text": "Patch",
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
    "id": "t_add12",
    "x": 132.4,
    "y": 519.1,
    "w": 17.7,
    "h": 24.1,
    "text": "+12",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 12.5,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_del4",
    "x": 167.7,
    "y": 519.1,
    "w": 17.7,
    "h": 21.4,
    "text": "-4",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 12.5,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_done",
    "x": 16.67,
    "y": 594.04,
    "w": 73.55,
    "h": 32.11,
    "text": "\u2022 Session done",
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
    "id": "ghost_composer",
    "x": 20.0,
    "y": 660.0,
    "w": 366.0,
    "h": 84.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_readonly",
    "x": 28.44,
    "y": 682.34,
    "w": 182.41,
    "h": 32.11,
    "text": "Read-only while another client has control",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 12.5,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
