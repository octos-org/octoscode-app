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
    "id": "approval_card",
    "x": 16.0,
    "y": 74.0,
    "w": 374.0,
    "h": 622.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "icon_shield",
    "x": 25.0,
    "y": 85.0,
    "w": 30.0,
    "h": 34.0,
    "role": "unknown",
    "native_candidates": []
  },
  {
    "id": "t01",
    "x": 77.82,
    "y": 98.0,
    "w": 210.89,
    "h": 27.2,
    "text": "Run this command?",
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
    "id": "cmd_box",
    "x": 20.0,
    "y": 160.0,
    "w": 366.0,
    "h": 66.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "t02",
    "x": 29.0,
    "y": 185.0,
    "w": 340.0,
    "h": 30.0,
    "text": "git push origin feat/steer-queue",
    "font_src": "self:resources/ux/LiberationMono-Regular.ttf",
    "size": 15,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "reason_text",
    "x": 21.0,
    "y": 268.0,
    "w": 258.0,
    "h": 52.0,
    "text": "Reason: Push the fix branch so Cl can run",
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
    "id": "approve_once",
    "x": 18.0,
    "y": 364.0,
    "w": 368.0,
    "h": 70.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "approve_once_surface",
    "x": 18.0,
    "y": 364.0,
    "w": 368.0,
    "h": 70.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "approve_once_control",
    "x": 18.0,
    "y": 364.0,
    "w": 368.0,
    "h": 70.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "approve_once_label",
    "x": 136.0,
    "y": 390.0,
    "w": 200.0,
    "h": 30.0,
    "text": "Approve once",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 20.0,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "approve_session",
    "x": 18.0,
    "y": 448.0,
    "w": 368.0,
    "h": 70.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "approve_session_surface",
    "x": 18.0,
    "y": 448.0,
    "w": 368.0,
    "h": 70.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "approve_session_control",
    "x": 18.0,
    "y": 448.0,
    "w": 368.0,
    "h": 70.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "approve_session_label",
    "x": 109.0,
    "y": 472.0,
    "w": 240.0,
    "h": 32.0,
    "text": "Approve for session",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 21.33,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "deny",
    "x": 18.0,
    "y": 532.0,
    "w": 368.0,
    "h": 58.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "deny_surface",
    "x": 18.0,
    "y": 532.0,
    "w": 368.0,
    "h": 58.0,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "deny_control",
    "x": 18.0,
    "y": 532.0,
    "w": 368.0,
    "h": 58.0,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "deny_label",
    "x": 170.0,
    "y": 553.0,
    "w": 80.0,
    "h": 28.01,
    "text": "Deny",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 18.67,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "t_hint",
    "x": 153.38,
    "y": 653.0,
    "w": 78.94,
    "h": 21.5,
    "text": "Y /S / N",
    "font_src": "self:resources/ux/Inter-400.ttf",
    "size": 12,
    "weight": 400,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
