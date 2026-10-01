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
    "id": "t_title",
    "x": 106.01,
    "y": 90.0,
    "w": 195.11,
    "h": 30.0,
    "text": "Pair with Octos",
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
    "id": "vf",
    "x": 66.0,
    "y": 140.0,
    "w": 274.0,
    "h": 204.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "vf_br_tl",
    "x": 78.0,
    "y": 152.0,
    "w": 22.0,
    "h": 22.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "vf_br_tr",
    "x": 306.0,
    "y": 152.0,
    "w": 22.0,
    "h": 22.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "vf_br_bl",
    "x": 78.0,
    "y": 310.0,
    "w": 22.0,
    "h": 22.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "vf_br_br",
    "x": 306.0,
    "y": 310.0,
    "w": 22.0,
    "h": 22.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t_cap1",
    "x": 67.67,
    "y": 360.93,
    "w": 269.54,
    "h": 21.0,
    "text": "Scan the pairing QR shown in Octos",
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
    "id": "t_cap2",
    "x": 136.28,
    "y": 386.42,
    "w": 135.75,
    "h": 21.0,
    "text": "on your computer",
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
    "id": "div_l",
    "x": 40.0,
    "y": 432.55,
    "w": 140.0,
    "h": 1.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "div_r",
    "x": 226.0,
    "y": 432.55,
    "w": 140.0,
    "h": 1.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t_or",
    "x": 193.98,
    "y": 426.35,
    "w": 19.17,
    "h": 18.0,
    "text": "Or",
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
    "id": "t_link_label",
    "x": 31.58,
    "y": 463.5,
    "w": 125.18,
    "h": 19.5,
    "text": "Paste pairing link",
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
    "id": "pair_link_wrap",
    "x": 28.0,
    "y": 484.0,
    "w": 350.0,
    "h": 48.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "pair_link_field",
    "x": 28.0,
    "y": 484.0,
    "w": 350.0,
    "h": 48.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "pair_link",
    "x": 42.0,
    "y": 484.0,
    "w": 322.0,
    "h": 48.0,
    "text": "",
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
    "id": "pair_submit",
    "x": 128.0,
    "y": 562.0,
    "w": 150.0,
    "h": 48.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "pair_submit_surface",
    "x": 128.0,
    "y": 562.0,
    "w": 150.0,
    "h": 48.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "pair_submit_control",
    "x": 128.0,
    "y": 562.0,
    "w": 150.0,
    "h": 48.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "pair_submit_label",
    "x": 185.0,
    "y": 576.0,
    "w": 36.0,
    "h": 22.5,
    "text": "Pair",
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
    "id": "pair_fallback",
    "x": 78.0,
    "y": 624.0,
    "w": 250.0,
    "h": 30.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "pair_fallback_surface",
    "x": 78.0,
    "y": 624.0,
    "w": 250.0,
    "h": 30.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "pair_fallback_control",
    "x": 78.0,
    "y": 624.0,
    "w": 250.0,
    "h": 30.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "pair_fallback_label",
    "x": 78.0,
    "y": 629.0,
    "w": 250.0,
    "h": 21.0,
    "text": "Enter server and token instead",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 14,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
