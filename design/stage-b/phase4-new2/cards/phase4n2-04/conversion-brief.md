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
    "x": 24,
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
    "id": "i_chev_octos",
    "x": 24,
    "y": 216,
    "w": 14,
    "h": 14,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_grp_octos",
    "x": 46,
    "y": 214,
    "w": 180,
    "h": 22,
    "text": "octos",
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
    "id": "ctl_grp_octos_control",
    "x": 16.0,
    "y": 210.0,
    "w": 250.0,
    "h": 30.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "grp_badge",
    "x": 320,
    "y": 214,
    "w": 30,
    "h": 22,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "grp_badge_t",
    "x": 320,
    "y": 216,
    "w": 30,
    "h": 18,
    "text": "3",
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
    "id": "grp_menu_btn",
    "x": 356,
    "y": 214,
    "w": 30,
    "h": 22,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "i_dots",
    "x": 362,
    "y": 218,
    "w": 18,
    "h": 14,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "ctl_grp_menu_control",
    "x": 356.0,
    "y": 210.0,
    "w": 34.0,
    "h": 30.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "menu",
    "x": 322,
    "y": 246,
    "w": 150,
    "h": 104,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "menu_hair",
    "x": 0,
    "y": 0,
    "w": 150,
    "h": 104,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "menu_t0",
    "x": 336,
    "y": 258,
    "w": 128,
    "h": 20,
    "text": "New chat here",
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
    "id": "ctl_menu0_control",
    "x": 326.0,
    "y": 252.0,
    "w": 142.0,
    "h": 28.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "menu_t1",
    "x": 336,
    "y": 288,
    "w": 128,
    "h": 20,
    "text": "Rename",
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
    "id": "ctl_menu1_control",
    "x": 326.0,
    "y": 282.0,
    "w": 142.0,
    "h": 28.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "menu_t2",
    "x": 336,
    "y": 318,
    "w": 128,
    "h": 20,
    "text": "Remove from sidebar",
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
    "id": "ctl_menu2_control",
    "x": 326.0,
    "y": 312.0,
    "w": 142.0,
    "h": 28.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "i_chev_octoscode_app",
    "x": 24,
    "y": 268,
    "w": 14,
    "h": 14,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_grp_octoscode_app",
    "x": 46,
    "y": 266,
    "w": 180,
    "h": 22,
    "text": "octoscode-app",
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
    "id": "ctl_grp_octoscode_app_control",
    "x": 16.0,
    "y": 262.0,
    "w": 250.0,
    "h": 30.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "r00_t",
    "x": 56,
    "y": 302,
    "w": 250,
    "h": 20,
    "text": "Bump octos-core to a6ea8505",
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
    "id": "r00_w",
    "x": 300,
    "y": 302,
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
    "id": "ctl_row0_control",
    "x": 16.0,
    "y": 296.0,
    "w": 374.0,
    "h": 36.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "r01_t",
    "x": 56,
    "y": 342,
    "w": 250,
    "h": 20,
    "text": "Why is hydrate slow?",
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
    "id": "r01_w",
    "x": 300,
    "y": 342,
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
    "id": "ctl_row1_control",
    "x": 16.0,
    "y": 336.0,
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
