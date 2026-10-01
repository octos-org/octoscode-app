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
    "h": 648,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "h_slash",
    "x": 32,
    "y": 84,
    "w": 90,
    "h": 17,
    "text": "/thread",
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
    "id": "t_title",
    "x": 32,
    "y": 108,
    "w": 200,
    "h": 24.4,
    "text": "Thread",
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
    "id": "t_scope",
    "x": 32,
    "y": 138,
    "w": 260,
    "h": 15,
    "text": "/home/user/octos \u00b7 main",
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
    "id": "inspector_refresh_row",
    "x": 282,
    "y": 86,
    "w": 92,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "inspector_refresh_t",
    "x": 282,
    "y": 86,
    "w": 92,
    "h": 17,
    "text": "\u21bb Refresh",
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
    "id": "inspector_refresh",
    "x": 282,
    "y": 86,
    "w": 92,
    "h": 20,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "d0",
    "x": 20,
    "y": 166,
    "w": 366,
    "h": 1,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "h_graph",
    "x": 32,
    "y": 180,
    "w": 140,
    "h": 13.42,
    "text": "THREAD GRAPH",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 11,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "node_0",
    "x": 32,
    "y": 200,
    "w": 342,
    "h": 44,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "node_0_role",
    "x": 44,
    "y": 206,
    "w": 70,
    "h": 14,
    "text": "current",
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
    "id": "node_0_name",
    "x": 44,
    "y": 224,
    "w": 250,
    "h": 15,
    "text": "Fix steer queue drop on reconnect",
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
    "id": "node_0_depth",
    "x": 290,
    "y": 214,
    "w": 72,
    "h": 14,
    "text": "depth 0",
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
    "id": "node_1",
    "x": 32,
    "y": 250,
    "w": 342,
    "h": 44,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "node_1_role",
    "x": 44,
    "y": 256,
    "w": 70,
    "h": 14,
    "text": "parent",
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
    "id": "node_1_name",
    "x": 44,
    "y": 274,
    "w": 250,
    "h": 15,
    "text": "Add session fork",
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
    "id": "node_1_depth",
    "x": 290,
    "y": 264,
    "w": 72,
    "h": 14,
    "text": "depth 1",
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
    "id": "node_2",
    "x": 32,
    "y": 300,
    "w": 342,
    "h": 44,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "node_2_role",
    "x": 44,
    "y": 306,
    "w": 70,
    "h": 14,
    "text": "parent",
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
    "id": "node_2_name",
    "x": 44,
    "y": 324,
    "w": 250,
    "h": 15,
    "text": "Review PR #2566",
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
    "id": "node_2_depth",
    "x": 290,
    "y": 314,
    "w": 72,
    "h": 14,
    "text": "depth 1",
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
    "id": "node_3",
    "x": 32,
    "y": 350,
    "w": 342,
    "h": 44,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "node_3_role",
    "x": 44,
    "y": 356,
    "w": 70,
    "h": 14,
    "text": "parent",
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
    "id": "node_3_name",
    "x": 44,
    "y": 374,
    "w": 250,
    "h": 15,
    "text": "Bump octos-core to a6ea8505",
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
    "id": "node_3_depth",
    "x": 290,
    "y": 364,
    "w": 72,
    "h": 14,
    "text": "depth 2",
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
    "id": "d1",
    "x": 20,
    "y": 404,
    "w": 366,
    "h": 1,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "h_scopes",
    "x": 32,
    "y": 416,
    "w": 160,
    "h": 13.42,
    "text": "APPROVAL SCOPES",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 11,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "scope_0",
    "x": 32,
    "y": 436,
    "w": 342,
    "h": 40,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "scope_0_t",
    "x": 44,
    "y": 447,
    "w": 160,
    "h": 16,
    "text": "Session",
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
    "id": "scope_0_tag",
    "x": 316,
    "y": 446,
    "w": 59.4,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "scope_0_tag_t",
    "x": 324,
    "y": 449,
    "w": 43.4,
    "h": 14,
    "text": "allowed",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 10,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "scope_1",
    "x": 32,
    "y": 482,
    "w": 342,
    "h": 40,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "scope_1_t",
    "x": 44,
    "y": 493,
    "w": 160,
    "h": 16,
    "text": "Workspace",
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
    "id": "scope_1_tag",
    "x": 316,
    "y": 492,
    "w": 59.4,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "scope_1_tag_t",
    "x": 324,
    "y": 495,
    "w": 43.4,
    "h": 14,
    "text": "allowed",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 10,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "d2",
    "x": 20,
    "y": 532,
    "w": 366,
    "h": 1,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "inspector_copy_link_row",
    "x": 32,
    "y": 546,
    "w": 120,
    "h": 36,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "inspector_copy_link_bg",
    "x": 32,
    "y": 546,
    "w": 120,
    "h": 36,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "inspector_copy_link_t",
    "x": 32,
    "y": 555.5,
    "w": 120,
    "h": 17.08,
    "text": "Copy link",
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
    "id": "inspector_copy_link",
    "x": 32,
    "y": 546,
    "w": 120,
    "h": 36,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "t_link",
    "x": 32,
    "y": 598,
    "w": 300,
    "h": 15,
    "text": "octos://session/dsflash:main",
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
    "id": "inspector_refresh_bottom_row",
    "x": 264,
    "y": 640,
    "w": 110,
    "h": 36,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "inspector_refresh_bottom_bg",
    "x": 264,
    "y": 640,
    "w": 110,
    "h": 36,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "inspector_refresh_bottom_t",
    "x": 264,
    "y": 649.5,
    "w": 110,
    "h": 17.08,
    "text": "Refresh",
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
    "id": "inspector_refresh_bottom",
    "x": 264,
    "y": 640,
    "w": 110,
    "h": 36,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  }
]
```
