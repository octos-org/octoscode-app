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
    "id": "t_head",
    "x": 20,
    "y": 18,
    "w": 200,
    "h": 24,
    "text": "OctosCode",
    "font_src": "self:resources/ux/Inter-700.ttf",
    "size": 19,
    "weight": 700,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "i_pencil",
    "x": 22,
    "y": 56,
    "w": 18,
    "h": 18,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_newchat",
    "x": 48,
    "y": 54,
    "w": 120,
    "h": 22,
    "text": "New chat",
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
    "id": "ctl_newchat_control",
    "x": 20.0,
    "y": 50.0,
    "w": 140.0,
    "h": 30.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "sf",
    "x": 20,
    "y": 92,
    "w": 366,
    "h": 40,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "sf_hair",
    "x": 0,
    "y": 0,
    "w": 366,
    "h": 40,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "i_search",
    "x": 34,
    "y": 104,
    "w": 16,
    "h": 16,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_search",
    "x": 58,
    "y": 102,
    "w": 240,
    "h": 22,
    "text": "Search chats",
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
    "id": "ctl_search_control",
    "x": 20.0,
    "y": 92.0,
    "w": 366.0,
    "h": 40.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "seg",
    "x": 20,
    "y": 148,
    "w": 214,
    "h": 34,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "seg_sel",
    "x": 129,
    "y": 152,
    "w": 105,
    "h": 26,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_seg_l",
    "x": 24,
    "y": 156,
    "w": 105,
    "h": 20,
    "text": "By workspace",
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
    "id": "t_seg_r",
    "x": 129,
    "y": 156,
    "w": 105,
    "h": 20,
    "text": "All",
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
    "id": "ctl_seg_ws_control",
    "x": 24.0,
    "y": 152.0,
    "w": 105.0,
    "h": 26.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "ctl_seg_all_control",
    "x": 129.0,
    "y": 152.0,
    "w": 105.0,
    "h": 26.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "i_sort",
    "x": 380,
    "y": 156,
    "w": 14,
    "h": 14,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_sort",
    "x": 330,
    "y": 156,
    "w": 48,
    "h": 20,
    "text": "Recent",
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
    "id": "i_st0",
    "x": 24,
    "y": 215,
    "w": 16,
    "h": 16,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "st_t0",
    "x": 48,
    "y": 214,
    "w": 240,
    "h": 20,
    "text": "Running",
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
    "id": "st_w0",
    "x": 300,
    "y": 214,
    "w": 84,
    "h": 20,
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
    "id": "ctl_strow0_control",
    "x": 16.0,
    "y": 208.0,
    "w": 374.0,
    "h": 36.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "i_st1",
    "x": 24,
    "y": 257,
    "w": 16,
    "h": 16,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "st_t1",
    "x": 48,
    "y": 256,
    "w": 240,
    "h": 20,
    "text": "Waiting for input",
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
    "id": "st_w1",
    "x": 300,
    "y": 256,
    "w": 84,
    "h": 20,
    "text": "1h",
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
    "id": "ctl_strow1_control",
    "x": 16.0,
    "y": 250.0,
    "w": 374.0,
    "h": 36.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "i_st2",
    "x": 24,
    "y": 299,
    "w": 16,
    "h": 16,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "st_t2",
    "x": 48,
    "y": 298,
    "w": 240,
    "h": 20,
    "text": "Done",
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
    "id": "st_w2",
    "x": 300,
    "y": 298,
    "w": 84,
    "h": 20,
    "text": "Yesterday",
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
    "id": "ctl_strow2_control",
    "x": 16.0,
    "y": 292.0,
    "w": 374.0,
    "h": 36.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "i_st3",
    "x": 24,
    "y": 341,
    "w": 16,
    "h": 16,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "st_t3",
    "x": 48,
    "y": 340,
    "w": 240,
    "h": 20,
    "text": "Failed",
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
    "id": "st_w3",
    "x": 300,
    "y": 340,
    "w": 84,
    "h": 20,
    "text": "2h",
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
    "id": "ctl_strow3_control",
    "x": 16.0,
    "y": 334.0,
    "w": 374.0,
    "h": 36.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "i_st4",
    "x": 24,
    "y": 383,
    "w": 16,
    "h": 16,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "st_t4",
    "x": 48,
    "y": 382,
    "w": 240,
    "h": 20,
    "text": "Idle",
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
    "id": "st_w4",
    "x": 300,
    "y": 382,
    "w": 84,
    "h": 20,
    "text": "Yesterday",
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
    "id": "ctl_strow4_control",
    "x": 16.0,
    "y": 376.0,
    "w": 374.0,
    "h": 36.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "i_plus",
    "x": 24,
    "y": 726,
    "w": 16,
    "h": 16,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_addws",
    "x": 48,
    "y": 724,
    "w": 160,
    "h": 20,
    "text": "Add workspace",
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
    "id": "ctl_addws_control",
    "x": 16.0,
    "y": 718.0,
    "w": 220.0,
    "h": 32.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  }
]
```
