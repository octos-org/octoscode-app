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
    "id": "outer_card",
    "x": 10.0,
    "y": 10.0,
    "w": 386.0,
    "h": 756.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_title",
    "x": 78.94,
    "y": 80.0,
    "w": 227.81,
    "h": 30.0,
    "text": "Open a workspace",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 26,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "folder",
    "x": 46.0,
    "y": 136.85,
    "w": 314.0,
    "h": 72.93,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "folder_icon",
    "x": 62.0,
    "y": 148.6,
    "w": 22.0,
    "h": 22.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_sfolder",
    "x": 92.0,
    "y": 150.85,
    "w": 200.0,
    "h": 21.0,
    "text": "Server folder",
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
    "id": "folder_field",
    "x": 92.0,
    "y": 178.94,
    "w": 200.0,
    "h": 22.5,
    "text": "~/home/octos",
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
    "id": "folder_chev",
    "x": 334.0,
    "y": 164.32,
    "w": 18.0,
    "h": 18.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_recent",
    "x": 30.45,
    "y": 243.63,
    "w": 56.39,
    "h": 22.5,
    "text": "Recent",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 15,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "recent_group",
    "x": 46.0,
    "y": 277.0,
    "w": 314.0,
    "h": 290.8,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "ws_icon0",
    "x": 60.0,
    "y": 288.9,
    "w": 20.0,
    "h": 20.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_ws0_name",
    "x": 94.73,
    "y": 291.0,
    "w": 45.11,
    "h": 21.0,
    "text": "octos",
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
    "id": "t_ws0_path",
    "x": 94.73,
    "y": 314.05,
    "w": 240.0,
    "h": 19.31,
    "text": "~/home/octos",
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
    "id": "hair_ws1",
    "x": 58.0,
    "y": 347.43,
    "w": 290.0,
    "h": 1.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "ws_icon1",
    "x": 60.0,
    "y": 362.21,
    "w": 20.0,
    "h": 20.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_ws1_name",
    "x": 94.64,
    "y": 361.49,
    "w": 118.6,
    "h": 21.43,
    "text": "octoscode-app",
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
    "id": "t_ws1_path",
    "x": 94.64,
    "y": 388.0,
    "w": 240.0,
    "h": 19.17,
    "text": "~/home/octos/octoscode-app",
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
    "id": "hair_ws2",
    "x": 58.0,
    "y": 421.27,
    "w": 290.0,
    "h": 1.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "ws_icon2",
    "x": 60.0,
    "y": 434.4,
    "w": 20.0,
    "h": 20.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_ws2_name",
    "x": 93.61,
    "y": 435.37,
    "w": 58.64,
    "h": 21.0,
    "text": "robrix2",
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
    "id": "t_ws2_path",
    "x": 93.61,
    "y": 461.31,
    "w": 240.0,
    "h": 18.05,
    "text": "~/home/octos/robrix2",
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
    "id": "hair_ws3",
    "x": 58.0,
    "y": 494.35,
    "w": 290.0,
    "h": 1.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "ws_icon3",
    "x": 60.0,
    "y": 508.27,
    "w": 20.0,
    "h": 20.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_ws3_name",
    "x": 94.64,
    "y": 509.35,
    "w": 85.89,
    "h": 21.0,
    "text": "octos-web",
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
    "id": "t_ws3_path",
    "x": 94.64,
    "y": 535.76,
    "w": 240.0,
    "h": 18.05,
    "text": "~/home/octos/octos-web",
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
    "id": "browse",
    "x": 46.0,
    "y": 605.84,
    "w": 314.0,
    "h": 44.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "browse_surface",
    "x": 46.0,
    "y": 605.84,
    "w": 314.0,
    "h": 44.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "browse_control",
    "x": 46.0,
    "y": 605.84,
    "w": 314.0,
    "h": 44.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "browse_label",
    "x": 134.21,
    "y": 617.69,
    "w": 137.59,
    "h": 20.3,
    "text": "Browse folders\u2026",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 16,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_newfolder",
    "x": 142.1,
    "y": 676.5,
    "w": 92.48,
    "h": 21.0,
    "text": "New folder",
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
