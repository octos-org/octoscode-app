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
    "id": "pair_back_chev",
    "x": 15.0,
    "y": 130.0,
    "w": 14.0,
    "h": 22.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "pair_back_control",
    "x": 5.0,
    "y": 121.0,
    "w": 34.0,
    "h": 40.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "t_title",
    "x": 125.53,
    "y": 164.25,
    "w": 151.72,
    "h": 36.0,
    "text": "Connection",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 24,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "conn_card",
    "x": 28.0,
    "y": 228.0,
    "w": 350.0,
    "h": 196.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "conn_div1",
    "x": 36.0,
    "y": 296.0,
    "w": 334.0,
    "h": 1.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "conn_div2",
    "x": 36.0,
    "y": 366.0,
    "w": 334.0,
    "h": 1.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_srv_k",
    "x": 41.52,
    "y": 247.48,
    "w": 56.8,
    "h": 22.5,
    "text": "Server",
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
    "id": "t_srv_v",
    "x": 206.38,
    "y": 248.14,
    "w": 161.27,
    "h": 22.5,
    "text": "192.168.1.20:50190",
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
    "id": "t_pair_k",
    "x": 40.5,
    "y": 317.76,
    "w": 57.72,
    "h": 22.5,
    "text": "Paired",
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
    "id": "t_pair_v",
    "x": 244.56,
    "y": 317.07,
    "w": 122.14,
    "h": 23.43,
    "text": "Today, 9:41 PM",
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
    "id": "t_stays",
    "x": 41.73,
    "y": 388.0,
    "w": 212.02,
    "h": 21.5,
    "text": "Stays on this device only",
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
    "id": "pair_forget",
    "x": 117.0,
    "y": 608.0,
    "w": 172.0,
    "h": 40.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "pair_forget_surface",
    "x": 117.0,
    "y": 608.0,
    "w": 172.0,
    "h": 40.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "pair_forget_control",
    "x": 117.0,
    "y": 608.0,
    "w": 172.0,
    "h": 40.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "pair_forget_label",
    "x": 117.0,
    "y": 618.0,
    "w": 158.0,
    "h": 24.0,
    "text": "Forget this device",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 16,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
