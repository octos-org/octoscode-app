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
    "id": "t_title",
    "x": 24,
    "y": 28,
    "w": 240,
    "h": 22,
    "text": "Thinking effort",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 18,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "seg_track",
    "x": 24,
    "y": 58,
    "w": 342,
    "h": 36,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "effort_low_row",
    "x": 24.0,
    "y": 58,
    "w": 85.5,
    "h": 36,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "effort_low_t",
    "x": 32.0,
    "y": 68,
    "w": 69.5,
    "h": 16,
    "text": "Low",
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
    "id": "effort_low",
    "x": 24.0,
    "y": 58,
    "w": 85.5,
    "h": 36,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "effort_medium_row",
    "x": 109.5,
    "y": 58,
    "w": 85.5,
    "h": 36,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "effort_medium_t",
    "x": 117.5,
    "y": 68,
    "w": 69.5,
    "h": 16,
    "text": "Medium",
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
    "id": "effort_medium",
    "x": 109.5,
    "y": 58,
    "w": 85.5,
    "h": 36,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "effort_high_row",
    "x": 195.0,
    "y": 58,
    "w": 85.5,
    "h": 36,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "effort_high_bg",
    "x": 197.0,
    "y": 60,
    "w": 81.5,
    "h": 32,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "effort_high_t",
    "x": 195.0,
    "y": 68,
    "w": 85.5,
    "h": 16,
    "text": "High",
    "font_src": "self:resources/ux/Inter-600.ttf",
    "size": 13,
    "weight": 600,
    "role": "text",
    "native_candidates": [
      "Label",
      "TextFlow"
    ]
  },
  {
    "id": "effort_high",
    "x": 195.0,
    "y": 58,
    "w": 85.5,
    "h": 36,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "effort_max_row",
    "x": 280.5,
    "y": 58,
    "w": 85.5,
    "h": 36,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "effort_max_t",
    "x": 288.5,
    "y": 68,
    "w": 69.5,
    "h": 16,
    "text": "Max",
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
    "id": "effort_max",
    "x": 280.5,
    "y": 58,
    "w": 85.5,
    "h": 36,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "t_help",
    "x": 24,
    "y": 102,
    "w": 320,
    "h": 15,
    "text": "Sets how much the model thinks before answering",
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
    "id": "t_show",
    "x": 24,
    "y": 136,
    "w": 220,
    "h": 17.08,
    "text": "Show reasoning",
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
    "id": "toggle_show_reasoning_row",
    "x": 322,
    "y": 132,
    "w": 44,
    "h": 26,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "toggle_show_reasoning_bg",
    "x": 322,
    "y": 132,
    "w": 44,
    "h": 26,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "toggle_show_reasoning_knob",
    "x": 343,
    "y": 135,
    "w": 20,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "toggle_show_reasoning",
    "x": 322,
    "y": 132,
    "w": 44,
    "h": 26,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "t_show_help",
    "x": 24,
    "y": 160,
    "w": 300,
    "h": 14,
    "text": "Shows the model's reasoning while it works",
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
    "id": "t_default",
    "x": 24,
    "y": 196,
    "w": 220,
    "h": 17.08,
    "text": "Default on for new chats",
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
    "id": "toggle_default_new_row",
    "x": 322,
    "y": 192,
    "w": 44,
    "h": 26,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "toggle_default_new_bg",
    "x": 322,
    "y": 192,
    "w": 44,
    "h": 26,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "toggle_default_new_knob",
    "x": 343,
    "y": 195,
    "w": 20,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "toggle_default_new",
    "x": 322,
    "y": 192,
    "w": 44,
    "h": 26,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "d0",
    "x": 20,
    "y": 240,
    "w": 366,
    "h": 1,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "thinking_expand_all_row",
    "x": 24,
    "y": 252,
    "w": 70,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "thinking_expand_all_t",
    "x": 24,
    "y": 252,
    "w": 70,
    "h": 17,
    "text": "Expand all",
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
    "id": "thinking_expand_all",
    "x": 24,
    "y": 252,
    "w": 70,
    "h": 20,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "thinking_collapse_all_row",
    "x": 104,
    "y": 252,
    "w": 80,
    "h": 20,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "thinking_collapse_all_t",
    "x": 104,
    "y": 252,
    "w": 80,
    "h": 17,
    "text": "Collapse all",
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
    "id": "thinking_collapse_all",
    "x": 104,
    "y": 252,
    "w": 80,
    "h": 20,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "row_0_row",
    "x": 24,
    "y": 284,
    "w": 342,
    "h": 40,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "row_0_chev",
    "x": 34,
    "y": 295,
    "w": 16,
    "h": 16,
    "text": "\u25be",
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
    "id": "row_0_t",
    "x": 56,
    "y": 295,
    "w": 190,
    "h": 16,
    "text": "Weighed two approaches",
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
    "id": "row_0_meta",
    "x": 240,
    "y": 296,
    "w": 114,
    "h": 14,
    "text": "1,204 tok \u00b7 3.2s",
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
    "id": "row_0",
    "x": 24,
    "y": 284,
    "w": 342,
    "h": 40,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  },
  {
    "id": "row_0_l0",
    "x": 72,
    "y": 330,
    "w": 280,
    "h": 14,
    "text": "- Latency vs correctness on the retry path",
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
    "id": "row_0_l1",
    "x": 72,
    "y": 352,
    "w": 280,
    "h": 14,
    "text": "- Token budget for the wider context",
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
    "id": "row_0_l2",
    "x": 72,
    "y": 374,
    "w": 280,
    "h": 14,
    "text": "- Order of the two tool calls",
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
    "id": "row_0_l3",
    "x": 72,
    "y": 396,
    "w": 280,
    "h": 14,
    "text": "- Fallback when the probe times out",
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
    "id": "row_1_row",
    "x": 24,
    "y": 428,
    "w": 342,
    "h": 40,
    "role": "layout",
    "native_candidates": [
      "View"
    ]
  },
  {
    "id": "row_1_chev",
    "x": 34,
    "y": 439,
    "w": 16,
    "h": 16,
    "text": "\u25b8",
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
    "id": "row_1_t",
    "x": 56,
    "y": 439,
    "w": 190,
    "h": 16,
    "text": "Checked the retry path",
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
    "id": "row_1_meta",
    "x": 240,
    "y": 440,
    "w": 114,
    "h": 14,
    "text": "842 tok \u00b7 1.9s",
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
    "id": "row_1",
    "x": 24,
    "y": 428,
    "w": 342,
    "h": 40,
    "role": "button",
    "native_candidates": [
      "Button",
      "KitButton"
    ]
  }
]
```
