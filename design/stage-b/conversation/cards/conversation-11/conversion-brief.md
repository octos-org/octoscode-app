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
    "id": "diff_view",
    "x": 20.0,
    "y": 92.0,
    "w": 362.0,
    "h": 566.0,
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
    "h": 28.5,
    "text": "Review",
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
    "id": "scope_pill",
    "x": 196.0,
    "y": 30.0,
    "w": 96.0,
    "h": 34.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "scope_label",
    "x": 205.0,
    "y": 38.0,
    "w": 74.0,
    "h": 22.0,
    "text": "Last turn v",
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
    "id": "scope_chev",
    "x": 266.0,
    "y": 40.0,
    "w": 16.0,
    "h": 16.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_add",
    "x": 309.01,
    "y": 37.0,
    "w": 46.0,
    "h": 22.5,
    "text": "+62",
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
    "id": "t_del",
    "x": 357.01,
    "y": 37.0,
    "w": 40.0,
    "h": 22.5,
    "text": "-5",
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
    "id": "file_header",
    "x": 20.0,
    "y": 121.97,
    "w": 366.0,
    "h": 42.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_file",
    "x": 42.86,
    "y": 131.97,
    "w": 220.0,
    "h": 23.69,
    "text": "ui_protocol_transport.rs",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 14,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_fadd",
    "x": 268.0,
    "y": 131.97,
    "w": 40.0,
    "h": 23.69,
    "text": "+31",
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
    "id": "t_fdel",
    "x": 312.0,
    "y": 131.97,
    "w": 30.0,
    "h": 23.69,
    "text": "-4",
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
    "id": "diff_rows",
    "x": 20.0,
    "y": 180.0,
    "w": 366.0,
    "h": 420.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "ln_0",
    "x": 30.0,
    "y": 199.0,
    "w": 30.0,
    "h": 22.0,
    "text": "198",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 14.67,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "dl_0",
    "x": 72.0,
    "y": 199.0,
    "w": 320.0,
    "h": 24.0,
    "text": "fn handle_disconnect(&mut self) {",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 16.0,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "row_1",
    "x": 24.0,
    "y": 241.0,
    "w": 358.0,
    "h": 44.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "ln_1",
    "x": 30.0,
    "y": 249.0,
    "w": 30.0,
    "h": 22.0,
    "text": "199",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 14.67,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "dl_1",
    "x": 72.0,
    "y": 249.0,
    "w": 320.0,
    "h": 24.0,
    "text": "- self.queue.clear();",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 16.0,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "row_2",
    "x": 24.0,
    "y": 291.0,
    "w": 358.0,
    "h": 44.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "ln_2",
    "x": 30.0,
    "y": 299.0,
    "w": 30.0,
    "h": 22.0,
    "text": "200",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 14.67,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "dl_2",
    "x": 72.0,
    "y": 299.0,
    "w": 320.0,
    "h": 24.0,
    "text": "- self.state = State::Disconnected;",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 16.0,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "row_3",
    "x": 24.0,
    "y": 341.0,
    "w": 358.0,
    "h": 44.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "ln_3",
    "x": 30.0,
    "y": 349.0,
    "w": 30.0,
    "h": 22.0,
    "text": "201",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 14.67,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "dl_3",
    "x": 72.0,
    "y": 349.0,
    "w": 320.0,
    "h": 24.0,
    "text": "+ self.persist_queue()?;",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 16.0,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "row_4",
    "x": 24.0,
    "y": 391.0,
    "w": 358.0,
    "h": 44.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "ln_4",
    "x": 30.0,
    "y": 399.0,
    "w": 30.0,
    "h": 22.0,
    "text": "202",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 14.67,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "dl_4",
    "x": 72.0,
    "y": 399.0,
    "w": 320.0,
    "h": 24.0,
    "text": "+ self.queue.mark_pending();",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 16.0,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "row_5",
    "x": 24.0,
    "y": 441.0,
    "w": 358.0,
    "h": 44.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "ln_5",
    "x": 30.0,
    "y": 449.0,
    "w": 30.0,
    "h": 22.0,
    "text": "203",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 14.67,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "dl_5",
    "x": 72.0,
    "y": 449.0,
    "w": 320.0,
    "h": 24.0,
    "text": "+ self.metrics.reconnects += 1;",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 16.0,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "row_6",
    "x": 24.0,
    "y": 491.0,
    "w": 358.0,
    "h": 44.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "ln_6",
    "x": 30.0,
    "y": 499.0,
    "w": 30.0,
    "h": 22.0,
    "text": "204",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 14.67,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "dl_6",
    "x": 72.0,
    "y": 499.0,
    "w": 320.0,
    "h": 24.0,
    "text": "+ self.state = State::Disconnected;",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 16.0,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "ln_7",
    "x": 30.0,
    "y": 549.0,
    "w": 30.0,
    "h": 22.0,
    "text": "205",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 14.67,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "dl_7",
    "x": 72.0,
    "y": 549.0,
    "w": 320.0,
    "h": 24.0,
    "text": "",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 16.0,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "folded",
    "x": 24.0,
    "y": 612.35,
    "w": 358.0,
    "h": 34.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_fold",
    "x": 100.37,
    "y": 620.35,
    "w": 182.7,
    "h": 20.3,
    "text": ":412 unmodified lines",
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
