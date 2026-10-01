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
    "id": "scrim_70",
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
    "y": 70,
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
    "y": 90,
    "w": 220,
    "h": 24.4,
    "text": "Resume chat",
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
    "id": "banner",
    "x": 32,
    "y": 124,
    "w": 342,
    "h": 56,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "banner_t",
    "x": 44,
    "y": 132,
    "w": 300,
    "h": 16,
    "text": "History browsing authority changed.",
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
    "id": "banner_s",
    "x": 44,
    "y": 152,
    "w": 300,
    "h": 14,
    "text": "Refresh the catalog before selecting this row.",
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
    "id": "cand_0",
    "x": 32,
    "y": 196,
    "w": 342,
    "h": 56,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "cand_0_t",
    "x": 44,
    "y": 205,
    "w": 240,
    "h": 16,
    "text": "Fix steer queue drop on reconnect",
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
    "id": "cand_0_s",
    "x": 44,
    "y": 226,
    "w": 200,
    "h": 14,
    "text": "octos \u00b7 2m",
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
    "id": "cand_0_chip",
    "x": 292,
    "y": 204,
    "w": 78.0,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "cand_0_chip_t",
    "x": 300,
    "y": 207,
    "w": 62.0,
    "h": 14,
    "text": "unverified",
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
    "id": "resume_row_0",
    "x": 32,
    "y": 196,
    "w": 342,
    "h": 56,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "cand_1",
    "x": 32,
    "y": 258,
    "w": 342,
    "h": 56,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "cand_1_t",
    "x": 44,
    "y": 267,
    "w": 240,
    "h": 16,
    "text": "Add session fork",
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
    "id": "cand_1_s",
    "x": 44,
    "y": 288,
    "w": 200,
    "h": 14,
    "text": "octoscode-app \u00b7 1h",
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
    "id": "cand_1_chip",
    "x": 292,
    "y": 266,
    "w": 78.0,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "cand_1_chip_t",
    "x": 300,
    "y": 269,
    "w": 62.0,
    "h": 14,
    "text": "unverified",
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
    "id": "resume_row_1",
    "x": 32,
    "y": 258,
    "w": 342,
    "h": 56,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "cand_2",
    "x": 32,
    "y": 320,
    "w": 342,
    "h": 56,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "cand_2_t",
    "x": 44,
    "y": 329,
    "w": 240,
    "h": 16,
    "text": "Review PR #2566",
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
    "id": "cand_2_s",
    "x": 44,
    "y": 350,
    "w": 200,
    "h": 14,
    "text": "octos \u00b7 Yesterday",
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
    "id": "cand_2_chip",
    "x": 292,
    "y": 328,
    "w": 78.0,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "cand_2_chip_t",
    "x": 300,
    "y": 331,
    "w": 62.0,
    "h": 14,
    "text": "unverified",
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
    "id": "resume_row_2",
    "x": 32,
    "y": 320,
    "w": 342,
    "h": 56,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "confirm",
    "x": 32,
    "y": 390,
    "w": 342,
    "h": 104,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "confirm_h",
    "x": 44,
    "y": 398,
    "w": 240,
    "h": 14,
    "text": "Confirm exact title to resume:",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 10,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "resume_input_row",
    "x": 44,
    "y": 420,
    "w": 236,
    "h": 34,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "resume_input_bg",
    "x": 44,
    "y": 420,
    "w": 236,
    "h": 34,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "resume_input_text",
    "x": 56,
    "y": 428.0,
    "w": 212,
    "h": 18,
    "text": "Type the exact thread title above",
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
    "id": "resume_input",
    "x": 44,
    "y": 420,
    "w": 236,
    "h": 34,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "resume_confirm_row",
    "x": 292,
    "y": 444,
    "w": 74,
    "h": 32,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "resume_confirm_bg",
    "x": 292,
    "y": 444,
    "w": 74,
    "h": 32,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "resume_confirm_t",
    "x": 292,
    "y": 451.5,
    "w": 74,
    "h": 17.08,
    "text": "Resume",
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
    "id": "resume_confirm",
    "x": 292,
    "y": 444,
    "w": 74,
    "h": 32,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "t_note",
    "x": 32,
    "y": 508,
    "w": 342,
    "h": 14,
    "text": "A confirmed source Session is required to browse history.",
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
    "id": "locked",
    "x": 32,
    "y": 534,
    "w": 342,
    "h": 44,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "locked_t",
    "x": 44,
    "y": 547,
    "w": 240,
    "h": 16,
    "text": "This retained Session is closed.",
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
    "id": "locked_chip",
    "x": 316,
    "y": 545,
    "w": 53.2,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "locked_chip_t",
    "x": 324,
    "y": 548,
    "w": 37.2,
    "h": 14,
    "text": "locked",
    "font_src": "self:resources/ux/Inter-500.ttf",
    "size": 10,
    "weight": 500,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  }
]
```
