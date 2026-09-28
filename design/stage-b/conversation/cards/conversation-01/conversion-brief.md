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
    "id": "t01",
    "x": 22.46,
    "y": 46.77,
    "w": 154.7,
    "h": 28.5,
    "text": "OctosCode \u25be",
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
    "id": "icon_bell",
    "x": 354.0,
    "y": 50.0,
    "w": 18.0,
    "h": 18.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "icon_search",
    "x": 380.0,
    "y": 50.0,
    "w": 18.0,
    "h": 18.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "new_chat",
    "x": 16.0,
    "y": 120.0,
    "w": 374.0,
    "h": 44.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "new_chat_surface",
    "x": 16.0,
    "y": 120.0,
    "w": 374.0,
    "h": 44.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "new_chat_control",
    "x": 16.0,
    "y": 120.0,
    "w": 374.0,
    "h": 44.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "new_chat_label",
    "x": 38.0,
    "y": 132.0,
    "w": 140.0,
    "h": 24.0,
    "text": "New chat",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 16.0,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "icon_compose",
    "x": 22.0,
    "y": 132.0,
    "w": 20.0,
    "h": 20.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "thread_1",
    "x": 16.0,
    "y": 215.5,
    "w": 374.0,
    "h": 60.5,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "thread_1_surface",
    "x": 16.0,
    "y": 215.5,
    "w": 374.0,
    "h": 60.5,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "thread_1_control",
    "x": 16.0,
    "y": 215.5,
    "w": 374.0,
    "h": 60.5,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "thread_1_label",
    "x": 32.22,
    "y": 229.5,
    "w": 300.0,
    "h": 32.51,
    "text": "Fix steer queue drop on reconnect",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 21.67,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "thread_2",
    "x": 16.0,
    "y": 299.5,
    "w": 374.0,
    "h": 49.5,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "thread_2_surface",
    "x": 16.0,
    "y": 299.5,
    "w": 374.0,
    "h": 49.5,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "thread_2_control",
    "x": 16.0,
    "y": 299.5,
    "w": 374.0,
    "h": 49.5,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "thread_2_label",
    "x": 34.96,
    "y": 313.5,
    "w": 300.0,
    "h": 21.5,
    "text": "Add session fork",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 14.33,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "thread_3",
    "x": 16.0,
    "y": 380.77,
    "w": 374.0,
    "h": 52.81,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "thread_3_surface",
    "x": 16.0,
    "y": 380.77,
    "w": 374.0,
    "h": 52.81,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "thread_3_control",
    "x": 16.0,
    "y": 380.77,
    "w": 374.0,
    "h": 52.81,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "thread_3_label",
    "x": 36.09,
    "y": 394.77,
    "w": 300.0,
    "h": 24.81,
    "text": "Review PR #2566",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 16.54,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "thread_4",
    "x": 16.0,
    "y": 467.5,
    "w": 374.0,
    "h": 54.06,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "thread_4_surface",
    "x": 16.0,
    "y": 467.5,
    "w": 374.0,
    "h": 54.06,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "thread_4_control",
    "x": 16.0,
    "y": 467.5,
    "w": 374.0,
    "h": 54.06,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "thread_4_label",
    "x": 36.09,
    "y": 481.5,
    "w": 300.0,
    "h": 26.06,
    "text": "Bump octos-core to a6ea8505",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 17.37,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "thread_5",
    "x": 16.0,
    "y": 551.0,
    "w": 374.0,
    "h": 55.15,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "thread_5_surface",
    "x": 16.0,
    "y": 551.0,
    "w": 374.0,
    "h": 55.15,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "thread_5_control",
    "x": 16.0,
    "y": 551.0,
    "w": 374.0,
    "h": 55.15,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "thread_5_label",
    "x": 34.96,
    "y": 565.0,
    "w": 300.0,
    "h": 27.15,
    "text": "Why is hydrate slow?",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 18.1,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "icon_fork",
    "x": 350.0,
    "y": 314.5,
    "w": 18.0,
    "h": 18.0,
    "role": "unknown",
    "native_candidates": []
  }
]
```
