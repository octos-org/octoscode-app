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
    "id": "user_bubble",
    "x": 90.88,
    "y": 48.91,
    "w": 284.01,
    "h": 85.09,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t01",
    "x": 104.88,
    "y": 60.91,
    "w": 256.01,
    "h": 27.09,
    "text": "Fix the steer queue so queued",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 18.06,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t02",
    "x": 104.88,
    "y": 100.38,
    "w": 226.68,
    "h": 21.62,
    "text": "steers survive a reconnect",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 14.41,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "icon_spinner",
    "x": 44.0,
    "y": 198.0,
    "w": 18.0,
    "h": 18.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t03",
    "x": 64.28,
    "y": 195.0,
    "w": 122.93,
    "h": 26.07,
    "text": "Working \u2022 12s",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 17.38,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "assistant_md",
    "x": 21.0,
    "y": 248.0,
    "w": 358.0,
    "h": 176.0,
    "text": "I'm tracing how queued steers are handled across reconnects\u2026\n\nI'll run tests to confirm the fix and update the affected `steer_dropped` path\u2026",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 17.5,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "chips",
    "x": 24.0,
    "y": 501.62,
    "w": 358.0,
    "h": 35.56,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "chip_ws",
    "x": 29.13,
    "y": 503.62,
    "w": 60.15,
    "h": 31.56,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t08",
    "x": 37.13,
    "y": 509.62,
    "w": 44.15,
    "h": 19.56,
    "text": "octos",
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
    "id": "chip_mode",
    "x": 121.69,
    "y": 502.69,
    "w": 59.98,
    "h": 32.3,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t09",
    "x": 129.69,
    "y": 508.69,
    "w": 43.98,
    "h": 20.3,
    "text": "Local",
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
    "id": "chip_branch",
    "x": 220.75,
    "y": 503.34,
    "w": 138.21,
    "h": 32.37,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t10",
    "x": 228.75,
    "y": 509.34,
    "w": 122.21,
    "h": 20.37,
    "text": "feat/steer-queue",
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
    "id": "composer",
    "x": 16.0,
    "y": 584.0,
    "w": 374.0,
    "h": 150.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "composer_input",
    "x": 30.0,
    "y": 594.0,
    "w": 300.0,
    "h": 40.0,
    "text": "",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15,
    "weight": 400,
    "role": "input",
    "native_candidates": [
      "TextInput",
      "KitFormField"
    ]
  },
  {
    "id": "icon_plus",
    "x": 28.0,
    "y": 685.0,
    "w": 18.0,
    "h": 24.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "approval_pill",
    "x": 63.25,
    "y": 681.76,
    "w": 122.73,
    "h": 33.83,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t13",
    "x": 73.25,
    "y": 687.76,
    "w": 102.73,
    "h": 20.83,
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
    "id": "icon_mic",
    "x": 309.0,
    "y": 681.0,
    "w": 17.0,
    "h": 27.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t14",
    "x": 215.36,
    "y": 691.24,
    "w": 73.39,
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
    "id": "stop_btn",
    "x": 346.0,
    "y": 674.0,
    "w": 36.0,
    "h": 36.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "icon_stop",
    "x": 356.0,
    "y": 684.0,
    "w": 16.0,
    "h": 16.0,
    "role": "unknown",
    "native_candidates": []
  }
]
```
