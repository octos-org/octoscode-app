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
    "id": "tasks_card",
    "x": 12.0,
    "y": 108.0,
    "w": 372.0,
    "h": 648.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t01",
    "x": 32.87,
    "y": 131.75,
    "w": 96.31,
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
    "id": "t02",
    "x": 31.36,
    "y": 209.15,
    "w": 57.5,
    "h": 27.37,
    "text": "Tasks",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 17,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "run_card",
    "x": 24.0,
    "y": 264.0,
    "w": 320.0,
    "h": 340.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_cmd",
    "x": 34.85,
    "y": 297.11,
    "w": 231.75,
    "h": 25.41,
    "text": "\u203a cargo test -p octos-cli steer_queue",
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
    "id": "run_pill",
    "x": 276.28,
    "y": 295.06,
    "w": 59.05,
    "h": 29.5,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_run",
    "x": 282.28,
    "y": 299.06,
    "w": 47.05,
    "h": 21.5,
    "text": "Running",
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
    "id": "t_run_dur",
    "x": 341.33,
    "y": 299.06,
    "w": 24.0,
    "h": 21.5,
    "text": "2m",
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
    "id": "console",
    "x": 32.0,
    "y": 350.0,
    "w": 328.0,
    "h": 190.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_log0",
    "x": 41.82,
    "y": 381.16,
    "w": 322.36,
    "h": 21.5,
    "text": "Compiling octos-cli v0.24.1 (/workspace/crates/octos-cli)",
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
    "id": "t_log1",
    "x": 43.56,
    "y": 422.21,
    "w": 322.36,
    "h": 21.5,
    "text": "Finished test [unoptimized + debuginfo] target(s) in 1.23s",
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
    "id": "t_log2",
    "x": 41.82,
    "y": 463.25,
    "w": 324.1,
    "h": 23.46,
    "text": "Running unittests src/lib.rs (target/debug/deps/octos_cli\u2026)",
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
    "id": "t_log3",
    "x": 43.48,
    "y": 501.81,
    "w": 118.65,
    "h": 24.53,
    "text": "running 12 tests \u2026",
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
    "id": "cancel_row",
    "x": 40.0,
    "y": 552.0,
    "w": 288.0,
    "h": 44.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "cancel_surface",
    "x": 40.0,
    "y": 552.0,
    "w": 288.0,
    "h": 44.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "cancel_control",
    "x": 40,
    "y": 552,
    "w": 288,
    "h": 44,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "cancel_label",
    "x": 125.46,
    "y": 590.31,
    "w": 48.79,
    "h": 23.46,
    "text": "Cancel",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 15.64,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_done_cmd",
    "x": 34.85,
    "y": 705.63,
    "w": 179.48,
    "h": 23.46,
    "text": "\u203a cargo clippy -p octos-cli",
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
    "id": "done_pill",
    "x": 274.44,
    "y": 701.42,
    "w": 45.3,
    "h": 27.96,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_done",
    "x": 280.44,
    "y": 705.42,
    "w": 33.3,
    "h": 19.96,
    "text": "Done",
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
    "id": "t_dur",
    "x": 346.76,
    "y": 705.63,
    "w": 20.91,
    "h": 19.55,
    "text": "1m",
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
