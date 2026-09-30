//! octoscode-module — the `AppModule` (re-homed from the D9 shape-(i) spike).
//!
//! It owns a [`Store`] and a transport on a background tokio runtime, renders
//! the connection state + session count **from the store**, and exposes store
//! data to cards only through [`bindings`] (a card never sees a Rust type).
//! L0 cards mount here later; this card ships no card.
//!
//! ## Card #12: the conversation
//!
//! The module now drives a [`flow::Conversation`] — the gate's end-to-end path
//! (connect → pick profile → open a workspace → `turn/start` → deltas → tool
//! rows → `turn/interrupt` → `turn/completed`) — and renders the
//! **fallback** view ([`fallback`]) **only through binding ids**. The view
//! emits binding **action ids**; [`perform_action`] maps them back to flow
//! calls. That is 8.8 condition 2 end to end: the view names ids, the module
//! owns the meaning.
//!
//! Threading rule (from the spike): the UI thread never blocks. The tokio
//! runtime owns the transport; its event waker (`SignalToUI`) wakes the UI,
//! which drains events into the store and re-reads bindings into widgets.
pub use makepad_widgets;

use makepad_app_module::{
    makepad_ai_services::wire::{ServiceCall, ServiceManifest, ToolResult},
    AppModule, ExecOutcome, InstanceHandles, InstanceParts, OpenSchema, ServiceExecutor,
    ValidatedOpen,
};
use makepad_widgets::*;
use std::sync::{Arc, Mutex};

use octoscode_store::Store;

pub mod actions;
pub mod bindings;
pub mod cards;
pub mod components;
pub mod fallback;
pub mod l0_host;
pub mod flow;
pub mod mount;
pub mod screen;

use flow::{Conversation, FlowUi};
// A top-level `::` path is not a `#[rust]` field type the `Script` derive's
// parser accepts (its `eat_type` reads one ident + optional generics), so the
// cache types are aliased to bare idents here.
use mount::MountCache as ComponentMounts;
use screen::Cache as ScreenCache;

script_mod! {
    use mod.prelude.widgets.*
    mod.widgets.OctoscodeView = set_type_default() do #(OctoscodeView::register_widget(vm)) {
        ..mod.widgets.RectView
        width: Fill height: Fill
        draw_bg.color: theme.color_bg_app
        // Card #28e (board 4): the root is an OVERLAY — base chrome, a dimmer,
        // and the floating command palette stack on one origin; `first_run`
        // replaces `base` before a connection.
        flow: Overlay
        // The base chrome (everything except the overlays and the first-run
        // card). The first-run frame hides it (`base.set_visible(false)`).
        base := View {
        width: Fill height: Fill
        // Card #28e (board 4): no global padding — the sidebar is flush with
        // the window edge (#F7F7F8, 260 px) and the review drawer is flush
        // right; each column carries its own inset.
        padding: 0
        flow: Down spacing: 10
        // Card #21c item 7: the debug header is GONE from the visible UI. The
        // connection state / session count stay as 1px labels so `/g` still
        // carries them and `sync_labels` keeps a target (board: "connection
        // state can go to /g metadata").
        header_meta := View {
            width: 0 height: 0 flow: Right
            status := Label { width: 0 height: 0 draw_text.text_style.font_size: 1 text: "" }
            sessions := Label { width: 0 height: 0 draw_text.text_style.font_size: 1 text: "" }
        }
        // Card #17 (D12): native columns + virtualized L0 items. The host owns
        // structure + scale; each list item's LOOK comes from an L0 component
        // lowered in-process (`components.rs`, the same chain as a card). Left =
        // thread list, center = timeline with the composer docked at its bottom,
        // right = the review slot (empty for now).
        //
        // Card #21c items 2/3/6: the component IS the item — no native `kind`
        // label, no row chrome, and each row's height comes from its lowered
        // component (`Splash height: Fit`), so a bubble hugs its text and prose
        // is not clipped.
        columns := View {
            width: Fill height: Fill
            flow: Right spacing: 10

            threads_column := View {
                width: 260 height: Fill flow: Down spacing: 6
                // Card #28e item 1 (board 4): the sidebar is flush with the
                // window edge on #F7F7F8 (the base no longer pads it).
                draw_bg.color: #F7F7F8
                padding: Inset{left: 12 right: 10 top: 12 bottom: 12}
                // Card #28e item 1 (board 4): the sidebar is 260 px with an
                // `OctosCode ▾` header above `New chat`, then THREADS, then the
                // autonomy sections (GOALS / LOOPS / FLEET) — native shell rows
                // for now, L0 components only where one exists (thread-row,
                // new-chat).
                sidebar_header := Label {
                    width: Fill height: Fit text: "OctosCode ▾"
                    draw_text.text_style.font_size: 13
                }
                // Card #21c item 7: `New chat` is #16's own `new-chat` component
                // at the top of the thread column (scene 01). A transparent hit
                // target overlays it so the HOST routes `session.new` — the
                // component's own button lives in the Splash isolate and never
                // reports to the host (the `row_hit` pattern, below).
                new_chat_row := View {
                    width: Fill height: Fit flow: Overlay
                    // Card #21g item 3: the #16 `new-chat` card ran flush to the
                    // thread column's right edge, while the thread rows below it
                    // stop short of that edge (the list reserves its scrollbar).
                    // Give the card the same right inset so the two align.
                    margin: Inset{left: 0 top: 0 right: 13 bottom: 0}
                    // Card #21e item 2: the #16 `new-chat` component's artboard is
                    // 374x76 (scene-01 `mapped.json`); a fixed 44px slot clipped its
                    // bottom edge flat under the label. `Fit` takes the component's
                    // own measured height (the same idiom `thread_splash` uses).
                    new_chat_splash := Splash { width: Fill height: Fit }
                    new_chat_hit := Button {
                        width: Fill height: Fill text: ""
                        draw_bg.color: #00000000
                        draw_bg.color_hover: #00000010
                        draw_bg.color_down: #00000020
                        draw_bg.border_size: 0.0
                        draw_bg.color_2: #00000000
                        draw_bg.border_color: #00000000
                        draw_bg.border_color_2: #00000000
                    }
                }
                // Card #28e item 1: the autonomy sections — GOALS / LOOPS / FLEET
                // — appear only when the session has them (board 4 frame 3).
                // Native shell rows (no #16 component exists for these yet); the
                // section label is small grey caps, each row 32 px.
                autonomy_sections := View {
                    width: Fill height: Fit flow: Down spacing: 4 visible: false
                    Label {
                        width: Fill height: Fit text: "GOALS"
                        draw_text.text_style.font_size: 10
                        draw_text.color: #6E6E73
                    }
                    goals_list := View {
                        width: Fill height: Fit flow: Down spacing: 2
                        goal_row_1 := Label { width: Fill height: 32 text: "" draw_text.text_style.font_size: 13 }
                        goal_row_2 := Label { width: Fill height: 32 text: "" draw_text.text_style.font_size: 13 }
                    }
                    Label {
                        width: Fill height: Fit text: "LOOPS"
                        draw_text.text_style.font_size: 10
                        draw_text.color: #6E6E73
                    }
                    loops_list := View {
                        width: Fill height: Fit flow: Down spacing: 2
                        loop_row_1 := Label { width: Fill height: 32 text: "" draw_text.text_style.font_size: 13 }
                        loop_row_2 := Label { width: Fill height: 32 text: "" draw_text.text_style.font_size: 13 }
                    }
                    Label {
                        width: Fill height: Fit text: "FLEET"
                        draw_text.text_style.font_size: 10
                        draw_text.color: #6E6E73
                    }
                    fleet_list := View {
                        width: Fill height: Fit flow: Down spacing: 2
                        fleet_row_1 := Label { width: Fill height: 32 text: "" draw_text.text_style.font_size: 13 }
                        fleet_row_2 := Label { width: Fill height: 32 text: "" draw_text.text_style.font_size: 13 }
                        fleet_row_3 := Label { width: Fill height: 32 text: "" draw_text.text_style.font_size: 13 }
                    }
                }
                // Card #21c item 6: one #16 `thread-row` per session (selected
                // state, ellipsized title) — the component draws its own label,
                // so no native title Label sits under it. A transparent `row_hit`
                // routes the click with the item id (`thread.open`).
                // Card #28e item 1: the THREADS section label (small grey caps)
                // above the list.
                Label {
                    width: Fill height: Fit text: "THREADS"
                    draw_text.text_style.font_size: 10
                    draw_text.color: #6E6E73
                }
                thread_list := PortalList {
                    width: Fill height: Fill flow: Down drag_scrolling: true
                    // Card #21g item 3: the rows must share the New chat card's
                    // right inset so the two align (the atlas insets the card and
                    // the selected row to one right edge).
                    margin: Inset{left: 0 top: 0 right: 13 bottom: 0}
                    ThreadRowTpl := View {
                        width: Fill height: Fit flow: Overlay
                        thread_splash := Splash { width: Fill height: Fit }
                        row_hit := Button {
                            width: Fill height: Fill text: ""
                            draw_bg.color: #00000000
                            draw_bg.color_hover: #00000012
                            draw_bg.color_down: #00000022
                            draw_bg.border_size: 0.0
                            draw_bg.color_2: #00000000
                            draw_bg.border_color: #00000000
                            draw_bg.border_color_2: #00000000
                        }
                    }
                }
            }
            // Card #28e item 1 (board 4): the 1 px hairline between the
            // sidebar and the conversation column.
            sidebar_rule := View {
                width: 1 height: Fill
                draw_bg.color: #E5E5E7
            }

            conversation_column := View {
                width: Fill height: Fill flow: Down spacing: 6
                // Card #28e item 2 (board 4): the conversation column is centered
                // with a max width of 720 px. `align.x: 0.5` centers the
                // `max_width: 720` child inside the Fill column.
                align: Align{x: 0.5 y: 0.0}
                conversation_inner := View {
                    width: Fill height: Fill flow: Down spacing: 6
                    max_width: 720
                // Card #21c item 2: the component IS the item. No native `kind`
                // label and no row chrome — the row is just the lowered
                // component plus a transparent hit target.
                timeline_list := PortalList {
                    width: Fill height: Fill flow: Down drag_scrolling: true
                    // Card #21c L2: tail the newest item so a newly appended
                    // turn scrolls into view. Without it the list stayed pinned
                    // to row 0 and turn 2 (`Worked for` alone changed) never
                    // showed. `auto_tail` only tails while already at the end
                    // (portal_list.rs:770), so scrolling up to read is preserved.
                    auto_tail: true
                    TimelineItemTpl := View {
                        // Card #21c item 3: the row height comes from the lowered
                        // component (`Splash height: Fit` measures its root), so a
                        // bubble hugs its text and prose is not clipped.
                        width: Fill height: Fit flow: Overlay
                        item_splash := Splash { width: Fill height: Fit }
                        row_hit := Button {
                            width: Fill height: Fill text: ""
                            draw_bg.color: #00000000
                            draw_bg.color_hover: #00000010
                            draw_bg.color_down: #00000020
                            draw_bg.border_size: 0.0
                            draw_bg.color_2: #00000000
                            draw_bg.border_color: #00000000
                            draw_bg.border_color_2: #00000000
                        }
                    }
                }
                // Card #21c item 5: ONE composer — the #16 `composer` component is
                // the whole input surface (its own input + `+` + mic + send). The
                // old native TextInput + `Steer now / Send / ×` row is GONE. The
                // component's own controls live in the Splash isolate and do not
                // report to the host, so transparent host hit targets are laid
                // over its fixed control rects (fixed chrome, RULES: measured
                // layout is for chrome) and routed to the declared action ids.
                composer_row := View {
                    width: Fill height: Fit flow: Overlay
                    composer_splash := Splash { width: Fill height: 190 }
                    composer_hits := View {
                        width: Fill height: 190 flow: Overlay
                        plus_hit := Button {
                            width: 36 height: 36 text: ""
                            margin: Inset{left: 5.0 top: 123.0}
                            draw_bg.color: #00000000
                            draw_bg.color_hover: #00000010
                            draw_bg.color_down: #00000020
                            draw_bg.border_size: 0.0
                            draw_bg.color_2: #00000000
                            draw_bg.border_color: #00000000
                            draw_bg.border_color_2: #00000000
                            draw_bg.color_hover: #00000000
                            draw_bg.color_down: #00000000
                            draw_bg.color_focus: #00000000
                            draw_bg.color_disabled: #00000000
                            draw_bg.color_2_hover: #00000000
                            draw_bg.color_2_down: #00000000
                            draw_bg.color_2_focus: #00000000
                            draw_bg.color_2_disabled: #00000000
                            draw_bg.border_color_hover: #00000000
                            draw_bg.border_color_down: #00000000
                            draw_bg.border_color_focus: #00000000
                            draw_bg.border_color_disabled: #00000000
                            draw_bg.border_color_2_hover: #00000000
                            draw_bg.border_color_2_down: #00000000
                            draw_bg.border_color_2_focus: #00000000
                            draw_bg.border_color_2_disabled: #00000000
                        }
                        mic_hit := Button {
                            width: 36 height: 36 text: ""
                            margin: Inset{left: 280.0 top: 121.0}
                            draw_bg.color: #00000000
                            draw_bg.color_hover: #00000010
                            draw_bg.color_down: #00000020
                            draw_bg.border_size: 0.0
                            draw_bg.color_2: #00000000
                            draw_bg.border_color: #00000000
                            draw_bg.border_color_2: #00000000
                            draw_bg.color_hover: #00000000
                            draw_bg.color_down: #00000000
                            draw_bg.color_focus: #00000000
                            draw_bg.color_disabled: #00000000
                            draw_bg.color_2_hover: #00000000
                            draw_bg.color_2_down: #00000000
                            draw_bg.color_2_focus: #00000000
                            draw_bg.color_2_disabled: #00000000
                            draw_bg.border_color_hover: #00000000
                            draw_bg.border_color_down: #00000000
                            draw_bg.border_color_focus: #00000000
                            draw_bg.border_color_disabled: #00000000
                            draw_bg.border_color_2_hover: #00000000
                            draw_bg.border_color_2_down: #00000000
                            draw_bg.border_color_2_focus: #00000000
                            draw_bg.border_color_2_disabled: #00000000
                        }
                        send_hit := Button {
                            width: 44 height: 44 text: ""
                            margin: Inset{left: 324.0 top: 115.0}
                            draw_bg.color: #00000000
                            draw_bg.color_hover: #00000010
                            draw_bg.color_down: #00000020
                            draw_bg.border_size: 0.0
                            draw_bg.color_2: #00000000
                            draw_bg.border_color: #00000000
                            draw_bg.border_color_2: #00000000
                            draw_bg.color_hover: #00000000
                            draw_bg.color_down: #00000000
                            draw_bg.color_focus: #00000000
                            draw_bg.color_disabled: #00000000
                            draw_bg.color_2_hover: #00000000
                            draw_bg.color_2_down: #00000000
                            draw_bg.color_2_focus: #00000000
                            draw_bg.color_2_disabled: #00000000
                            draw_bg.border_color_hover: #00000000
                            draw_bg.border_color_down: #00000000
                            draw_bg.border_color_focus: #00000000
                            draw_bg.border_color_disabled: #00000000
                            draw_bg.border_color_2_hover: #00000000
                            draw_bg.border_color_2_down: #00000000
                            draw_bg.border_color_2_focus: #00000000
                            draw_bg.border_color_2_disabled: #00000000
                        }
                    }
                }
            } // conversation_inner
        }

            // Card #28e item 3 (board 4 frame 1): the 560 px Review panel, toggled by the
            // Review affordance (an "Edited files" card later; the header pill
            // works today). Empty for now — the header + scope pill only.
            review_panel := SolidView {
                width: 560 height: Fill flow: Down spacing: 6
                visible: false
                draw_bg.color: #FFFFFF
                review_header := View {
                    width: Fill height: Fit flow: Right spacing: 8
                    Label {
                        width: Fit height: Fit text: "Review"
                        draw_text.text_style.font_size: 14
                    }
                    review_scope := RoundedView {
                        width: Fit height: Fit flow: Right
                        padding: Inset{left: 10 right: 10 top: 4 bottom: 4}
                        draw_bg +: {color: #F0F0F2 border_radius: 999.0}
                        Label {
                            width: Fit height: Fit text: "Last turn ▾"
                            draw_text.text_style.font_size: 11
                        }
                    }
                    review_toggle_hit := Button {
                        width: Fit height: Fit text: "Review"
                        draw_bg.color: #00000000
                        draw_bg.color_hover: #00000010
                        draw_bg.color_down: #00000020
                        draw_bg.border_size: 0.0
                        draw_bg.color_2: #00000000
                        draw_bg.border_color: #00000000
                        draw_bg.border_color_2: #00000000
                    }
                    review_close := Button {
                        width: 28 height: 28 text: "✕"
                        draw_bg.color: #00000000
                        draw_bg.color_hover: #00000010
                        draw_bg.color_down: #00000020
                        draw_bg.border_size: 0.0
                        draw_bg.color_2: #00000000
                        draw_bg.border_color: #00000000
                        draw_bg.border_color_2: #00000000
                    }
                }
            }

            // Card #28e item 4 (board 4 frame 2): the 420 px Session-settings
            // drawer, docked right INSIDE the columns Right-flow (hidden = no
            // space, visible = the center narrows, as in board 4 frame 2).
            // Content comes in Stage C; this is the drawer shell (title + close).
            settings_drawer := SolidView {
                width: 420 height: Fill flow: Down spacing: 10
                visible: false
                draw_bg.color: #FFFFFF
                settings_header := View {
                    width: Fill height: Fit flow: Right spacing: 8
                    Label {
                        width: Fill height: Fit text: "Session settings"
                        draw_text.text_style.font_size: 14
                    }
                    settings_close := Button {
                        width: 28 height: 28 text: "✕"
                        draw_bg.color: #00000000
                        draw_bg.color_hover: #00000010
                        draw_bg.color_down: #00000020
                        draw_bg.border_size: 0.0
                        draw_bg.color_2: #00000000
                        draw_bg.border_color: #00000000
                        draw_bg.border_color_2: #00000000
                    }
                }
                Label { width: Fill height: Fit text: "Model" draw_text.text_style.font_size: 11 draw_text.color: #6E6E73 }
                Label { width: Fill height: Fit text: "Permissions" draw_text.text_style.font_size: 11 draw_text.color: #6E6E73 }
                Label { width: Fill height: Fit text: "Sandbox" draw_text.text_style.font_size: 11 draw_text.color: #6E6E73 }
                Label { width: Fill height: Fit text: "Context" draw_text.text_style.font_size: 11 draw_text.color: #6E6E73 }
            }
        }
        } // base

        // Card #28e item 5 (board 4 frame 3): a dimmer between the base chrome
        // and the floating palette (the "conversation dimmed slightly" layer).
        dimmer := SolidView {
            width: Fill height: Fill
            visible: false
            draw_bg.color: #1D1D1F40
        }

        // Card #28e item 5 (board 4 frame 3): the floating 560 px command
        // palette, near the top of the window. Cmd+K and "/" in an empty
        // composer open it; Esc closes. The search field + the command rows are
        // native shell chrome (no #16 component yet). CENTERED at any window
        // width: the dock wrapper centers the card (the old abs_pos(440,120)
        // was a 1440-only guess); the card's top margin keeps it near the top.
        palette_dock := View {
            width: Fill height: Fill
            // Hidden WITH its child (sync_chrome toggles the wrapper): a
            // Fill/Fill overlay wrapper that stays visible would put a draw
            // area over the whole window even while `palette` is invisible,
            // and headless f1 showed the composer's send click never landing.
            visible: false
            align: Align{x: 0.5 y: 0.0}
        palette := RoundedView {
            width: 560 height: Fit flow: Down spacing: 4
            visible: false
            margin: Inset{top: 120}
            draw_bg +: {color: #FFFFFF border_radius: 12.0 border_size: 1.0 border_color: #E5E5E7}
            padding: Inset{left: 8 right: 8 top: 8 bottom: 8}
            palette_search := TextInput {
                width: Fill height: 34 text: ""
                empty_text: "/"
                draw_text.text_style.font_size: 13
            }
            palette_list := PortalList {
                width: Fill height: 204 flow: Down
                // A fixed 6×34 px viewport: a PortalList under a Fit-height
                // parent computes zero visible rows (the f3 captures showed the
                // search field and hint row with a blank gap between them).
                PaletteRowTpl := View {
                    width: Fill height: 34 flow: Right spacing: 8
                    padding: Inset{left: 6 top: 8}
                    palette_row_name := Label { width: 150 height: Fit text: "" draw_text.text_style.font_size: 13 }
                    palette_row_desc := Label { width: Fill height: Fit text: "" draw_text.text_style.font_size: 11 draw_text.color: #6E6E73 }
                }
            }
            Label {
                width: Fill height: Fit text: "↑↓ move · ↵ run · esc"
                draw_text.text_style.font_size: 10
                draw_text.color: #6E6E73
            }
        } // palette
        } // palette_dock

        // Card #28e item 6 (board 4 frame 4): the first-run screen. Before a
        // connection the window shows only a centered 480-wide card area (the
        // Connect card comes from board 2 mapping in Stage C). This replaces
        // `base` (`base.set_visible(false)` while `store.is_live()` is false).
        first_run := View {
            width: Fill height: Fill flow: Right
            visible: false
            draw_bg.color: #FFFFFF
            // Board 4 frame 4: the sidebar is still there but EMPTY — only
            // "OctosCode" and a grey "No threads yet".
            first_run_sidebar := View {
                width: 260 height: Fill flow: Down spacing: 6
                draw_bg.color: #F7F7F8
                padding: Inset{left: 12 right: 10 top: 12 bottom: 12}
                Label {
                    width: Fill height: Fit text: "OctosCode"
                    draw_text.text_style.font_size: 13
                }
                Label {
                    width: Fill height: Fit text: "No threads yet"
                    draw_text.text_style.font_size: 13
                    draw_text.color: #6E6E73
                }
            }
            first_run_rule := View {
                width: 1 height: Fill
                draw_bg.color: #E5E5E7
            }
            first_run_center := View {
                width: Fill height: Fill flow: Down
                align: Align{x: 0.5 y: 0.5}
            first_run_card := RoundedView {
                width: 480 height: Fit flow: Down spacing: 10
                draw_bg +: {color: #FFFFFF border_radius: 12.0 border_size: 1.0 border_color: #E5E5E7}
                padding: Inset{left: 24 right: 24 top: 24 bottom: 24}
                Label {
                    width: Fill height: Fit text: "Connect to Octos"
                    draw_text.text_style.font_size: 16
                }
                Label { width: Fill height: Fit text: "Server" draw_text.text_style.font_size: 11 draw_text.color: #6E6E73 }
                server_input := TextInput {
                    width: Fill height: 36 text: "http://127.0.0.1:50190"
                    draw_text.text_style.font_size: 13
                }
                Label { width: Fill height: Fit text: "Access token" draw_text.text_style.font_size: 11 draw_text.color: #6E6E73 }
                token_input := TextInput {
                    width: Fill height: 36 text: ""
                    empty_text: "••••••••"
                    draw_text.text_style.font_size: 13
                }
                connect_button := Button {
                    width: Fill height: 40 text: "Connect"
                    draw_bg.color: #000000
                    draw_bg.border_radius: 999.0
                }
                Label {
                    width: Fill height: Fit text: "Stored for this server only"
                    draw_text.text_style.font_size: 11
                    draw_text.color: #6E6E73
                }
                Label {
                    width: Fill height: Fit text: "Use local solo server"
                    draw_text.text_style.font_size: 13
                    draw_text.color: #2F6FEB
                }
            }
        }
        }
    }
}

/// What the UI thread needs to drive the conversation, behind one lock.
#[derive(Default)]
pub(crate) struct Bridge {
    /// The conversation (transport + store + flow UI). `None` until `start`.
    conv: Option<Arc<Conversation>>,
    /// The store, so a binding read still works before the flow exists.
    pub(crate) store: Arc<Store>,
    /// The flow UI, for the same reason.
    pub(crate) ui: Arc<Mutex<FlowUi>>,
}

/// Seed a store with `n` synthetic timeline rows and one session, for the
/// virtualization proof (`OCTOSCODE_SYNTHETIC_TIMELINE`). No transport: the
/// window draws the virtualized list on its own.
fn seed_synthetic(store: &Arc<Store>, n: usize) {
    use octoscode_store::Session;
    store.set_connection("Live".into(), false);
    store.set_sessions(vec![Session {
        id: "synthetic:main".into(),
        title: Some("Synthetic 2000".into()),
        message_count: n,
        updated_at: None,
        last_prompt: None,
        active_turn: false,
    }]);
    store.set_active(Some("synthetic:main".into()));
    // One TURN per row, each with its own user message: the rows are then
    // DISTINCT (`synthetic row #i`), so a scroll is visible in the `/snap`
    // bodies — which is what makes the virtualization proof checkable.
    let tl = &store.domains.session.timeline;
    for i in 0..n {
        tl.upsert_user_message(
            "synthetic:main",
            &format!("t{i}"),
            &format!("synthetic row #{i}"),
            serde_json::json!({}),
        );
    }
}

/// Card #28e — the board-4 fixture (`design/stage-a/desktop/atlas-prompt.md`):
/// workspace "octos", the five thread titles, one settled turn (the "Worked
/// for" row), a session goal, two loops and three fleet peers. Rendered by the
/// SAME store reads the live path uses (`sync_chrome` / `timeline_rows`) — a
/// deterministic capture seed: no transport and no clicks (the remote-click
/// pipeline is known-red: the A/B against fb9b8ce dropped the same clicks).
fn seed_synthetic_live(store: &Arc<Store>) {
    use octoscode_store::Session;
    use octoscode_store::domains::autonomy::{GoalRecord, GoalState, LoopRecord};
    use octoscode_store::domains::peer::Peer;
    use octoscode_store::timeline::EntryKind;

    store.set_connection("Live".into(), true);
    let titles = [
        "Fix steer queue drop on reconnect",
        "Add session fork",
        "Review PR #2566",
        "Bump octos-core to a6ea8505",
        "Why is hydrate slow?",
    ];
    store.set_sessions(
        titles
            .iter()
            .map(|t| Session {
                id: format!("board:{t}"),
                title: Some((*t).to_owned()),
                message_count: 2,
                updated_at: None,
                last_prompt: None,
                active_turn: false,
            })
            .collect(),
    );
    let first = format!("board:{}", titles[0]);
    store.set_active(Some(first.clone()));

    // The conversation column (board 4 frame 1): a user bubble, an answer
    // paragraph, a settled turn.
    let answer = "The queued steers were dropped because the input buffer was \
        cleared on reconnect; `steer_queue.rs` now re-drains the buffer after \
        the socket is re-established, so commands issued offline reach the turn.";
    let tl = &store.domains.session.timeline;
    tl.upsert_user_message(
        &first,
        "t1",
        "Fix the steer queue so queued steers survive a reconnect",
        serde_json::json!({}),
    );
    tl.append(
        &first,
        Some("t1".to_owned()),
        EntryKind::ASSISTANT_TEXT,
        answer.to_owned(),
    );
    tl.finalize_assistant(&first, "t1", answer);
    tl.close_turn(&first, "t1");
    store.domains.turn.started("t1");
    store.domains.turn.set_terminal("t1", "completed");

    // GOALS / LOOPS / FLEET (board 4 frame 3).
    store.domains.autonomy.set_goal(
        &first,
        GoalState {
            goal: Some(GoalRecord {
                goal_id: "g1".into(),
                objective: "Fix steer queue on reconnect".into(),
                status: "active".into(),
                token_budget: 0,
                tokens_used: 0,
                created_at_ms: 0,
                updated_at_ms: 0,
            }),
            ..Default::default()
        },
    );
    store.domains.autonomy.set_loops(vec![
        LoopRecord {
            loop_id: "l1".into(),
            session_id: first.clone(),
            profile_id: None,
            prompt: "Run CI smoke".into(),
            mode: "auto".into(),
            status: "active".into(),
            interval_seconds: Some(900),
            next_run_at_ms: None,
            expires_at_ms: 0,
            updated_at_ms: 0,
            fires: 0,
        },
        LoopRecord {
            loop_id: "l2".into(),
            session_id: first.clone(),
            profile_id: None,
            prompt: "Nightly review".into(),
            mode: "auto".into(),
            status: "paused".into(),
            interval_seconds: None,
            next_run_at_ms: None,
            expires_at_ms: 0,
            updated_at_ms: 0,
            fires: 0,
        },
    ]);
    for (name, topic) in [("tests", "Running"), ("docs", "Blocked"), ("review", "Done")] {
        let mut p = Peer::named(name);
        p.topic = Some(topic.into());
        p.origin_session_id = Some(first.clone());
        store.domains.peer.upsert(p);
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct OctoscodeView {
    #[deref]
    view: View,
    #[rust]
    bridge: Arc<Mutex<Bridge>>,
    #[rust]
    runtime: Option<tokio::runtime::Runtime>,
    #[rust]
    started: bool,
    /// The memoised per-item lowerings (a `PortalList` re-instantiates its
    /// visible rows every frame; without this the CPU re-lowers them each
    /// frame). See [`screen::Cache`].
    #[rust]
    cache: ScreenCache,
    /// Card #21b: the per-`Splash` mounted components. Each visible row's
    /// lowered DSL is evaluated in the app's VM and made the Splash's own
    /// `view` (`mount`), memoised by the widget uid so a `PortalList` that
    /// re-instantiates its rows every frame does not re-evaluate them.
    #[rust]
    mounts: ComponentMounts,
    /// The two virtualized lists' widget uids (0 = not captured yet), so
    /// `draw_walk` can tell which `PortalList` a draw step belongs to.
    #[rust]
    thread_uid: u64,
    #[rust]
    timeline_uid: u64,
    /// Card #28e — the command palette's `PortalList` uid (0 = not captured).
    #[rust]
    palette_uid: u64,
}

impl OctoscodeView {
    fn start(&mut self) {
        // The 2,000-entry synthetic timeline (the virtualization proof): no
        // transport at all — a store with 2,000 rows and a session, so the
        // window draws the virtualized list on its own (`/g` then shows the
        // timeline drawing ~20 items with a scroll range over 2,000).
        if let Ok(n) = std::env::var("OCTOSCODE_SYNTHETIC_TIMELINE") {
            let n: usize = n.parse().unwrap_or(2000);
            {
                let b = self.bridge.lock().unwrap();
                seed_synthetic(&b.store, n);
            }
            ::log::info!("[octoscode] synthetic timeline: {n} entries (no transport)");
            return;
        }
        // Card #28e — the board-4 capture seed: a LIVE-looking store so the
        // window draws the full base chrome (sidebar, conversation, panels)
        // with the board's own fixture rows. No transport.
        if std::env::var("OCTOSCODE_SYNTHETIC_LIVE").is_ok() {
            {
                let b = self.bridge.lock().unwrap();
                seed_synthetic_live(&b.store);
                let mut u = b.ui.lock().unwrap();
                u.begin_turn_now("t1");
                u.end_turn_now(true);
            }
            makepad_widgets::log!("[octoscode] synthetic live: board-4 seed (no transport)");
            return;
        }
        // Card #28e item 6 (board 4 frame 4): the first-run frame needs NO
        // connection, so `is_live()` stays false and the window shows only the
        // centered 480 px card. `OCTOSCODE_NO_CONNECT`/`OCTOSCODE_FIRST_RUN`
        // gate the transport off for that capture.
        if std::env::var("OCTOSCODE_NO_CONNECT").is_ok()
            || std::env::var("OCTOSCODE_FIRST_RUN").is_ok()
        {
            ::log::info!("[octoscode] first-run: no transport (board 4 frame 4)");
            return;
        }
        let base =
            std::env::var("OCTOS_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:50082".to_string());
        let bearer = std::env::var("OCTOS_BEARER").unwrap_or_default();
        let profile =
            std::env::var("OCTOS_PROFILE_ID").unwrap_or_else(|_| "octoscode".to_string());
        // The workspace cwd the web passes to `session/open` (`session-defaults.ts:5-7`).
        let cwd = std::env::var("OCTOS_WORKSPACE_CWD").ok();

        let runtime = match tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                ::log::error!("octoscode: runtime: {e}");
                return;
            }
        };

        let connected = {
            let _guard = runtime.enter();
            Conversation::connect(
                &base,
                &bearer,
                &profile,
                cwd.clone(),
                Some(Arc::new(|| SignalToUI::set_ui_signal())),
            )
        };

        let (conv, mut evt_rx) = match connected {
            Ok(pair) => pair,
            Err(e) => {
                ::log::error!("octoscode: connect: {e}");
                return;
            }
        };
        let conv = Arc::new(conv);

        {
            let mut b = self.bridge.lock().unwrap();
            b.store = conv.store.clone();
            b.ui = conv.ui();
            b.conv = Some(conv.clone());
        }

        // Drive the conversation: open the workspace, then drain events.
        let drv = conv.clone();
        runtime.spawn(async move {
            if let Err(e) = drv.open_workspace(cwd).await {
                ::log::error!("octoscode: session/open: {e}");
                SignalToUI::set_ui_signal();
                return;
            }
            // Optionally onboard a profile (the live gate's `profile/local/create`).
            if std::env::var("OCTOS_CREATE_PROFILE").is_ok() {
                if let Err(e) = drv.create_profile().await {
                    ::log::warn!("octoscode: profile/local/create: {e}");
                }
            }
            while let Some(evt) = evt_rx.recv().await {
                let e = drv.on_event(evt);
                ::log::debug!("[octoscode] {e:?}");
                SignalToUI::set_ui_signal();
            }
        });

        self.runtime = Some(runtime);
    }

    /// Run one binding action with the item index that emitted it (card #21 §3).
    /// The mapping is the pure [`actions::resolve`]; this only performs the
    /// resulting effect (off the UI thread).
    fn perform_action(&self, action: &str, index: usize) {
        let (store, ui, conv) = {
            let b = self.bridge.lock().unwrap();
            (b.store.clone(), b.ui.clone(), b.conv.clone())
        };
        let effect = {
            let ctx = bindings::Ctx::new(&store, &ui);
            actions::resolve(action, index, &ctx)
        };
        // UI-local effects need no runtime/transport.
        match &effect {
            actions::Effect::ToggleTool(key) => {
                let _ = ui.lock().map(|mut u| u.toggle_expanded(key));
                return;
            }
            // Card #28e — board-4 chrome toggles. Flip the FlowUi flag; the
            // next `sync_labels` moves it onto the view (`set_visible`).
            actions::Effect::UiChrome(which) => {
                if let Ok(mut u) = ui.lock() {
                    match which {
                        actions::UiChrome::ReviewToggle => {
                            u.toggle_review();
                        }
                        actions::UiChrome::SettingsToggle => {
                            u.toggle_settings();
                        }
                        actions::UiChrome::PaletteToggle => {
                            u.toggle_palette();
                        }
                    }
                }
                return;
            }
            actions::Effect::Unhandled(id) => {
                ::log::warn!("octoscode: unhandled action id {id:?}");
                return;
            }
            _ => {}
        }
        let Some(rt) = self.runtime.as_ref() else {
            return;
        };
        let Some(conv) = conv else {
            return;
        };
        match effect {
            actions::Effect::Refresh => {
                rt.spawn(async move {
                    if let Err(e) = conv.refresh_sessions().await {
                        ::log::warn!("octoscode: session.refresh: {e}");
                    }
                });
            }
            // Card #14 defect 4: "New chat" mints a FRESH session id, so a new
            // chat never reuses the previous run's context.
            actions::Effect::NewChat => {
                let cwd = std::env::var("OCTOS_WORKSPACE_CWD").ok();
                rt.spawn(async move {
                    match conv.new_chat(cwd).await {
                        Ok(id) => ::log::info!("octoscode: new chat opened {id}"),
                        Err(e) => ::log::warn!("octoscode: session.new: {e}"),
                    }
                });
            }
            actions::Effect::Submit => {
                rt.spawn(async move {
                    if let Err(e) = conv.submit_draft().await {
                        ::log::warn!("octoscode: composer.submit: {e}");
                    }
                });
            }
            actions::Effect::Steer(text) => {
                rt.spawn(async move {
                    if let Err(e) = conv.steer(&text).await {
                        ::log::warn!("octoscode: turn.steer: {e}");
                    }
                });
            }
            actions::Effect::Interrupt(turn) => {
                rt.spawn(async move {
                    if let Err(e) = conv.interrupt(&turn).await {
                        ::log::warn!("octoscode: turn.interrupt: {e}");
                    }
                });
            }
            actions::Effect::Open(session) => {
                let cwd = std::env::var("OCTOS_WORKSPACE_CWD").ok();
                rt.spawn(async move {
                    match conv.open_session(&session, cwd).await {
                        Ok(id) => ::log::info!("octoscode: thread.open opened {id}"),
                        Err(e) => ::log::warn!("octoscode: thread.open: {e}"),
                    }
                });
            }
            // Handled above / needs `cx` (copy).
            actions::Effect::ToggleTool(_)
            | actions::Effect::Unhandled(_)
            | actions::Effect::UiChrome(_)
            | actions::Effect::CopyAnswer => {}
        }
    }

    fn sync_labels(&mut self, cx: &mut Cx) {
        // Header labels only. The two virtualized lists are drawn in `draw_walk`
        // (a `PortalList` instantiates its visible items each frame — that is the
        // virtualization: a 2,000-entry timeline builds ~20 rows, not 2,000).
        let (status, sessions) = {
            let b = self.bridge.lock().unwrap();
            (b.store.summary(), b.store.session_count())
        };
        let text = format!("conn: {}", status.trim_start_matches("conn: "));
        self.view.label(cx, ids!(status)).set_text(cx, &text);
        self.view
            .label(cx, ids!(sessions))
            .set_text(cx, &format!("sessions: {sessions}"));
        // The #16 `composer` component is the dock's look. It is NOT virtualized
        // (one instance), so it is lowered here rather than in `draw_walk`; its
        // two live slots are the draft and the idle placeholder. Card #21b: it is
        // MOUNTED (evaluated in our VM + `mem::replace` + deep insert), not
        // `set_text` — the latter mints a standalone tree that never seats.
        let bridge = self.bridge.clone();
        let composer_live = {
            let b = self.bridge.lock().unwrap();
            let ctx = bindings::Ctx::new(&b.store, &b.ui);
            bindings::query(&ctx, "turn.active")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        };
        let composer = {
            let mut cache = std::mem::take(&mut self.cache);
            let c = cache
                .lower(&bridge, components::ItemKind::Composer, 0, None)
                .unwrap_or_default();
            self.cache = cache;
            c
        };
        // Card #21f item 1b: while a turn runs the SAME dock is the STOP control
        // (atlas conversation-08 `stop2`, a white 12×12 rounded square on the flat
        // black disc). The composer component carries one send glyph, so swap it
        // for the stop asset — otherwise the arrow persists through the whole
        // running turn (`g3b-turn2-running.png`). The mount cache compares the DSL
        // string, so the swap also forces exactly one repaint when `turn.active`
        // flips either way.
        let composer = if composer_live {
            composer
                .replace("icon_send-3fe1783d764e.svg", "icon_stop.svg")
                .replace("icon_send.svg", "icon_stop.svg")
        } else {
            composer
        };
        let composer_splash = self.view.splash(cx, ids!(composer_splash));
        if let Err(e) = self.mounts.mount(cx, &composer_splash, &composer) {
            makepad_widgets::log!("[octoscode] composer mount: {e}");
        }
        // Card #21d item 5: the `new-chat` component (#16) is the thread
        // column's first row. The row existed but was never mounted, so it
        // rendered as an empty pill (no label, no compose icon).
        let new_chat = {
            let mut cache = std::mem::take(&mut self.cache);
            let c = cache
                .lower(&bridge, components::ItemKind::NewChat, 0, None)
                .unwrap_or_default();
            self.cache = cache;
            c
        };
        let new_chat_splash = self.view.splash(cx, ids!(new_chat_splash));
        if let Err(e) = self.mounts.mount(cx, &new_chat_splash, &new_chat) {
            makepad_widgets::log!("[octoscode] new-chat mount: {e}");
        }
        self.sync_chrome(cx);
        ::log::info!("[octoscode] {text} | sessions: {sessions}");
    }

    /// Card #28e — move the chrome state (FlowUi flags + store) onto the view:
    /// the first-run swap, the review panel / settings drawer / palette /
    /// dimmer visibility, and the GOALS/LOOPS/FLEET sidebar sections.
    fn sync_chrome(&mut self, cx: &mut Cx) {
        // Card #28e — the headless-capture gate: `OCTOSCODE_CHROME=review|
        // settings|palette` pre-opens that surface deterministically (a
        // headless run cannot click the toggle). Parsed once per process.
        let (env_review, env_settings, env_palette) = chrome_env();
        let (live, review, settings, palette) = {
            let b = self.bridge.lock().unwrap();
            (
                b.store.is_live(),
                b.ui.lock().map(|u| u.review_open()).unwrap_or(false) || env_review,
                b.ui.lock().map(|u| u.settings_open()).unwrap_or(false) || env_settings,
                b.ui.lock().map(|u| u.palette_open()).unwrap_or(false) || env_palette,
            )
        };
        // First run (board 4 frame 4): before a connection the window shows
        // only the centered card area; the base chrome is hidden.
        self.view.widget(cx, ids!(base)).set_visible(cx, live);
        self.view.widget(cx, ids!(first_run)).set_visible(cx, !live);
        self.view.widget(cx, ids!(review_panel)).set_visible(cx, review);
        self.view.widget(cx, ids!(settings_drawer)).set_visible(cx, settings);
        self.view.widget(cx, ids!(palette)).set_visible(cx, palette);
        // The dock wrapper hides with the palette (a visible Fill/Fill overlay
        // would shadow the composer's buttons even with the card invisible).
        self.view.widget(cx, ids!(palette_dock)).set_visible(cx, palette);
        self.view.widget(cx, ids!(dimmer)).set_visible(cx, palette || settings);

        // GOALS / LOOPS / FLEET rows (board 4 frame 3): visible only when the
        // session has them.
        let (goals, loops, fleet) = {
            let b = self.bridge.lock().unwrap();
            let session = b.store.active_session();
            let goal = session
                .as_deref()
                .and_then(|s| b.store.domains.autonomy.goal(s));
            let goal_txt = goal
                .map(|g| format!("{} · {}", g.objective, g.status))
                .unwrap_or_default();
            let loops: Vec<String> = b
                .store
                .domains
                .autonomy
                .loops()
                .into_iter()
                .map(|l| match l.interval_seconds {
                    // Board 4 frame 3: "Run CI smoke · every 15 min"; a loop
                    // without a cadence shows its status (paused).
                    Some(s) if s % 60 == 0 => format!("{} · every {} min", l.prompt, s / 60),
                    _ => format!("{} · {}", l.prompt, l.status),
                })
                .collect();
            let fleet: Vec<String> = b
                .store
                .domains
                .peer
                .list()
                .into_iter()
                .map(|p| {
                    // Board 4 frame 3: "tests · Running" — the peer's topic
                    // when it has one, else the open/closed state.
                    let state = p.topic.clone().unwrap_or_else(|| {
                        if p.closed { "closed".into() } else { "open".into() }
                    });
                    format!("{} · {}", p.name, state)
                })
                .collect();
            (goal_txt, loops, fleet)
        };
        let any = !goals.is_empty() || !loops.is_empty() || !fleet.is_empty();
        self.view.widget(cx, ids!(autonomy_sections)).set_visible(cx, any);
        self.view.label(cx, ids!(goal_row_1)).set_text(cx, &goals);
        let mut set_rows = |ids: &[LiveId], rows: &[String]| {
            for (i, id) in ids.iter().enumerate() {
                let txt = rows.get(i).cloned().unwrap_or_default();
                self.view.label(cx, &[*id]).set_text(cx, &txt);
            }
        };
        set_rows(&[live_id!(loop_row_1), live_id!(loop_row_2)], &loops);
        set_rows(
            &[live_id!(fleet_row_1), live_id!(fleet_row_2), live_id!(fleet_row_3)],
            &fleet,
        );
    }

}

/// Card #28e — the `OCTOSCODE_CHROME` capture gate, parsed once.
fn chrome_env() -> (bool, bool, bool) {
    static CHROME: std::sync::OnceLock<(bool, bool, bool)> = std::sync::OnceLock::new();
    *CHROME.get_or_init(|| {
        let v = std::env::var("OCTOSCODE_CHROME")
            .unwrap_or_default()
            .to_lowercase();
        (v == "review", v == "settings", v == "palette")
    })
}

impl Widget for OctoscodeView {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // The two lists share makepad's default item template, so a draw step's
        // widget uid is how it says WHICH list it is (captured once).
        if self.thread_uid == 0 {
            self.thread_uid = self.view.portal_list(cx, ids!(thread_list)).widget_uid().0;
            self.timeline_uid = self.view.portal_list(cx, ids!(timeline_list)).widget_uid().0;
            self.palette_uid = self.view.portal_list(cx, ids!(palette_list)).widget_uid().0;
        }
        let (thread_uid, timeline_uid, palette_uid) =
            (self.thread_uid, self.timeline_uid, self.palette_uid);
        // Both the lowering cache and the mount cache are taken OUT of self so the
        // loop body borrows only `bridge` (a local Arc) — `self.view.draw_walk`
        // already holds `self.view`.
        let mut cache = std::mem::take(&mut self.cache);
        let mut mounts = std::mem::take(&mut self.mounts);
        let bridge = self.bridge.clone();

        while let Some(step) = self.view.draw_walk(cx, scope, walk).step() {
            if let Some(mut list) = step.as_portal_list().borrow_mut() {
                let uid = list.widget_uid().0;
                if uid == thread_uid {
                    // The LEFT column: one `thread-row` component per session.
                    let rows = {
                        let b = bridge.lock().unwrap();
                        screen::thread_rows(&b.store)
                    };
                    list.set_item_range(cx, 0, rows.len());
                    while let Some(id) = list.next_visible_item(cx) {
                        let Some(_row) = rows.get(id) else { continue };
                        let item = list.item(cx, id, id!(ThreadRowTpl));
                        let body = cache
                            .lower(&bridge, components::ItemKind::ThreadRow, id, None)
                            .unwrap_or_default();
                        let splash = item.splash(cx, ids!(thread_splash));
                        if let Err(e) = mounts.mount(cx, &splash, &body) {
                            makepad_widgets::log!("[octoscode] thread-row mount: {e}");
                        }
                        item.draw_all_unscoped(cx);
                    }
                } else if uid == timeline_uid {
                    // The CENTER column: one component per timeline entry, in
                    // display order (`screen::timeline_rows`).
                    let (live, rows) = {
                        let b = bridge.lock().unwrap();
                        let ctx = bindings::Ctx::new(&b.store, &b.ui);
                        let live = bindings::query(&ctx, "turn.active")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        let rows = screen::timeline_rows(&b.store, live);
                        (live, rows)
                    };
                    let _ = live;
                    list.set_item_range(cx, 0, rows.len());
                    while let Some(id) = list.next_visible_item(cx) {
                        let Some(row) = rows.get(id) else { continue };
                        let item = list.item(cx, id, id!(TimelineItemTpl));
                        // Card #21c item 2: no native kind label on screen; the
                        // kind is carried by the component's own node ids in `/g`.
                        let body = cache
                            .lower(&bridge, row.kind, row.index, row.turn.as_deref())
                            .unwrap_or_default();
                        let splash = item.splash(cx, ids!(item_splash));
                        if let Err(e) = mounts.mount(cx, &splash, &body) {
                            makepad_widgets::log!("[octoscode] {} mount: {e}", row.kind.id());
                        }
                        item.draw_all_unscoped(cx);
                    }
                } else if uid == palette_uid {
                    // Card #28e — the command palette's rows: (monospace) name
                    // + grey description, first row highlighted by the shell's
                    // hover state.
                    let rows = palette_commands();
                    list.set_item_range(cx, 0, rows.len());
                    while let Some(id) = list.next_visible_item(cx) {
                        let Some((name, desc)) = rows.get(id) else { continue };
                        let item = list.item(cx, id, id!(PaletteRowTpl));
                        item.label(cx, ids!(palette_row_name)).set_text(cx, name);
                        item.label(cx, ids!(palette_row_desc)).set_text(cx, desc);
                        item.draw_all_unscoped(cx);
                    }
                }
            }
        }
        self.cache = cache;
        self.mounts = mounts;
        DrawStep::done()
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        if !self.started {
            self.started = true;
            self.start();
            self.sync_labels(cx);
        }
        match event {
            Event::Signal => self.sync_labels(cx),
            Event::Actions(actions) => {
                // Card #21c item 5: the ONE composer is the mounted #16
                // component. Its own input is a real `TextInput` (id
                // `i0_composer_0` inside the mounted tree); its `changed` action
                // is the `composer.draft` binding (the component's own
                // `service-actions.json` declares exactly that behavior).
                if let Some(text) = self
                    .view
                    .text_input(cx, &[live_id!(i0_composer_0)])
                    .changed(actions)
                {
                    // Card #28e item 5: "/" typed into an EMPTY composer opens
                    // the command palette (board 4 frame 3).
                    if text == "/" {
                        if let Ok(mut u) = self.bridge.lock().unwrap().ui.lock() {
                            u.set_palette_open(true);
                        }
                    }
                    self.bridge.lock().unwrap().ui.lock().unwrap().set_draft_inner(text.clone());
                    makepad_widgets::log!("[octoscode] draft synced: {} chars", text.len());
                }
                if self.view.button(cx, ids!(refresh)).clicked(actions) {
                    self.perform_action("session.refresh", 0);
                }
                // The #16 `new-chat` component overlaid by a host hit target.
                if self.view.button(cx, ids!(new_chat_hit)).clicked(actions) {
                    self.perform_action(bindings::ACTION_NEW_CHAT, 0);
                }
                // The composer's send control. While a turn is running the same
                // control is STOP (scene 08 / Codex) and sends `turn/interrupt`
                // (the L1 fix); otherwise it submits the draft.
                if self.view.button(cx, ids!(send_hit)).clicked(actions) {
                    makepad_widgets::log!("[octoscode] send_hit clicked");
                    let live = {
                        let b = self.bridge.lock().unwrap();
                        let ctx = bindings::Ctx::new(&b.store, &b.ui);
                        bindings::query(&ctx, "turn.active")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false)
                    };
                    if live {
                        self.perform_action(bindings::ACTION_INTERRUPT, 0);
                    } else {
                        self.perform_action(bindings::ACTION_SUBMIT, 0);
                    }
                }
                // `+` (attach) and the mic are not wired to a protocol method
                // yet; they are present as hit targets so the component's own
                // chrome stays clickable and the ids exist for a later card.
                // Card #21 §3 — the per-item controls. A row click is routed
                // WITH its item id (the same `items_with_actions` contract the
                // makepad examples use).
                let thread_list = self.view.portal_list(cx, ids!(thread_list));
                for (item_id, item) in thread_list.items_with_actions(actions) {
                    if item.button(cx, ids!(row_hit)).clicked(actions) {
                        self.perform_action("thread.open", item_id);
                    }
                }
                let timeline_list = self.view.portal_list(cx, ids!(timeline_list));
                let (live, rows) = {
                    let b = self.bridge.lock().unwrap();
                    let ctx = bindings::Ctx::new(&b.store, &b.ui);
                    let live = bindings::query(&ctx, "turn.active")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let rows = screen::timeline_rows(&b.store, live);
                    (live, rows)
                };
                let _ = live;
                for (item_id, item) in timeline_list.items_with_actions(actions) {
                    let Some(row) = rows.get(item_id) else { continue };
                    if !item.button(cx, ids!(row_hit)).clicked(actions) {
                        continue;
                    }
                    // Each row kind owns a different control id.
                    let control = match row.kind {
                        components::ItemKind::ToolCell => "expand",
                        components::ItemKind::AnswerActions => "copy",
                        _ => continue,
                    };
                    if let Some(action) = components::action_for(row.kind, control) {
                        if action == "tool.toggle" {
                            self.perform_action(action, row.index);
                        } else {
                            self.perform_action(action, 0);
                        }
                    }
                }
                // Card #28e — the board-4 chrome controls.
                if self.view.button(cx, ids!(review_toggle_hit)).clicked(actions)
                    || self.view.button(cx, ids!(review_close)).clicked(actions)
                {
                    self.perform_action("review.toggle", 0);
                }
                if self.view.button(cx, ids!(settings_close)).clicked(actions) {
                    self.perform_action("settings.toggle", 0);
                }
                self.sync_labels(cx);
            }
            // Card #28e — board-4 keys. Esc closes the palette (frame 3);
            // Cmd+K toggles it.
            Event::KeyDown(e) => {
                let ui = self.bridge.lock().unwrap().ui.clone();
                let mut open_changed = false;
                if let Ok(mut u) = ui.lock() {
                    if e.key_code == KeyCode::Escape && u.palette_open() {
                        u.set_palette_open(false);
                        open_changed = true;
                    } else if e.key_code == KeyCode::KeyK && e.modifiers.logo {
                        u.toggle_palette();
                        open_changed = true;
                    } else if e.key_code == KeyCode::KeyE && e.modifiers.logo {
                        u.toggle_review();
                        open_changed = true;
                    } else if e.key_code == KeyCode::Period && e.modifiers.logo {
                        u.toggle_settings();
                        open_changed = true;
                    // Card #28e item 5: typing "/" in an EMPTY composer opens
                    // the palette (board 4 frame 3). A non-empty draft keeps
                    // the "/" as text.
                    } else if e.key_code == KeyCode::Slash
                        && !u.palette_open()
                        && u.draft().is_empty()
                    {
                        u.set_palette_open(true);
                        open_changed = true;
                    }
                }
                if open_changed {
                    self.sync_labels(cx);
                }
            }
            _ => {}
        }
    }
}

pub struct OctoscodeModule;
pub static OCTOSCODE_MODULE: OctoscodeModule = OctoscodeModule;

/// Card #28e — the board-4 command palette rows (frame 3): monospace name +
/// grey description. Static shell data for now; the actions they route land
/// with the Stage-C command surface.
fn palette_commands() -> Vec<(&'static str, &'static str)> {
    vec![
        ("/model", "Switch model"),
        ("/monitor", "Add a monitor"),
        ("/mode", "Change permissions"),
        ("/compact", "Compact context"),
        ("/btw", "Ask a side question"),
        ("/resume", "Resume a session"),
    ]
}

impl AppModule for OctoscodeModule {
    fn id(&self) -> &'static str {
        "octoscode"
    }
    fn label(&self) -> &'static str {
        "OctosCode"
    }
    fn register(&self, vm: &mut ScriptVm) {
        script_mod(vm);
        // Card #21b: the design/kit vocabulary every lowered #16 component names
        // (`DesignSurface`, `KitButton`, …) must be in THIS VM — the isolate the
        // shell hosts the module in (`module_host.rs:133` allocates it, then
        // calls `register(vm)`/`create(vm, ..)` inside it). `register_vocabulary`
        // below uses `register_splash_isolate_mod`, which only reaches isolates
        // allocated AFTER it, so it cannot reach ours; register directly, exactly
        // as `beauty-host` does in the App's own `script_mod`
        // (`beauty.rs:356-364`).
        octoscript_widgets::design::script_mod(vm);
        octoscript_widgets::kit::script_mod(vm);
        // Card #15b: the same vocabulary, process-wide, for any OTHER isolate
        // (the tests and the card probes lower there).
        l0_host::register_vocabulary();
        // Card #21b: log what the RUNNING app resolves at startup
        // (`id -> path -> on-disk|placeholder`), so a capture's `/log` proves
        // which components root the launched process used — the test harness
        // passing from the repo root proved nothing.
        components::log_resolutions();
    }
    fn open_schema(&self) -> OpenSchema {
        OpenSchema::new(1)
    }
    fn capabilities(&self) -> &'static [&'static str] {
        &[]
    }
    fn create(
        &self,
        vm: &mut ScriptVm,
        _open: ValidatedOpen,
        _handles: InstanceHandles,
    ) -> InstanceParts {
        let value = script_eval!(vm, {
            use mod.widgets.*
            OctoscodeView {}
        });
        let root = WidgetRef::script_from_value(vm, value);
        // Inject the bridge into the widget's `#[rust]` field.
        let bridge = Arc::new(Mutex::new(Bridge {
            conv: None,
            store: Arc::new(Store::new()),
            ui: Arc::new(Mutex::new(FlowUi::default())),
        }));
        if let Some(mut view) = root.borrow_mut::<OctoscodeView>() {
            view.bridge = bridge;
        }
        InstanceParts {
            root,
            executor: Box::new(OctoscodeExecutor),
            shutdown: Box::new(|_| {}),
        }
    }
}

struct OctoscodeExecutor;
impl ServiceExecutor for OctoscodeExecutor {
    fn manifest(&self) -> ServiceManifest {
        ServiceManifest::new("octoscode", "OctosCode", "Native octoscode app (AppModule).")
    }
    fn execute(&mut self, _cx: &mut Cx, call: &ServiceCall) -> ExecOutcome {
        ExecOutcome::Done(ToolResult::unavailable(
            &call.call_id,
            "OctosCode (native module) has no tools",
        ))
    }
}
