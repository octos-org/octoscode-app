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
    "id": "review_panel",
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
    "id": "t_title",
    "x": 20.45,
    "y": 80.48,
    "w": 73.4,
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
    "x": 196.55,
    "y": 78.08,
    "w": 117.9,
    "h": 34.36,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "scope_label",
    "x": 204.55,
    "y": 86.08,
    "w": 69.9,
    "h": 21.0,
    "text": "Last turn \u25be",
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
    "x": 290.45,
    "y": 88.08,
    "w": 14.0,
    "h": 16.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_add",
    "x": 325.82,
    "y": 85.19,
    "w": 34.0,
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
    "x": 365.82,
    "y": 85.19,
    "w": 24.0,
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
    "id": "file_1",
    "x": 16.0,
    "y": 146.89,
    "w": 374.0,
    "h": 40.78,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "file_1_icon",
    "x": 23.89,
    "y": 155.89,
    "w": 16.0,
    "h": 18.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "file_1_path",
    "x": 53.0,
    "y": 154.89,
    "w": 281.0,
    "h": 24.78,
    "text": "crates/octos-cli/src/api/ui_protocol_transport.rs",
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
    "id": "file_1_add",
    "x": 338.0,
    "y": 154.89,
    "w": 30.0,
    "h": 24.78,
    "text": "+31",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 12,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "file_1_del",
    "x": 371.0,
    "y": 154.89,
    "w": 17.0,
    "h": 24.78,
    "text": "-4",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 12,
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
    "y": 210.4,
    "w": 374.0,
    "h": 40.78,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "file_2_icon",
    "x": 23.89,
    "y": 219.4,
    "w": 16.0,
    "h": 18.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "file_2_path",
    "x": 53.0,
    "y": 218.4,
    "w": 280.0,
    "h": 24.78,
    "text": "crates/octos-core/src/ui_protocol.rs",
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
    "id": "file_2_add",
    "x": 333.33,
    "y": 219.97,
    "w": 30.0,
    "h": 21.08,
    "text": "+9",
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
    "id": "file_2_del",
    "x": 367.33,
    "y": 219.97,
    "w": 24.0,
    "h": 21.08,
    "text": "-1",
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
    "id": "file_3",
    "x": 16.0,
    "y": 278.55,
    "w": 374.0,
    "h": 34.59,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "file_3_icon",
    "x": 23.89,
    "y": 287.55,
    "w": 16.0,
    "h": 18.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "file_3_path",
    "x": 53.0,
    "y": 286.55,
    "w": 280.0,
    "h": 18.59,
    "text": "crates/octos-cli/tests/steer_queue.rs",
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
    "id": "file_3_add",
    "x": 329.16,
    "y": 283.27,
    "w": 30.0,
    "h": 19.5,
    "text": "+22",
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
    "id": "file_3_del",
    "x": 363.16,
    "y": 283.27,
    "w": 24.0,
    "h": 19.5,
    "text": "-0",
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
    "id": "diff_band_rule_top",
    "x": 0.0,
    "y": 334.1,
    "w": 406.0,
    "h": 1.2,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "diff_file_header",
    "x": 0.0,
    "y": 335.4,
    "w": 406.0,
    "h": 54.9,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "diff_file_icon",
    "x": 24.76,
    "y": 360.35,
    "w": 16.0,
    "h": 18.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "diff_file_path",
    "x": 44.76,
    "y": 359.35,
    "w": 305.35,
    "h": 21.68,
    "text": "crates/octos-cli/src/api/ui_protocol_transport.rs",
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
    "id": "diff_band_rule_bottom",
    "x": 0.0,
    "y": 390.3,
    "w": 406.0,
    "h": 1.2,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "diff_rows",
    "x": 20.0,
    "y": 403.56,
    "w": 366.0,
    "h": 299.68,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "ln_0",
    "x": 18.76,
    "y": 413.56,
    "w": 22.0,
    "h": 30.96,
    "text": "128",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 13,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "dl_0",
    "x": 84.76,
    "y": 413.56,
    "w": 280.0,
    "h": 30.96,
    "text": "let msg = read_message().await?;",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 13,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "ln_1",
    "x": 18.76,
    "y": 448.52,
    "w": 22.0,
    "h": 30.96,
    "text": "129",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 13,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "dl_1",
    "x": 84.76,
    "y": 448.52,
    "w": 280.0,
    "h": 30.96,
    "text": "if !connected {",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 13,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "chip_130",
    "x": 178.3,
    "y": 480.48,
    "w": 99.0,
    "h": 32.96,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "ln_2",
    "x": 18.76,
    "y": 483.48,
    "w": 22.0,
    "h": 30.96,
    "text": "130",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 13,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "mk_2",
    "x": 75.76,
    "y": 483.48,
    "w": 12.0,
    "h": 30.96,
    "text": "-",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 13,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "dl_2",
    "x": 84.76,
    "y": 483.48,
    "w": 280.0,
    "h": 30.96,
    "text": "queue.drop_pending();",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 13,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "chip_131",
    "x": 244.0,
    "y": 515.44,
    "w": 55.5,
    "h": 32.96,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "ln_3",
    "x": 18.76,
    "y": 518.44,
    "w": 22.0,
    "h": 30.96,
    "text": "131",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 13,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "mk_3",
    "x": 75.76,
    "y": 518.44,
    "w": 12.0,
    "h": 30.96,
    "text": "-",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 13,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "dl_3",
    "x": 84.76,
    "y": 518.44,
    "w": 280.0,
    "h": 30.96,
    "text": "metrics.steer_dropped += 1;",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 13,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "chip_132",
    "x": 178.3,
    "y": 550.4,
    "w": 133.1,
    "h": 32.96,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "ln_4",
    "x": 18.76,
    "y": 553.4,
    "w": 22.0,
    "h": 30.96,
    "text": "132",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 13,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "mk_4",
    "x": 75.76,
    "y": 553.4,
    "w": 12.0,
    "h": 30.96,
    "text": "+",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 13,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "dl_4",
    "x": 84.76,
    "y": 553.4,
    "w": 280.0,
    "h": 30.96,
    "text": "queue.preserve_pending();",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 13,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "ln_5",
    "x": 18.76,
    "y": 588.36,
    "w": 22.0,
    "h": 30.96,
    "text": "133",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 13,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "mk_5",
    "x": 75.76,
    "y": 588.36,
    "w": 12.0,
    "h": 30.96,
    "text": "+",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 13,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "dl_5",
    "x": 84.76,
    "y": 588.36,
    "w": 280.0,
    "h": 30.96,
    "text": "metrics.steer_preserved += 1;",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 13,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "ln_6",
    "x": 18.76,
    "y": 623.32,
    "w": 22.0,
    "h": 30.96,
    "text": "134",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 13,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "mk_6",
    "x": 75.76,
    "y": 623.32,
    "w": 12.0,
    "h": 30.96,
    "text": "+",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 13,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "dl_6",
    "x": 84.76,
    "y": 623.32,
    "w": 280.0,
    "h": 30.96,
    "text": "reconnect().await?;",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 13,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "ln_7",
    "x": 18.76,
    "y": 658.28,
    "w": 22.0,
    "h": 30.96,
    "text": "135",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 13,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "dl_7",
    "x": 84.76,
    "y": 658.28,
    "w": 280.0,
    "h": 30.96,
    "text": "}",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 13,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "folded",
    "x": 10.76,
    "y": 712.24,
    "w": 366.0,
    "h": 34.59,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_fold",
    "x": 88.71,
    "y": 720.24,
    "w": 197.88,
    "h": 19.5,
    "text": "\u22ee 412 unmodified lines \u22ee",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 13,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
