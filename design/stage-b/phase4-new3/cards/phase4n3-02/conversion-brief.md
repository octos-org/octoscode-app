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
    "id": "wd_card",
    "x": 32,
    "y": 118,
    "w": 342,
    "h": 56,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "wd_label",
    "x": 44,
    "y": 126,
    "w": 240,
    "h": 16,
    "text": "Server's working directory",
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
    "id": "wd_path",
    "x": 44,
    "y": 146,
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
    "id": "wd_help",
    "x": 252,
    "y": 146,
    "w": 118,
    "h": 16,
    "text": "Start a new session here",
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
    "id": "wd_start_row",
    "x": 44,
    "y": 178,
    "w": 318,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "wd_start_t",
    "x": 44,
    "y": 178,
    "w": 318,
    "h": 17,
    "text": "Start a new session in /home/user/octos",
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
    "id": "wd_start",
    "x": 44,
    "y": 178,
    "w": 318,
    "h": 20,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "h_recent",
    "x": 32,
    "y": 206,
    "w": 240,
    "h": 16,
    "text": "Recent workspaces",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 12,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "rec_0",
    "x": 32,
    "y": 230,
    "w": 342,
    "h": 52,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "rec_0_name",
    "x": 44,
    "y": 238,
    "w": 160,
    "h": 16,
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
    "id": "rec_0_br",
    "x": 210,
    "y": 239,
    "w": 100,
    "h": 14,
    "text": "feat/steer-queue",
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
    "id": "rec_0_path",
    "x": 44,
    "y": 260,
    "w": 190,
    "h": 14,
    "text": "/home/user/octos",
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
    "id": "rec_0_pick_row",
    "x": 240,
    "y": 260,
    "w": 86,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "rec_0_pick_t",
    "x": 240,
    "y": 260,
    "w": 86,
    "h": 17,
    "text": "Start session",
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
    "id": "rec_0_pick",
    "x": 240,
    "y": 260,
    "w": 86,
    "h": 20,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "rec_0_chev",
    "x": 356,
    "y": 246,
    "w": 14,
    "h": 18,
    "text": "\u203a",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 14,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "rec_1",
    "x": 32,
    "y": 288,
    "w": 342,
    "h": 52,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "rec_1_name",
    "x": 44,
    "y": 296,
    "w": 160,
    "h": 16,
    "text": "octoscode-app",
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
    "id": "rec_1_br",
    "x": 210,
    "y": 297,
    "w": 100,
    "h": 14,
    "text": "main",
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
    "id": "rec_1_path",
    "x": 44,
    "y": 318,
    "w": 190,
    "h": 14,
    "text": "/home/user/octoscode-app",
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
    "id": "rec_1_pick_row",
    "x": 240,
    "y": 318,
    "w": 86,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "rec_1_pick_t",
    "x": 240,
    "y": 318,
    "w": 86,
    "h": 17,
    "text": "Start session",
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
    "id": "rec_1_pick",
    "x": 240,
    "y": 318,
    "w": 86,
    "h": 20,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "rec_1_chev",
    "x": 356,
    "y": 304,
    "w": 14,
    "h": 18,
    "text": "\u203a",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 14,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "div1",
    "x": 20,
    "y": 352,
    "w": 366,
    "h": 1,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "f_path_row",
    "x": 32,
    "y": 372,
    "w": 250,
    "h": 40,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "f_path_bg",
    "x": 32,
    "y": 372,
    "w": 250,
    "h": 40,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "f_path_text",
    "x": 44,
    "y": 383.0,
    "w": 226,
    "h": 18,
    "text": "",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 14,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "f_path_ph",
    "x": 44,
    "y": 383.0,
    "w": 226,
    "h": 18,
    "text": "/path/to/project",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 14,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "f_path",
    "x": 32,
    "y": 372,
    "w": 250,
    "h": 40,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "btn_enter_row",
    "x": 296,
    "y": 372,
    "w": 78,
    "h": 40,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "btn_enter_bg",
    "x": 296,
    "y": 372,
    "w": 78,
    "h": 40,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "btn_enter_t",
    "x": 296,
    "y": 383.5,
    "w": 78,
    "h": 17.08,
    "text": "Enter",
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
    "id": "btn_enter",
    "x": 296,
    "y": 372,
    "w": 78,
    "h": 40,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "help_browse",
    "x": 32,
    "y": 424,
    "w": 300,
    "h": 14,
    "text": "Choosing a folder fills the path box.",
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
    "id": "btn_browse_row",
    "x": 32,
    "y": 448,
    "w": 200,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "btn_browse_t",
    "x": 32,
    "y": 448,
    "w": 200,
    "h": 17,
    "text": "Browse server folders\u2026",
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
    "id": "btn_browse",
    "x": 32,
    "y": 448,
    "w": 200,
    "h": 20,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  }
]
```
