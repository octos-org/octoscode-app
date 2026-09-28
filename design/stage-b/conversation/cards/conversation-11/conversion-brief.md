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
    "id": "t01",
    "x": 33.75,
    "y": 33.6,
    "w": 66.7,
    "h": 26.09,
    "text": "Review",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 17.39,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t02",
    "x": 205.26,
    "y": 38.35,
    "w": 82.33,
    "h": 19.61,
    "text": "Last turn v",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 13.07,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t03",
    "x": 309.01,
    "y": 37.0,
    "w": 64.28,
    "h": 23.38,
    "text": "+62 -5",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15.59,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t04",
    "x": 42.86,
    "y": 131.97,
    "w": 239.09,
    "h": 25.58,
    "text": "ui_protocol_transport.rs +31 -4",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 17.05,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t05",
    "x": 36.09,
    "y": 206.41,
    "w": 25.94,
    "h": 19.48,
    "text": "198",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 12.99,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t06",
    "x": 36.09,
    "y": 256.03,
    "w": 25.94,
    "h": 19.48,
    "text": "199",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 12.99,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t07",
    "x": 36.09,
    "y": 304.53,
    "w": 27.07,
    "h": 19.48,
    "text": "200",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 12.99,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t08",
    "x": 36.09,
    "y": 356.42,
    "w": 25.94,
    "h": 20.71,
    "text": "201",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 13.81,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t09",
    "x": 36.09,
    "y": 407.17,
    "w": 27.07,
    "h": 19.48,
    "text": "202",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 12.99,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t10",
    "x": 36.09,
    "y": 456.8,
    "w": 25.94,
    "h": 19.48,
    "text": "203",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 12.99,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t11",
    "x": 36.09,
    "y": 507.56,
    "w": 27.07,
    "h": 20.71,
    "text": "204",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 13.81,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t12",
    "x": 36.09,
    "y": 557.19,
    "w": 25.94,
    "h": 20.71,
    "text": "205",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 13.81,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t13",
    "x": 86.84,
    "y": 207.5,
    "w": 249.24,
    "h": 23.22,
    "text": "fn handle_disconnect(&mut self) {",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15.48,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t14",
    "x": 75.56,
    "y": 254.91,
    "w": 197.36,
    "h": 24.41,
    "text": "- self.queue.clear();",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 16.27,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t15",
    "x": 78.94,
    "y": 303.41,
    "w": 295.48,
    "h": 24.41,
    "text": "- self.state = State::Disconnected;",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 16.27,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t16",
    "x": 81.2,
    "y": 355.29,
    "w": 210.89,
    "h": 28.02,
    "text": "+ self.persist_queue()?;",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 18.68,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t17",
    "x": 81.2,
    "y": 404.92,
    "w": 241.34,
    "h": 26.8,
    "text": "+ self.queue.mark_pending();",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 17.87,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t18",
    "x": 81.2,
    "y": 454.5,
    "w": 267.28,
    "h": 27.0,
    "text": "+ self.metrics.reconnects += 1;",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 18.0,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t19",
    "x": 80.01,
    "y": 506.36,
    "w": 294.51,
    "h": 26.91,
    "text": "+ self.state = State::Disconnected;",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 17.94,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t20",
    "x": 100.37,
    "y": 620.35,
    "w": 182.7,
    "h": 21.93,
    "text": ":412 unmodified lines",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 14.62,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
