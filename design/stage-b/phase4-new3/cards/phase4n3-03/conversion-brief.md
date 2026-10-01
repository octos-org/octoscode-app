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
    "id": "scrim_64",
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
    "id": "modal_card",
    "x": 16,
    "y": 64,
    "w": 374,
    "h": 640,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_title",
    "x": 32,
    "y": 82,
    "w": 240,
    "h": 24.4,
    "text": "Add workspace",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 20,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "btn_back_row",
    "x": 32,
    "y": 112,
    "w": 200,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "btn_back_t",
    "x": 32,
    "y": 112,
    "w": 200,
    "h": 17,
    "text": "Back to workspaces",
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
    "id": "btn_back",
    "x": 32,
    "y": 112,
    "w": 200,
    "h": 20,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "btn_close_row",
    "x": 330,
    "y": 82,
    "w": 24,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "btn_close_t",
    "x": 330,
    "y": 82,
    "w": 24,
    "h": 18.3,
    "text": "\u2715",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 15,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "btn_close",
    "x": 330,
    "y": 82,
    "w": 24,
    "h": 20,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "f_folder_row",
    "x": 32,
    "y": 150,
    "w": 250,
    "h": 40,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "f_folder_bg",
    "x": 32,
    "y": 150,
    "w": 250,
    "h": 40,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "f_folder_text",
    "x": 44,
    "y": 161.0,
    "w": 226,
    "h": 18,
    "text": "notes",
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
    "id": "f_folder",
    "x": 32,
    "y": 150,
    "w": 250,
    "h": 40,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "btn_create_row",
    "x": 296,
    "y": 150,
    "w": 78,
    "h": 40,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "btn_create_bg",
    "x": 296,
    "y": 150,
    "w": 78,
    "h": 40,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "btn_create_t",
    "x": 296,
    "y": 161.5,
    "w": 78,
    "h": 17.08,
    "text": "Create",
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
    "id": "btn_create",
    "x": 296,
    "y": 150,
    "w": 78,
    "h": 40,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "val_name",
    "x": 32,
    "y": 200,
    "w": 300,
    "h": 14,
    "text": "",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 11,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "parent_label",
    "x": 32,
    "y": 232,
    "w": 120,
    "h": 14.64,
    "text": "Parent",
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
    "id": "parent_path",
    "x": 32,
    "y": 250,
    "w": 200,
    "h": 16,
    "text": "/home/user/octos",
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
    "id": "btn_drill_row",
    "x": 32,
    "y": 286,
    "w": 200,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "btn_drill_t",
    "x": 32,
    "y": 286,
    "w": 200,
    "h": 17,
    "text": "Open folder browser",
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
    "id": "btn_drill",
    "x": 32,
    "y": 286,
    "w": 200,
    "h": 20,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "btn_drill_disabled_row",
    "x": 32,
    "y": 312,
    "w": 260,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "btn_drill_disabled_t",
    "x": 32,
    "y": 312,
    "w": 260,
    "h": 17,
    "text": "Open folder browser (unavailable)",
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
    "id": "btn_drill_disabled",
    "x": 32,
    "y": 312,
    "w": 260,
    "h": 20,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "disc_1",
    "x": 32,
    "y": 330,
    "w": 342,
    "h": 44,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "disc_t",
    "x": 44,
    "y": 342,
    "w": 200,
    "h": 16,
    "text": "\u00b7 3 existing folders",
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
    "id": "disc_l",
    "x": 44,
    "y": 360,
    "w": 240,
    "h": 14,
    "text": "notes \u00b7 drafts \u00b7 tmp",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 11,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "note_browser",
    "x": 32,
    "y": 392,
    "w": 342,
    "h": 28,
    "text": "The folder browser is available only when the server advertises it.",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 11,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
