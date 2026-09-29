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
    "id": "t01",
    "x": 15.48,
    "y": 33.74,
    "w": 101.5,
    "h": 24.86,
    "text": "OctosCode",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 16.57,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t02",
    "x": 17.2,
    "y": 102.99,
    "w": 73.97,
    "h": 26.64,
    "text": "Review",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 17.76,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t03",
    "x": 202.96,
    "y": 109.93,
    "w": 86.09,
    "h": 19.88,
    "text": "Last turn -",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 13.25,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t04",
    "x": 324.84,
    "y": 109.05,
    "w": 60.82,
    "h": 21.62,
    "text": "+62 -5",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 14.41,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t05",
    "x": 48.17,
    "y": 195.33,
    "w": 275.25,
    "h": 21.32,
    "text": "crates/octos-cli/src/api/ui_protocol_transport.rs",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 14.21,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t06",
    "x": 335.47,
    "y": 195.33,
    "w": 48.17,
    "h": 17.76,
    "text": "+31-4",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 11.84,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t07",
    "x": 48.17,
    "y": 266.36,
    "w": 213.32,
    "h": 21.32,
    "text": "crates/octos-core/src/ui_protocol.rs",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 14.21,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t08",
    "x": 335.47,
    "y": 266.36,
    "w": 48.17,
    "h": 19.53,
    "text": "+9 -1",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 13.02,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t09",
    "x": 47.9,
    "y": 336.84,
    "w": 217.32,
    "h": 25.01,
    "text": "crates/octos-cli/tests/steer_queue.rs",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 16.67,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t10",
    "x": 328.58,
    "y": 337.39,
    "w": 56.77,
    "h": 19.53,
    "text": "+22 -0",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 13.02,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t11",
    "x": 15.48,
    "y": 424.4,
    "w": 307.94,
    "h": 21.32,
    "text": "crates/octos-cli/src/api/ui_protocol_transport.rs",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 14.21,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t12",
    "x": 15.48,
    "y": 484.78,
    "w": 25.81,
    "h": 17.76,
    "text": "128",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 11.84,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t13",
    "x": 15.48,
    "y": 527.4,
    "w": 25.81,
    "h": 15.98,
    "text": "129",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 10.65,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t14",
    "x": 15.48,
    "y": 566.46,
    "w": 25.81,
    "h": 17.76,
    "text": "130",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 11.84,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t15",
    "x": 15.48,
    "y": 605.53,
    "w": 25.81,
    "h": 17.76,
    "text": "131",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 11.84,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t16",
    "x": 15.48,
    "y": 644.59,
    "w": 25.81,
    "h": 17.76,
    "text": "132",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 11.84,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t17",
    "x": 15.48,
    "y": 685.44,
    "w": 25.81,
    "h": 15.98,
    "text": "133",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 10.65,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t18",
    "x": 15.48,
    "y": 724.5,
    "w": 25.81,
    "h": 17.76,
    "text": "134",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 11.84,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t19",
    "x": 17.2,
    "y": 765.35,
    "w": 24.08,
    "h": 15.0,
    "text": "125",
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
    "id": "t20",
    "x": 82.58,
    "y": 484.78,
    "w": 256.33,
    "h": 21.32,
    "text": "let msg = read_message().await?;",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 14.21,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t21",
    "x": 104.94,
    "y": 527.4,
    "w": 104.94,
    "h": 15.98,
    "text": "connected (",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 10.65,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t22",
    "x": 68.81,
    "y": 571.79,
    "w": 15.48,
    "h": 15.0,
    "text": "-",
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
    "id": "t23",
    "x": 127.31,
    "y": 566.46,
    "w": 172.03,
    "h": 21.32,
    "text": "queue. drop_pending();",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 14.21,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t24",
    "x": 72.25,
    "y": 610.86,
    "w": 12.04,
    "h": 15.0,
    "text": "-",
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
    "id": "t25",
    "x": 127.31,
    "y": 605.53,
    "w": 221.92,
    "h": 23.09,
    "text": "metrics.steer_dropped += 1;",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15.39,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t26",
    "x": 123.86,
    "y": 644.59,
    "w": 209.88,
    "h": 21.32,
    "text": "queue. preserve_pending();",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 14.21,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t27",
    "x": 70.53,
    "y": 685.44,
    "w": 13.76,
    "h": 15.98,
    "text": "+",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 10.65,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t28",
    "x": 70.53,
    "y": 728.05,
    "w": 12.04,
    "h": 15.0,
    "text": "+",
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
    "id": "t29",
    "x": 127.31,
    "y": 683.66,
    "w": 235.69,
    "h": 23.09,
    "text": "metrics.steer_preserved += 1;",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15.39,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t30",
    "x": 128.7,
    "y": 720.96,
    "w": 153.92,
    "h": 25.07,
    "text": "reconnect().await?;",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 16.71,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
