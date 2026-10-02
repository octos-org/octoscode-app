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
// A3: the board-2 chrome (sidebar body, header, Settings, Stop confirm).
pub mod chrome;
pub mod components;
pub mod design;
pub mod fallback;
pub mod l0_host;
pub mod flow;
pub mod mount;
pub mod screen;
pub mod screens;

use flow::{Conversation, FlowUi};
// A top-level `::` path is not a `#[rust]` field type the `Script` derive's
// parser accepts (its `eat_type` reads one ident + optional generics), so the
// cache types are aliased to bare idents here.
use mount::MountCache as ComponentMounts;
use screen::Cache as ScreenCache;
use chrome::ChromeRuntime;

/// #P4h1 row 306 — install the recents store and purge the legacy v1 cache at
/// startup (the web's App.tsx:1019-1023). This is the production caller that
/// keeps `clear_recent_workspaces` off the test-only list (RULES 3); an honest
/// failure is logged, never silently swallowed.
///
/// #M1: this MUST NOT live in the `script_mod!` body. A bare Rust block there is
/// not DSL, so the script parser choked on the `:` of the `::log::warn!` path
/// and `mod.widgets.OctoscodeView` never bound ("variable OctoscodeView not
/// found in scope", `SplashVmId(1)`) — the mounted card rendered 0 non-zero
/// rects. Wrapping it in `#{ … }` does not help either: `script_mod!` splices a
/// script EXPRESSION, so a block's unit value has no `script_to_value`. A Rust
/// call belongs in a Rust fn, invoked from Rust.
fn init_recents_persistence() {
    if !screens::recents::init_persistence() {
        ::log::warn!("octoscode: workspace recents: legacy v1 cache could not be purged");
    }
}

script_mod! {
    use mod.prelude.widgets.*
    // #31d — assign the theme roles BEFORE this class body dereferences any
    // `theme.*` ref (class defaults capture at evaluation; the wm_theme bridge
    // documents the same ordering constraint). Loads the persisted preference.
    #(screens::theme::eval_roles(vm))
    // #P4h1 row 306 — install the recents store and purge the legacy v1 cache
    // at startup (the web's App.tsx:1019-1023); the warn lives in the helper.
    // The production caller that keeps `clear_recent_workspaces` off the
    // test-only list (RULES 3); an honest failure is logged, never silently
    // swallowed. #FX1 portability: this was a bare `{ if .. ::log::warn! }`
    // block, which the script parser reads as SCRIPT tokens (IfTest /
    // EmitUnary / Operator(:)) and fails to parse — the app came up a
    // 46-widget husk with 11 [E] DSL errors and no screen_dock
    // (fx1-evidence/walk-conv.log). The `#(call)` splice is the one shape
    // both dialects parse (the eval_roles precedent above).
    #(screens::recents::init_persistence_logged())
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
        // A3: no gap — the conversation header bar is the window's top edge.
        flow: Down spacing: 0
        // Card #21c item 7: the debug header is GONE from the visible UI. The
        // connection state / session count stay as 1px labels so `/g` still
        // carries them and `sync_labels` keeps a target (board: "connection
        // state can go to /g metadata").
        header_meta := View {
            width: 0 height: 0 flow: Right
            status := Label { width: 0 height: 0 draw_text.text_style.font_size: 1 text: ""  draw_text.color: theme.color_text_muted}
            sessions := Label { width: 0 height: 0 draw_text.text_style.font_size: 1 text: ""  draw_text.color: theme.color_text_muted}
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
            flow: Right spacing: 0

            // A3 (board 2): the sidebar paints in `sidebar_dock` (a left
            // overlay, below) so the SAME widget is the desktop column AND
            // the phone drawer; at >= 760 px this spacer reserves its room
            // (280 + the 1 px rule), the web's 280 px `.root`.
            sidebar_spacer := View {
                width: 281 height: Fill
            }

            conversation_column := View {
                width: Fill height: Fill flow: Down spacing: 6
                // Card #28e item 2 (board 4): the conversation column is centered
                // with a max width of 720 px. `align.x: 0.5` centers the
                // `max_width: 720` child inside the Fill column.
                align: Align{x: 0.5 y: 0.0}
                // A3: the conversation header (the web's 52 px
                // `.conversation-header`): menu trigger (compact), title,
                // Review + Settings — plus the held banner and the new-chat
                // defaults strip under it (board 2 screens 10/12).
                oc_header := mod.widgets.OcHeaderBar {}
                conversation_inner := View {
                    width: Fill height: Fill flow: Down spacing: 6
                    max_width: 720
                    // #28e2 item 2: the OctoSense dock floats over the window's
                    // bottom ~90px (measured dock top y≈810 at 1440×900), which
                    // cut the composer's control row (+ · Ask for approval ·
                    // model · mic · send) off the captures. Reserve that strip
                    // so the whole composer card is visible at 1440×900 and
                    // 900×800 alike.
                    // #38c (backlog 80b9f33): the shell hands the module a
                    // window that ends AT the screen's right edge (measured:
                    // window [54,76,846,603] in a 900-wide scene — 54 px left
                    // margin, 0 right), so a full-width bubble row and the
                    // right-aligned `now` timestamp ended at x=900, clipped by
                    // the screen. The window rect is the shell's; the CONTENT
                    // must fit it: a right inset keeps the bubble and the
                    // timestamp inside the column with a margin
                    // (bubble right ≤ column right − 12, asserted by the walk
                    // check `the conversation content fits…`).
                    padding: Inset{bottom: 96 right: 16}
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
                        // #P4a1 — the approval pill's hit target (the
                        // component's pill art sits at left 49 / top 121,
                        // 122x46 in the composer artboard): click cycles the
                        // session's permission mode on the wire.
                        approval_pill_hit := Button {
                            width: 122 height: 46 text: ""
                            margin: Inset{left: 49.0 top: 121.0}
                            draw_bg.color: #00000000
                            draw_bg.color_hover: #00000010
                            draw_bg.color_down: #00000020
                            draw_bg.border_size: 0.0
                            draw_bg.color_2: #00000000
                            draw_bg.border_color: #00000000
                            draw_bg.border_color_2: #00000000
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

            // #28e3 item 1: flow spacers — at wide windows they reserve the
            // docked panels' room (the panels themselves paint in right-aligned
            // overlay docks exactly over the spacer); on narrow windows the
            // spacers hide and the panels overlay with the dimmer, so the
            // center keeps its 420px minimum. The #29d screen mount slot moved
            // to the first-run card area (#28e4): the #28e shell owns the
            // review panel now, and the entry mounts the screens there.
            columns_review_spacer := View {
                width: 560 height: Fill
                visible: false
            }
            columns_settings_spacer := View {
                width: 420 height: Fill
                visible: false
            }
        } // columns

        } // base

        // A3 (board 2 screen 5): the compact drawer's scrim — the web's
        // NavigationSurface backdrop (closeOnBackdrop). Shown only while the
        // drawer is open below 760 px; a tap closes it.
        drawer_scrim := View {
            width: Fill height: Fill flow: Overlay
            visible: false
            SolidView { width: Fill height: Fill draw_bg.color: #1D1D1F59 }
            drawer_scrim_hit := mod.widgets.OcBackdrop {}
        }
        // A3 (board 2 screens 1-5): the product sidebar. One widget, two
        // seats: at >= 760 px a 280 px column over `sidebar_spacer`; below,
        // the drawer (min(320, w - 48), the web's `.drawer`) over the scrim.
        sidebar_dock := View {
            width: Fit height: Fill flow: Right
            visible: false
            threads_column := RoundedView {
                width: 280 height: Fill flow: Down spacing: 0
                padding: Inset{left: 10 right: 10 top: 6 bottom: 8}
                draw_bg +: {color: theme.color_bg_app border_radius: 1.0}
                // The brand row (the board's "OctosCode"); the drawer adds
                // its close button on the right.
                sidebar_header := View {
                    width: Fill height: 46 flow: Right align: Align{y: 0.5}
                    padding: Inset{left: 8}
                    Label {
                        width: Fill height: Fit text: "OctosCode" padding: 0
                        draw_text.text_style: theme.oc_text_brand
                        draw_text.color: theme.color_fg_app
                    }
                    // Desktop: collapse to the rail (the web's sidebar
                    // toggle, ProductSidebar.tsx:532).
                    sidebar_collapse_slot := View {
                        width: 32 height: 32 flow: Overlay align: Align{x: 0.5 y: 0.5}
                        Svg {
                            width: 17 height: 17
                            animating: false
                            draw_svg.svg: file_resource(#(crate::chrome::icon("panel_muted")))
                            draw_svg.preserve_viewbox: true
                        }
                        sidebar_collapse := mod.widgets.OcHitRound {}
                    }
                    drawer_close_slot := View {
                        width: 32 height: 32 flow: Overlay align: Align{x: 0.5 y: 0.5}
                        visible: false
                        Svg {
                            width: 16 height: 16
                            animating: false
                            draw_svg.svg: file_resource(#(crate::chrome::icon("x_fg")))
                            draw_svg.preserve_viewbox: true
                        }
                        drawer_close := mod.widgets.OcHitRound {}
                    }
                }
                // The collapsed rail (desktop): chrome.rs `OcSidebarRail`.
                oc_sidebar_rail := mod.widgets.OcSidebarRail { visible: false }
                // New chat, Search chats, By workspace | All + Recent, and
                // the tree (`thread_list`): chrome.rs `OcSidebarBody`.
                oc_sidebar_body := mod.widgets.OcSidebarBody {}
                // Card #28e item 1: GOALS / LOOPS / FLEET appear only when the
                // session has them (board 4 frame 3), above the footer.
                autonomy_sections := View {
                    width: Fill height: Fit flow: Down spacing: 4 visible: false
                    Label {
                        width: Fill height: Fit text: "GOALS"
                        draw_text.text_style: theme.oc_text_caps
                        draw_text.color: theme.color_text_muted
                    }
                    goals_list := View {
                        width: Fill height: Fit flow: Down spacing: 2
                        // #28e2 item 3: the goal row carries the board's
                        // progress ring; rows ellipsize with "…" instead of
                        // clipping mid-word.
                        goal_wrap_1 := View {
                            width: Fill height: 28 flow: Right spacing: 6 align: Align{y: 0.5}
                            goal_ring := Svg {
                                width: 14 height: 14
                                animating: false
                                draw_svg.svg: file_resource(#(crate::design::icon_resource("icon_ring.svg")))
                                draw_svg.preserve_viewbox: true
                            }
                            goal_row_1 := Label {
                                width: Fill height: Fit text: ""
                                draw_text.text_style: theme.oc_text_row
                                max_lines: 1 text_overflow: TextOverflow.Ellipsis
                             draw_text.color: theme.color_fg_app}
                        }
                        goal_row_2 := Label {
                            width: Fill height: 28 text: ""
                            draw_text.text_style: theme.oc_text_row
                            max_lines: 1 text_overflow: TextOverflow.Ellipsis
                         draw_text.color: theme.color_fg_app}
                    }
                    Label {
                        width: Fill height: Fit text: "LOOPS"
                        draw_text.text_style: theme.oc_text_caps
                        draw_text.color: theme.color_text_muted
                    }
                    loops_list := View {
                        width: Fill height: Fit flow: Down spacing: 2
                        loop_row_1 := Label {
                            width: Fill height: 28 text: ""
                            draw_text.text_style: theme.oc_text_row
                            max_lines: 1 text_overflow: TextOverflow.Ellipsis
                         draw_text.color: theme.color_fg_app}
                        loop_row_2 := Label {
                            width: Fill height: 28 text: ""
                            draw_text.text_style: theme.oc_text_row
                            max_lines: 1 text_overflow: TextOverflow.Ellipsis
                         draw_text.color: theme.color_fg_app}
                    }
                    Label {
                        width: Fill height: Fit text: "FLEET"
                        draw_text.text_style: theme.oc_text_caps
                        draw_text.color: theme.color_text_muted
                    }
                    fleet_list := View {
                        width: Fill height: Fit flow: Down spacing: 2
                        // #28e3 item 2: every fleet row reserves the SAME
                        // 14px status slot, so all rows share one text inset
                        // (the #28e2 dots collapsed when hidden and pushed
                        // only the Blocked row's text). The dot itself is a
                        // RoundedView — a plain View never paints draw_bg —
                        // and sync_labels shows it on the Blocked row only.
                        fleet_wrap_1 := View {
                            width: Fill height: 28 flow: Right spacing: 6 align: Align{y: 0.5}
                            fleet_slot_1 := View {
                                width: 14 height: 14 flow: Overlay
                                fleet_dot_1 := RoundedView {
                                    width: 8 height: 8
                                    margin: Inset{left: 3 top: 3}
                                    draw_bg +: {color: #E5B800 border_radius: 4.0}
                                    visible: false
                                }
                            }
                            fleet_row_1 := Label {
                                width: Fill height: Fit text: ""
                                draw_text.text_style: theme.oc_text_row
                                max_lines: 1 text_overflow: TextOverflow.Ellipsis
                             draw_text.color: theme.color_fg_app}
                        }
                        fleet_wrap_2 := View {
                            width: Fill height: 28 flow: Right spacing: 6 align: Align{y: 0.5}
                            fleet_slot_2 := View {
                                width: 14 height: 14 flow: Overlay
                                fleet_dot_2 := RoundedView {
                                    width: 8 height: 8
                                    margin: Inset{left: 3 top: 3}
                                    draw_bg +: {color: #E5B800 border_radius: 4.0}
                                    visible: false
                                }
                            }
                            fleet_row_2 := Label {
                                width: Fill height: Fit text: ""
                                draw_text.text_style: theme.oc_text_row
                                max_lines: 1 text_overflow: TextOverflow.Ellipsis
                             draw_text.color: theme.color_fg_app}
                        }
                        fleet_wrap_3 := View {
                            width: Fill height: 28 flow: Right spacing: 6 align: Align{y: 0.5}
                            fleet_slot_3 := View {
                                width: 14 height: 14 flow: Overlay
                                fleet_dot_3 := RoundedView {
                                    width: 8 height: 8
                                    margin: Inset{left: 3 top: 3}
                                    draw_bg +: {color: #E5B800 border_radius: 4.0}
                                    visible: false
                                }
                            }
                            fleet_row_3 := Label {
                                width: Fill height: Fit text: ""
                                draw_text.text_style: theme.oc_text_row
                                max_lines: 1 text_overflow: TextOverflow.Ellipsis
                             draw_text.color: theme.color_fg_app}
                        }
                    }
                }
                // + Add workspace (chrome.rs `OcSidebarFoot`).
                oc_sidebar_foot := mod.widgets.OcSidebarFoot {}
            }
            // The 1 px hairline between the sidebar and the conversation.
            sidebar_rule := SolidView {
                width: 1 height: Fill
                draw_bg.color: theme.color_outset_1
            }
        }

        // Card #28e item 5 (board 4 frame 3): a dimmer between the base chrome
        // and the floating palette (the "conversation dimmed slightly" layer).
        dimmer := SolidView {
            width: Fill height: Fill
            visible: false
            draw_bg.color: #1D1D1F40
        }

        // #28e3 item 1: the review panel and the settings drawer paint in
        // right-aligned overlay docks ABOVE the base chrome (later siblings
        // draw on top), so a narrow window overlays them with the dimmer
        // instead of squeezing the center below its 420px minimum. Wide
        // windows show the same docked pixels via the columns spacers.
        review_dock := View {
            // Fill/Fill + internal right align: the palette_dock's proven
            // overlay pattern (a Fit-width wrapper here loses the root's
            // overlay positioning and lands top-left). The dock hides with
            // its panel, so it never shadows clicks while hidden.
            width: Fill height: Fill
            align: Align{x: 1.0 y: 0.0}
            visible: false

            // Card #28e item 3 (board 4 frame 1): the 560 px Review panel, toggled by the
            // Review affordance. #36c: the body was header-only ("Empty for
            // now — the header + scope pill only"), so a live preview showed
            // "+62 -5" with no file rows and no diff. The panel now mounts a
            // PortalList of file rows + the selected file's diff, fed from the
            // screen cache through the same `bindings::query` path the other
            // lists use (`turn.active` at :2142) — `bindings.rs:172-173` routes
            // `review::query`, so `review.file{N}.path/add/del` and
            // `review.line{N}` resolve live. The web reference is
            // `DiffReviewDialog.tsx:125-146` (`preview.files.map(...)` as
            // `<details className="diff-file">` rows, hunks beneath) with the
            // explicit empty state at :120-122; the 8 line slots are the card's
            // own `review.line0..7` bindings.
            review_panel := SolidView {
                width: 560 height: Fill flow: Down spacing: 6
                visible: true
                draw_bg.color: theme.color_bg_app
                review_header := View {
                    width: Fill height: Fit flow: Right spacing: 8
                    Label {
                        width: Fit height: Fit text: "Review"
                        draw_text.text_style.font_size: 14
                        // #36c r1: a Label's default is
                        // `theme.color_label_outer` (the linked rev,
                        // `widgets/src/label.rs:23`), and THIS APP never assigns
                        // that role: `desktop_style.rs:201-215` installs a style
                        // sheet only from `MAKEPAD_WIDGET_STYLE` or an
                        // iOS/Android `OsType`, and on macOS falls to `_ =>
                        // None` (line 209), so no sheet is ever installed and
                        // the role stays unset. Measured on the same capture:
                        // labels WITH an explicit colour draw ink (`+62 −5` 357
                        // dark px, the row's `M` 135), labels WITHOUT draw none
                        // (`Review` 0, `Last turn` 0, `path` 0, `line_text` 0).
                        // So every label in the panel needs an explicit colour.
                        draw_text.color: theme.color_fg_app
                    }
                    // #36c r1: the totals are the LIVE sums over the fetched
                    // preview — `review.rs:341-348` folds `review.add` /
                    // `review.del` from `st.files` (the web sums the same
                    // preview, DiffReviewDialog.tsx:34-41), and draw_walk sets
                    // them below. This used to hardcode the board's static
                    // "+62 -5", which is why a review with different files still
                    // showed 62/5 and made the empty body look inconsistent.
                    // #36c r1: an explicit colour is REQUIRED here — the
                    // default `theme.color_label_outer` is never assigned (see
                    // the "Review" label above), so this is the one header label
                    // that already drew ink (357 dark px on the capture).
                    // Empty until the first fetch lands — `sync_labels` writes the
                    // live `+n −n` sums (see the arms above). No placeholder
                    // number is honest: the board's "+62 −5" was fiction, and so
                    // would be any other hardcoded total. The web shows no
                    // totals before the preview either (DiffReviewDialog.tsx:78).
                    review_totals := Label {
                        width: Fit height: Fit text: ""
                        draw_text.text_style.font_size: 11
                        draw_text.color: theme.color_text_muted
                    }
                    review_scope := View {
                        width: Fit height: Fit flow: Overlay
                        review_scope_pill := RoundedView {
                            width: Fit height: Fit flow: Right spacing: 4
                            padding: Inset{left: 10 right: 10 top: 4 bottom: 4}
                            draw_bg +: {color: theme.color_bg_even border_radius: 999.0}
                            // #28e2 item 1: "▾" was tofu (Inter lacks it) —
                            // the kit chevron SVG instead.
                            Label {
                                width: Fit height: Fit text: "Last turn"
                                draw_text.text_style.font_size: 11
                                // #36c r1: explicit colour — without it this
                                // label drew ZERO ink (the default role is
                                // never assigned; see the "Review" label above).
                                draw_text.color: theme.color_fg_app
                            }
                            Svg {
                                width: 8 height: 8
                                animating: false
                                draw_svg.svg: file_resource(#(crate::design::icon_resource("chevron_down.svg")))
                                draw_svg.preserve_viewbox: true
                            }
                        }
                        // #28e2 item 4: the toggle hit target is the pill
                        // itself now — the old transparent button carried its
                        // own "Review" text, the duplicate the review flagged.
                        review_toggle_hit := Button {
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
                    // #28e2 item 1: "✕" was tofu — the close SVG instead.
                    review_close_wrap := View {
                        width: 28 height: 28 flow: Overlay
                        review_close_icon := Svg {
                            width: 12 height: 12
                            align: Align{x: 0.5 y: 0.5}
                            animating: false
                            draw_svg.svg: file_resource(#(crate::design::icon_resource("icon_close.svg")))
                            draw_svg.preserve_viewbox: true
                        }
                        review_close := Button {
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
                // #36c: the review BODY — a file-row list plus the selected
                // file's diff, both fed from the screen cache in draw_walk.
                //
                // A `PortalList` under a Fit-height parent computes zero visible
                // rows (the f3 captures behind `palette_list:704`), so both
                // lists below carry an explicit height.
                review_files := PortalList {
                    width: Fill height: 132 flow: Down
                    ReviewFileRowTpl := View {
                        width: Fill height: 32 flow: Right spacing: 8
                        padding: Inset{left: 12 right: 12}
                        review_file_status := Label { width: 14 height: Fit text: "M" draw_text.text_style.font_size: 11 draw_text.color: theme.color_text_muted }
                        // #36c r1: the counts come BEFORE the Fill path. With
                        // `path` first, the row's fixed parts (14+44+44 plus
                        // 3x8 spacing) exceeded the 336px inner width, so the
                        // Fill resolved NEGATIVE and the two count labels were
                        // pushed to r=[0,0,0,0] — text set but never drawn. The
                        // path takes Fill last and gets 210px.
                        review_file_add := Label { width: 44 height: Fit text: "" draw_text.text_style.font_size: 11 draw_text.color: #3a8a3a }
                        review_file_del := Label { width: 44 height: Fit text: "" draw_text.text_style.font_size: 11 draw_text.color: #b04040 }
                        review_file_path := Label { width: Fill height: Fit text: "" draw_text.text_style.font_size: 12 draw_text.color: theme.color_fg_app }
                    }
                }
                review_diff := PortalList {
                    width: Fill height: 372 flow: Down
                    ReviewLineRowTpl := View {
                        // The tint layers are a BACKDROP, so the row itself is a
                        // `flow: Overlay` stack: a `flow: Right` row laid the two
                        // SolidViews out as SIBLINGS (measured w=276 beside the
                        // labels, painting white) instead of behind them.
                        // The labels sit in their own inner `flow: Right` child,
                        // which keeps the web's 12px gutters.
                        width: Fill height: 20 flow: Overlay
                        // #36e item 2: the web's +/− line tint
                        // (`DiffReviewDialog.module.css:45-54`) — an ADDED line
                        // takes a success-coloured fill, a REMOVED line an
                        // error-coloured one, a CONTEXT line none. The web tints
                        // the changed WORD; a native row is one Label, so the row
                        // carries the fill (the nearest faithful form).
                        //
                        // Two PRE-COLOURED layers toggled with `set_visible`, the
                        // app's own precedent for a per-row fill
                        // (`palette_row_bg`, :809 + :2328) — the linked Makepad
                        // rev has no `set_bg_color`, so a per-row colour cannot be
                        // set after mounting.
                        //
                        // The colour is a LITERAL hex spliced in at lower time
                        // from `theme::resolved()` — NOT a `theme.*` role: a new
                        // role is not readable by a widget default here, and
                        // naming one aborted the module's `script_mod`
                        // (`property … not found in prototype chain` ->
                        // `OctoscodeView not found` -> the whole app unmounted).
                        // This is the same shape the design emitter uses when it
                        // writes `hex_rgba` literals
                        // (`octoscript-makepad/src/design.rs:733-735`), and
                        // `lib.rs:121` is the `#(...)` splice precedent.
                        // A HARD-CODED literal per palette, not a `#(...)`
                        // splice: measured, a spliced `draw_bg.color` delivers
                        // NOTHING here (a hardcoded literal in the same row
                        // paints — the A/B and the control layer both
                        // discriminate). `draw_walk` shows the pair matching
                        // `theme::resolved()` and the line's mark.
                        review_line_bg_add_light := SolidView {
                            width: Fill height: Fill
                            draw_bg.color: #cef2dc
                            visible: false
                        }
                        review_line_bg_del_light := SolidView {
                            width: Fill height: Fill
                            draw_bg.color: #fbd4d4
                            visible: false
                        }
                        review_line_bg_add_dark := SolidView {
                            width: Fill height: Fill
                            draw_bg.color: #1d442f
                            visible: false
                        }
                        review_line_bg_del_dark := SolidView {
                            width: Fill height: Fill
                            draw_bg.color: #462425
                            visible: false
                        }

                        // The row's own 12px gutters and right-aligned flow, in an
                        // inner child so the layers above stay full-bleed.
                        View {
                            width: Fill height: Fill flow: Right spacing: 8
                            padding: Inset{left: 12 right: 12}
                            review_line_num := Label { width: 34 height: Fit text: "" draw_text.text_style.font_size: 11 draw_text.color: theme.color_text_muted }
                            review_line_text := Label { width: Fill height: Fit text: "" draw_text.text_style.font_size: 11 draw_text.color: theme.color_fg_app }
                        }
                    }
                }
            }
        }
        // A3 (board 2 screens 6-11): Settings. Desktop: the web's centred
        // dialog (`SettingsSurface.module.css` .frame: min(800, 100vw - 48)
        // x min(800, 100dvh - 48), radius 24 -> 16 here) over the dimmer,
        // closed by the backdrop or the x; phone: a full-screen sheet with the
        // board's icon rail. The frame margin/size are set by sync_chrome.
        settings_dock := View {
            width: Fill height: Fill flow: Overlay
            align: Align{x: 0.5 y: 0.5}
            visible: false
            settings_backdrop := mod.widgets.OcBackdrop {}
            settings_frame := View {
                width: Fill height: Fill flow: Overlay
                max_width: 800 max_height: 640
                margin: 24
                // Swallows clicks on the panel's empty areas so they never
                // reach the backdrop under it.
                settings_swallow := mod.widgets.OcBackdrop {}
                settings_drawer := mod.widgets.OcSettingsPanel {}
            }
        }

        // A3 (board 2 screen 7): the Stop-server confirm, centred over its
        // own scrim. Only `server_stop_confirm` here sends server/shutdown.
        stop_dock := View {
            width: Fill height: Fill flow: Overlay
            align: Align{x: 0.5 y: 0.5}
            visible: false
            SolidView { width: Fill height: Fill draw_bg.color: #1D1D1F4D }
            stop_backdrop := mod.widgets.OcBackdrop {}
            stop_frame := View {
                width: Fill height: Fit max_width: 400
                margin: Inset{left: 16 right: 16}
                stop_dialog := mod.widgets.OcStopConfirm {}
            }
        }

        // A3 (board 2 screen 4): the workspace overflow menu, anchored to the
        // clicked "⋯" by sync_chrome (margin within this full-window layer);
        // a click anywhere else closes it.
        sb_menu_dock := View {
            width: Fill height: Fill flow: Overlay
            visible: false
            sb_menu_dismiss := mod.widgets.OcBackdrop {}
            sb_menu := mod.widgets.OcWorkspaceMenu {}
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
            draw_bg +: {color: #FFFFFF border_radius: 12.0 border_size: 1.0 border_color: theme.color_outset_1}
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
                // #28e2 item 5: the first row is highlighted (board 4 frame 3)
                // — a toggleable background layer the draw step shows for
                // row 0 only.
                PaletteRowTpl := View {
                    width: Fill height: 34 flow: Overlay
                    palette_row_bg := RoundedView {
                        width: Fill height: Fill
                        margin: Inset{left: 2 right: 2}
                        draw_bg +: {color: theme.color_bg_even border_radius: 8.0}
                        visible: false
                    }
                    palette_row_inner := View {
                        width: Fill height: Fill flow: Right spacing: 8
                        padding: Inset{left: 6 top: 8}
                        palette_row_name := Label { width: 150 height: Fit text: "" draw_text.text_style.font_size: 13  draw_text.color: theme.color_fg_app}
                        palette_row_desc := Label { width: Fill height: Fit text: "" draw_text.text_style.font_size: 11 draw_text.color: theme.color_text_muted }
                    }
                }
            }
            // #28e2 item 1: the hint's arrows and return were tofu (Inter
            // lacks those glyphs) — small SVGs instead of text glyphs.
            palette_hint := View {
                width: Fill height: Fit flow: Right spacing: 4
                Svg {
                    width: 9 height: 9
                    animating: false
                    draw_svg.svg: file_resource(#(crate::design::icon_resource("icon_arrow_up.svg")))
                    draw_svg.preserve_viewbox: true
                }
                Svg {
                    width: 9 height: 9
                    animating: false
                    draw_svg.svg: file_resource(#(crate::design::icon_resource("icon_arrow_down.svg")))
                    draw_svg.preserve_viewbox: true
                }
                Label { width: Fit height: Fit text: "move" draw_text.text_style.font_size: 10 draw_text.color: theme.color_text_muted }
                Label { width: Fit height: Fit text: "·" draw_text.text_style.font_size: 10 draw_text.color: theme.color_text_muted }
                Svg {
                    width: 9 height: 9
                    animating: false
                    draw_svg.svg: file_resource(#(crate::design::icon_resource("icon_return.svg")))
                    draw_svg.preserve_viewbox: true
                }
                Label { width: Fit height: Fit text: "run" draw_text.text_style.font_size: 10 draw_text.color: theme.color_text_muted }
                Label { width: Fit height: Fit text: "· esc" draw_text.text_style.font_size: 10 draw_text.color: theme.color_text_muted }
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
            draw_bg.color: theme.color_bg_app
            // Board 4 frame 4: the sidebar is still there but EMPTY — only
            // "OctosCode" and a grey "No threads yet".
            first_run_sidebar := View {
                width: 260 height: Fill flow: Down spacing: 6
                draw_bg.color: theme.color_bg_odd
                padding: Inset{left: 12 right: 10 top: 12 bottom: 12}
                Label {
                    width: Fill height: Fit text: "OctosCode"
                    draw_text.text_style.font_size: 13
                    draw_text.color: theme.color_fg_app
                }
                Label {
                    width: Fill height: Fit text: "No threads yet"
                    draw_text.text_style.font_size: 13
                    draw_text.color: theme.color_text_muted
                }
            }
            first_run_rule := View {
                width: 1 height: Fill
                draw_bg.color: theme.color_outset_1
            }
            first_run_center := View {
                // #28e6: the card area IS the screen_dock (the Overlay sibling
                // AFTER first_run, so it paints above this chrome) — the swap
                // probe proved the dock seats AND renders the Connect DSL
                // while every first-run-shaped slot mis-seated it to 133x700
                // (both probe arms, /snap + PNG). Nothing mounts here.
                width: Fill height: Fill flow: Down
            }
        }

        // #28e6: the screens' dock — AND, since the swap probe, the
        // first-run Connect card area. It sits AFTER first_run in this
        // Overlay so it paints above the first-run chrome; visible when
        // OCTOSCODE_SCREEN names a screen or on first run (sync_chrome).
        screen_dock := View {
            width: Fill height: Fill
            align: Align{x: 1.0 y: 0.0}
            visible: false
            // Fill/Fill: the reference host (screens_probe.rs:147) gives the
            // screen the full window; a Fit slot collapsed the mounted tree
            // to 0x0 (/snap-probed after the move out of review_column).
            // A3: Overlay so the in-app dock's close sits over the card.
            flow: Overlay
            screen_splash := Splash {
                width: Fill height: Fill
            }
            // A3: a screen docked IN-APP (+ Add workspace -> the folder
            // browser) gets a close; the env dev mount does not show it.
            screen_dock_close_slot := View {
                width: Fill height: Fit align: Align{x: 1.0 y: 0.0}
                padding: Inset{top: 10 right: 10}
                visible: false
                View {
                    width: 32 height: 32 flow: Overlay align: Align{x: 0.5 y: 0.5}
                    Svg {
                        width: 14 height: 14
                        animating: false
                        draw_svg.svg: file_resource(#(crate::chrome::icon("x_fg")))
                        draw_svg.preserve_viewbox: true
                    }
                    screen_dock_close := mod.widgets.OcHitRound {}
                }
            }
        }

        // #M2 — the FLEET dock (autonomy-06, the peers card). This slot is
        // what was missing: `screens::fleet::lower` had ZERO call sites, so the
        // card was never shown even though its handlers were already routed
        // (`fleet::is_action` at :1538 -> resolve -> spawn). Fit/Follows height
        // rather than Fill, because the fleet card is a list that grows with the
        // peer count and must not stretch the window (the Fill/Fill slot above
        // is for a full-window screen; the review card uses the same measured
        // idiom in its column).
        fleet_dock := View {
            width: Fill height: Fit
            align: Align{x: 1.0 y: 0.0}
            visible: false
            fleet_splash := Splash {
                width: Fill height: Fit
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
    /// #29a: the board-2 screens' state (2.1/2.2/2.3 drafts + validation +
    /// classification + onboarding results). The card reads it through
    /// `screens::connect::copies`; the action path writes it.
    pub(crate) screens: Arc<Mutex<screens::connect::ConnectUi>>,
}

/// Seed a store with `n` synthetic timeline rows and one session, for the
/// virtualization proof (`OCTOSCODE_SYNTHETIC_TIMELINE`). No transport: the
/// window draws the virtualized list on its own.
/// #31e — three pending approvals pushed STRAIGHT into the store's approval
/// domain (`OCTOSCODE_APPROVAL_SEED`; the `OCTOSCODE_SYNTHETIC_TIMELINE`
/// precedent: a proof-only seed). The keyboard decision's evidence needs the
/// SAME store rows the client's `approval/requested` handler would push; the
/// notification-distribution path belongs to the client domain's card, and
/// the fake server's frames never reached it (nine probe rounds showed the
/// store empty while the wire carried the pushes).
/// Which #D1 screen set owns a name in `OCTOSCODE_SCREEN`.
///
/// The nine phase4 cards are one board with three seams: the five pairing cards
/// (`screens::pairing`), the two provider-editor cards (`screens::provider`)
/// and the two folder-browser cards (`screens::browser`). Each set owns its own
/// action ids — `fd1_phase4_wiring.rs::the_three_sets_do_not_overlap` pins that
/// an id is never claimed twice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase4Set {
    Pairing,
    Provider,
    Browser,
}

/// Map an `OCTOSCODE_SCREEN` name to the #D1 screen set that owns it, or `None`
/// when the name is not one of #D1's nine cards.
///
/// The names are the atlas's own screen numbers, so a screenshot in a report
/// names the same card the env var mounts. WHICH card of the set is mounted is
/// the set's own live state (`<set>::lower_mounted`), not this table's job —
/// that is why the enum is all this needs to carry.
fn phase4_screen(which: &str) -> Option<Phase4Set> {
    Some(match which {
        "p4-01" | "p4-02" | "p4-03" | "p4-04" | "p4-05" => Phase4Set::Pairing,
        "p4-06" | "p4-07" => Phase4Set::Provider,
        "p4-08" | "p4-09" => Phase4Set::Browser,
        _ => return None,
    })
}

fn seed_approvals(store: &Arc<Store>) {    use octoscode_store::domains::approval::PendingApproval;
    for id in ["a1-approve-me", "a2-approve-session", "a3-deny-me"] {
        store.domains.approval.push(PendingApproval {
            id: id.to_owned(),
            target: None,
            decided: false,
            auto_resolved: false,
            cancelled: false,
            preview_id: None,
        });
    }
}

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

    // GOALS / LOOPS / FLEET (board 4 frame 3). #31a item 2: an EMPTY session
    // must show only THREADS — OCTOSCODE_SYNTHETIC_EMPTY=1 skips the autonomy
    // seed so the sections' data-driven visibility is provable (live store,
    // no goal/loops/fleet).
    if std::env::var("OCTOSCODE_SYNTHETIC_EMPTY").is_ok() {
        return;
    }
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

/// #D1t: the two board-1 transport effects, so ONE `rt.spawn` arm can carry
/// either screen's typed effect across the await point (the two enums are
/// unrelated types, so the `match` needs a common carrier).
#[derive(Debug)]
enum D1Effect {
    Provider(screens::provider::Effect),
    Browser(screens::browser::Effect),
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
    /// #32h item 1: the mounted card's tap wiring — (widget id, action id)
    /// pairs from the lowered card's wired DesignNativeButton blocks. The
    /// Event::Actions loop routes their clicks into the screens' tables
    /// (the outer loop's L4: nothing dispatched in-splash clicks — Stage C
    /// had tested the tables by calling ids directly, never by clicking).
    ///
    /// #35b item 1: renamed from `splash_taps` because it is no longer the
    /// first-run Connect card's alone — every docked card mounted into
    /// `screen_splash` publishes its taps here (setup-08/11/12 and the theme
    /// cards), and each tap is dispatched to the resolver that owns its id
    /// (`screens::taps::owner_of`).
    #[rust]
    screen_taps: Vec<(LiveId, String)>,
    /// #32h: the 1 Hz remount guard — sync_labels runs on EVERY Signal and
    /// the phone's transport events arrive constantly, so the first-run card
    /// was re-lowered + re-wired each time (device log: "card events: 1
    /// tap(s) wired" at 1 Hz), dropping taps mid-remount and unseating the
    /// mask. The card's only live inputs are these two copies; unchanged
    /// copies = skip the whole lower+mount.
    #[rust]
    connect_key: Option<(String, String)>,
    /// #32h TOP: the composer text the WIDGET currently holds (changed
    /// events and our own set_text keep it current). The store draft is
    /// pushed to the widget ONLY when it differs — a real external change
    /// (send-clear, new chat, restore) — never on the typing path: baking
    /// the draft into the DSL remounted the composer every keystroke and
    /// killed the Android IME target (the device: val stuck at the first
    /// character).
    #[rust]
    composer_synced: Option<String>,
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
    /// #36c — the review sheet's two body lists (file rows + the selected
    /// file's diff), captured like the palette's so `draw_walk` can tell which
    /// `PortalList` a draw step belongs to (0 = not captured yet).
    #[rust]
    review_files_uid: u64,
    #[rust]
    review_diff_uid: u64,
    /// #28e3 item 1: the window's inner width, tracked from
    /// `WindowGeomChange` (0 = no event yet — treated as wide).
    #[rust]
    window_w: f64,
    /// #31a: the <760 sidebar toggle — once the window hid the sidebar there
    /// was no way back; this flip shows it over the content (Codex-style).
    /// A3: the drawer state now lives in `screens::sidebar` (drawer.open /
    /// drawer.close); this field is kept for the env-seeded captures.
    #[rust]
    sidebar_open: bool,
    /// A3: the board-2 chrome's runtime (layout seat, menu anchor, the
    /// in-app docked screen).
    #[rust]
    chrome: ChromeRuntime,
    /// A3: the window's inner height (the settings frame's max height).
    #[rust]
    window_h: f64,
}

impl OctoscodeView {
    // #28e4 merge: the #28e2 signature (cx — the palette search field is
    // pre-filled through it) carries main's #29d error-screen seed.
    fn start(&mut self, cx: &mut Cx) {
        // #29d — seed the error screen the way the host's crash boundary would
        // (`OCTOSCODE_ERROR_SEED`; the `OCTOSCODE_SYNTHETIC_TIMELINE` precedent:
        // a proof-only seed, no transport). The sample carries secrets so the
        // capture proves the redaction boundary renders.
        if std::env::var("OCTOSCODE_ERROR_SEED").is_ok() {
            crate::screens::palette::report_error(
                "Render panicked: bad connection state\n\
                 GET https://octos.example/ws?token=abc123&x=1\n\
                 Authorization: Bearer sk-test-9f8e7d6c"
                    .to_owned(),
            );
        }
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

        // A3 — the board-2 capture seed (chrome.rs `seed_board2`): the board's
        // two workspaces, five threads, one session per status. No transport.
        if let Ok(variant) = std::env::var("OCTOSCODE_BOARD2_SEED") {
            {
                let b = self.bridge.lock().unwrap();
                chrome::seed_board2(&b.store, &variant);
            }
            makepad_widgets::log!("[octoscode] synthetic live: board-2 seed ({variant})");
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
            // #28e2 item 5: the palette's search field shows the typed
            // "/mo" like board 4 frame 3.
            self.view
                .text_input(cx, &[live_id!(palette_search)])
                .set_text(cx, "/mo");
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
        // #31e — the keyboard-decision seed, AFTER the bridge swap: connect
        // REPLACES `b.store` with the Conversation's own store above, so a
        // pre-connect seed landed on a store the shell then threw away (the
        // Y/S/N gate read oldest=None while the seed line printed — the swap
        // was the break). Still WITH transport: the receipts need the wire.
        if std::env::var("OCTOSCODE_APPROVAL_SEED").is_ok() {
            seed_approvals(&conv.store);
            makepad_widgets::log!("[octoscode] approval seed: 3 pending cards");
        }
        // Entry #29c: the stage-C screens fold their three profile reads once
        // at startup, behind the temporary-mount flag (until #28e's shell).
        if std::env::var_os("OCTOSCODE_STAGE_C_SCREENS").is_some() {
            let drv = conv.clone();
            runtime.spawn(async move {
                match screens::models::refresh(&drv, &drv.store).await {
                    Ok(n) => makepad_widgets::log!("[octoscode] screens: {n} profile reads folded"),
                    Err(e) => makepad_widgets::log!("[octoscode] screens refresh: {e}"),
                }
                match screens::fleet::refresh(&drv, &drv.store).await {
                    Ok(n) => makepad_widgets::log!("[octoscode] fleet: {n} fleet reads folded"),
                    Err(e) => makepad_widgets::log!("[octoscode] fleet refresh: {e}"),
                }
                SignalToUI::set_ui_signal();
            });
        }

        // Drive the conversation: open the workspace, then drain events.
        let drv = conv.clone();
        runtime.spawn(async move {
            // #32h: ensure a profile that EXISTS server-side BEFORE the first
            // session/open. The previous order ran open_workspace FIRST: on
            // the phone it failed (the baked fallback "octoscode" does not
            // exist; -32120 "agent is outside the requested profile scope"),
            // logged via ::log (invisible on logcat) and RETURNED — the
            // ensure block below never ran (the outer loop's device test of
            // fbbaa08: no "profile ready" line at all). The desktop live
            // gate keeps its explicit env.
            let ensure_profile = std::env::var_os("OCTOS_CREATE_PROFILE").is_some()
                || std::env::var_os("OCTOS_PROFILE_ID").is_none();
            if ensure_profile {
                match drv.create_profile().await {
                    Ok(id) => {
                        drv.adopt_profile(id.clone());
                        makepad_widgets::log!("[octoscode] profile ready: {id}");
                    }
                    Err(e) => {
                        makepad_widgets::log!("[octoscode] profile/local/create failed: {e}")
                    }
                }
            }
            if let Err(e) = drv.open_workspace(cwd).await {
                makepad_widgets::log!("[octoscode] session/open: {e}");
                SignalToUI::set_ui_signal();
                return;
            }
            while let Some(evt) = evt_rx.recv().await {
                // #29c: the screens' occupancy window folds from the
                // token_cost_update progress payloads (workspace-events.ts:6-10).
                screens::models::note_transport_event(&evt);
                screens::review::note_transport_event(&evt);
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
    ///
    /// `cx` is needed for exactly one thing: the clipboard. Both copy actions
    /// (`answer.copy`, `error.copy_diagnostics`) resolve to an effect carrying
    /// the text, and the platform clipboard is only reachable through
    /// `Cx::copy_to_clipboard` (makepad `platform/src/cx_api.rs:1629`) — it is
    /// the host's, not the module's, so the write must happen here rather than
    /// inside a resolver. #35d.
    fn perform_action(&mut self, cx: &mut Cx, action: &str, index: usize) {
        // #30b: board-3 autonomy actions route through their own table first
        // (one-owner rule); no other router sees these ids. `goal.set` /
        // `monitor.create` carry the composer draft as their entry text.
        if screens::autonomy::is_action(action) {
            let (store, ui, conv) = {
                let b = self.bridge.lock().unwrap();
                (b.store.clone(), b.ui.clone(), b.conv.clone())
            };
            let value = ui.lock().unwrap().draft();
            let effect = {
                let ctx = bindings::Ctx::new(&store, &ui);
                screens::autonomy::resolve(action, index, Some(&value), &ctx)
            };
            if let screens::autonomy::Effect::Unhandled(id) = &effect {
                ::log::warn!("octoscode: unhandled autonomy action {id:?}");
                return;
            }
            if let (Some(rt), Some(conv)) = (self.runtime.as_ref(), conv) {
                screens::autonomy::spawn(effect, rt, conv);
            }
            return;
        }
        // #30c: board-3 (Fleet/Tasks) actions route through the screens table
        // first; the conversation router never sees them (one-owner rule).
        if screens::fleet::is_action(action) {
            let (store, ui, conv) = {
                let b = self.bridge.lock().unwrap();
                (b.store.clone(), b.ui.clone(), b.conv.clone())
            };
            let effect = {
                let ctx = bindings::Ctx::new(&store, &ui);
                screens::fleet::resolve(action, index, &ctx)
            };
            if let screens::fleet::Effect::Unhandled(id) = &effect {
                ::log::warn!("octoscode: unhandled screen action {id:?}");
                return;
            }
            if let (Some(rt), Some(conv)) = (self.runtime.as_ref(), conv) {
                screens::fleet::spawn(effect, rt, &conv, &ui, &store);
            }
            return;
        }
        // #29b: board-2 (setup screens 04/05/06) actions route through the
        // screens table first; the conversation router never sees them.
        if screens::workspace::is_action(action) {
            let (store, ui, conv) = {
                let b = self.bridge.lock().unwrap();
                (b.store.clone(), b.ui.clone(), b.conv.clone())
            };
            let effect = {
                let ctx = bindings::Ctx::new(&store, &ui);
                screens::workspace::resolve(action, index, &ctx)
            };
            if let screens::workspace::Effect::Unhandled(id) = &effect {
                ::log::warn!("octoscode: unhandled screen action {id:?}");
                return;
            }
            if let screens::workspace::Effect::CopyDiagnostics = &effect {
                // #40b — the click audit's last dead row: this click was a
                // silent no-op ("UI-local"). The fork exposes no clipboard
                // API in reach, so the copy emits the store's diagnostics
                // summary on the route log — the observable, honest payload
                // (the same `[octoscode] route ... ->` channel every routed
                // action reports through; the audit's ACTION_LOG watches it).
                let total = store.diagnostics.total();
                let methods = store.diagnostics.methods();
                ::log::info!(
                    "[octoscode] route error.copy_diagnostics -> {total} notifications across {} methods: {}",
                    methods.len(),
                    methods.join(", ")
                );
                return;
            }
            if matches!(effect, screens::workspace::Effect::Close) {
                return; // UI-local: the error card closes itself
            }
            if let (Some(rt), Some(conv)) = (self.runtime.as_ref(), conv) {
                screens::workspace::spawn(effect, rt, conv);
            }
            return;
        }
        // A3 / #D2b: board 2's Settings half (screens 6-12). The native chrome
        // and the Stage B cards dispatch the SAME ids; ONE owner
        // (`screens::settings`). The RPC ids (server/shutdown behind the
        // confirm gate, permission/profile/set) go through the production
        // client; the rest apply locally, as the web keeps them.
        if screens::settings::owns(action) {
            makepad_widgets::log!("[octoscode] settings action: {action}");
            let (store, ui, conv) = {
                let b = self.bridge.lock().unwrap();
                (b.store.clone(), b.ui.clone(), b.conv.clone())
            };
            use screens::settings::UiEffect;
            match screens::settings::apply_ui(action) {
                UiEffect::None => {}
                UiEffect::Open => {
                    if let Ok(mut u) = ui.lock() {
                        if !u.settings_open() {
                            u.toggle_settings();
                        }
                    }
                }
                UiEffect::Close => {
                    if let Ok(mut u) = ui.lock() {
                        if u.settings_open() {
                            u.toggle_settings();
                        }
                    }
                }
                UiEffect::Thinking(t) => screens::settings::set_thinking(&store, t),
                UiEffect::Send => {
                    if screens::settings::action_params(action, &store).is_none() {
                        if action == "settings.model.next" {
                            // One configured model: nothing to switch to.
                            makepad_widgets::log!("[octoscode] settings: no other model to select");
                        } else {
                            // Unadvertised (no server/shutdown) or no session:
                            // fail closed, visibly — never a silent no-op.
                            screens::settings::note_failed(action, "not offered by this server");
                        }
                    } else if let (Some(rt), Some(conv)) = (self.runtime.as_ref(), conv) {
                        let action = action.to_owned();
                        let next_model = screens::settings::next_model(&store).map(|m| m.model);
                        rt.spawn(async move {
                            match screens::settings::perform(&conv, &action, &store).await {
                                Ok(_) => {
                                    screens::settings::note_sent(&action);
                                    if let (true, Some(m)) = (action == "settings.model.next", next_model) {
                                        screens::settings::mark_model_selected(&store, &m);
                                    }
                                    makepad_widgets::log!("[octoscode] settings sent: {action}");
                                    if action == "server.stop.confirm" {
                                        // The web disconnects on purpose after a
                                        // confirmed stop (App.tsx onStopServer):
                                        // the window returns to the connect card.
                                        store.set_connection("Offline".to_owned(), false);
                                    }
                                }
                                Err(e) => {
                                    makepad_widgets::log!("[octoscode] settings {action}: {e}");
                                    screens::settings::note_failed(&action, &e);
                                }
                            }
                            SignalToUI::set_ui_signal();
                        });
                    } else {
                        screens::settings::note_failed(action, "not connected");
                    }
                }
                UiEffect::TakeOver => {
                    // Board 12: claim the session's driver seat — the
                    // external-driver seat API (`session/driver/acquire`,
                    // web external-driver.ts:727-758; CAS on the revision the
                    // last `session/driver/get` reported). The server decides;
                    // a refusal leaves the banner up and is logged.
                    let session = store.active_session().unwrap_or_default();
                    if let (Some(rt), Some(conv)) = (self.runtime.as_ref(), conv) {
                        rt.spawn(async move {
                            let params = chrome::take_over_params(&session);
                            match conv.client().request("session/driver/acquire", params).await {
                                Ok(_) => {
                                    chrome::set_held(&session, None);
                                    makepad_widgets::log!("[octoscode] take over: seat acquired");
                                }
                                Err(e) => makepad_widgets::log!("[octoscode] take over refused: {e}"),
                            }
                            SignalToUI::set_ui_signal();
                        });
                    }
                }
            }
            return;
        }
        // #D2a: board 2's sidebar half (screens 1-5 — grouped tree, statuses,
        // search, collapsed rail, compact drawer) owns its 13 card events. They
        // route through the sidebar table FIRST (one-owner rule); the
        // conversation router never sees them. `session.open` / `new_chat` /
        // `workspace.new_chat_here` are the only ones that touch the transport,
        // and they reuse the router's OWN production ids (`thread.open`,
        // `session.new` — actions.rs:64/71) rather than minting a session here.
        if screens::sidebar::is_action(action) {
            let (store, ui) = {
                let b = self.bridge.lock().unwrap();
                (b.store.clone(), b.ui.clone())
            };
            let effect = {
                let ctx = bindings::Ctx::new(&store, &ui);
                screens::sidebar::resolve(action, index, &ctx)
            };
            match effect {
                screens::sidebar::Effect::Unhandled(id) => {
                    ::log::warn!("octoscode: unhandled sidebar action {id:?}");
                    return;
                }
                // UI-local halves are already applied inside resolve.
                screens::sidebar::Effect::Applied => {
                    makepad_widgets::log!("[octoscode] sidebar action: {action}");
                    return;
                }
                screens::sidebar::Effect::NewChat => {
                    makepad_widgets::log!("[octoscode] sidebar action: {action}");
                    self.perform_action(cx, bindings::ACTION_NEW_CHAT, 0);
                    return;
                }
                screens::sidebar::Effect::OpenSession { index } => {
                    makepad_widgets::log!("[octoscode] sidebar action: {action} row {index}");
                    self.perform_action(cx, "thread.open", index);
                    return;
                }
                screens::sidebar::Effect::NewChatInWorkspace { workspace, path } => {
                    // #D2a: uniform observability — this IS a handled sidebar
                    // action, so it emits the same "sidebar action:" line.
                    makepad_widgets::log!(
                        "[octoscode] sidebar action: {action} -> {workspace} ({})",
                        path.as_deref().unwrap_or("server default cwd")
                    );
                    // A3: the web's "New session in {workspace}" opens the
                    // session WITH that workspace's cwd (session/open { cwd },
                    // session-defaults.ts:5-7) — the same `new_chat` path the
                    // router's `session.new` takes, scoped by the group's path.
                    let conv = { self.bridge.lock().unwrap().conv.clone() };
                    if let (Some(rt), Some(conv)) = (self.runtime.as_ref(), conv) {
                        rt.spawn(async move {
                            match conv.new_chat(path).await {
                                Ok(id) => ::log::info!("octoscode: new chat in {workspace}: {id}"),
                                Err(e) => makepad_widgets::log!("[octoscode] new chat dropped: {e}"),
                            }
                        });
                    }
                    return;
                }
                screens::sidebar::Effect::AddWorkspace => {
                    // The host docks the folder browser (sync_labels consumes
                    // `sidebar::take_add_request`).
                    makepad_widgets::log!("[octoscode] sidebar action: {action}");
                    return;
                }
            }
        }
        // #30a: the board-3 review screens' ids route through their own table
        // first (one-owner rule); the conversation router never sees them.
        if screens::review::is_action(action) {
            let (store, ui, conv) = {
                let b = self.bridge.lock().unwrap();
                (b.store.clone(), b.ui.clone(), b.conv.clone())
            };
            let effect = {
                let ctx = bindings::Ctx::new(&store, &ui);
                screens::review::resolve(action, index, &ctx)
            };
            if let screens::review::Effect::Unhandled(id) = &effect {
                ::log::warn!("octoscode: unhandled screen action {id:?}");
                return;
            }
            if let (Some(rt), Some(conv)) = (self.runtime.as_ref(), conv) {
                screens::review::spawn(effect, rt, conv);
            }
            return;
        }
        // #29a: the board-2 screens' own ids are not conversation actions, so
        // they route to the screens' table FIRST — the conversation router
        // must never see them (the one-owner rule, LESSONS).
        if screens::connect::is_action(action) {
            self.perform_screen_action(action, None);
            return;
        }
        // #30d: board-3 (autonomy-08/09/10) actions route through the screens
        // table first; the conversation router never sees them (one-owner).
        if screens::sessions::is_action(action) {
            let (store, ui, conv) = {
                let b = self.bridge.lock().unwrap();
                (b.store.clone(), b.ui.clone(), b.conv.clone())
            };
            let effect = {
                let ctx = bindings::Ctx::new(&store, &ui);
                screens::sessions::resolve(action, index, &ctx)
            };
            if let screens::sessions::Effect::Unhandled(id) = &effect {
                ::log::warn!("octoscode: unhandled screen action {id:?}");
                return;
            }
            // UI-local effects (stage/cancel/remove/dismiss) were already
            // applied inside resolve; only the protocol effects spawn.
            if matches!(
                effect,
                screens::sessions::Effect::ResumeStage(_)
                    | screens::sessions::Effect::ResumeCancel
                    | screens::sessions::Effect::AttachmentRemove(_)
                    | screens::sessions::Effect::AsideDismiss
            ) {
                return;
            }
            if let (Some(rt), Some(conv)) = (self.runtime.as_ref(), conv) {
                screens::sessions::spawn(effect, rt, conv);
            }
            return;
        }
        // #30e — the theme preference's own action id (one-owner rule): the
        // sidebar's single theme button cycles system -> dark -> light ->
        // system (use-theme.ts:44-49) and re-resolves the mounted card set
        // app-wide. UI-local: no protocol method carries a display theme
        // (parity row preferences/g-timeline, model.ts:1 — browser storage).
        if screens::theme::is_action(action) {
            let (store, ui) = {
                let b = self.bridge.lock().unwrap();
                (b.store.clone(), b.ui.clone())
            };
            let effect = {
                let ctx = bindings::Ctx::new(&store, &ui);
                screens::theme::resolve(action, index, &ctx)
            };
            match effect {
                screens::theme::Effect::Cycle { preference, resolved } => {
                    ::log::info!(
                        "octoscode: theme -> {preference} (resolves {resolved}); \
                         the next mount lowers the {resolved} card set"
                    );
                }
                screens::theme::Effect::Unhandled(id) => {
                    ::log::warn!("octoscode: unhandled screen action {id:?}");
                }
            }
            return;
        }
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
            // #29d — UI-local screen effects (resolve already applied them):
            // selection/query feed, and the report copy.
            // #35d: `CopyReport` is the ONE effect that still had work to do —
            // `resolve` builds the report and flips `copy_state`, but the text
            // was then DISCARDED here, so the control flipped state and never
            // copied anything. The web's handler writes it
            // (FatalErrorBoundary.tsx:44-50, `navigator.clipboard.writeText`)
            // and only then reports "Copied"/"Copy failed". The clipboard is the
            // HOST's, so the write happens here with the `cx` this arm now has.
            actions::Effect::Screen(
                crate::screens::palette::Effect::Move(_)
                | crate::screens::palette::Effect::QuerySet,
            ) => return,
            actions::Effect::Screen(
                crate::screens::palette::Effect::CopyReport(report),
            ) => {
                if report.is_empty() {
                    // resolve already set copy_state="failed" (palette.rs:286).
                    makepad_widgets::log!("[octoscode] copy diagnostics: empty report, nothing copied");
                    return;
                }
                cx.copy_to_clipboard(report);
                makepad_widgets::log!(
                    "[octoscode] copy diagnostics: {} bytes to clipboard",
                    report.len()
                );
                return;
            }
            _ => {}
        }
        let Some(rt) = self.runtime.as_ref() else {
            return;
        };
        // #29d — `connection.retry` replays the production handshake. It must
        // run BEFORE the conv guard: a retry is exactly for the case where the
        // initial connect failed and `bridge.conv` is still None.
        if let actions::Effect::Screen(crate::screens::palette::Effect::Retry) = effect {
            let bridge = self.bridge.clone();
            let base =
                std::env::var("OCTOS_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:50082".into());
            let bearer = std::env::var("OCTOS_BEARER").unwrap_or_default();
            let profile =
                std::env::var("OCTOS_PROFILE_ID").unwrap_or_else(|_| "octoscode".to_string());
            let cwd = std::env::var("OCTOS_WORKSPACE_CWD").ok();
            let connected = {
                let _guard = rt.enter();
                crate::flow::Conversation::connect(
                    &base,
                    &bearer,
                    &profile,
                    cwd.clone(),
                    Some(Arc::new(|| SignalToUI::set_ui_signal())),
                )
            };
            match connected {
                Ok((conv, mut evt_rx)) => {
                    let conv = Arc::new(conv);
                    {
                        let mut b = bridge.lock().unwrap();
                        b.store = conv.store.clone();
                        b.ui = conv.ui();
                        b.conv = Some(conv.clone());
                    }
                    ::log::info!("octoscode: connection.retry connected");
                    rt.spawn(async move {
                        while let Some(evt) = evt_rx.recv().await {
                            let e = conv.on_event(evt);
                            ::log::debug!("[octoscode] {e:?}");
                            SignalToUI::set_ui_signal();
                        }
                    });
                }
                Err(e) => ::log::warn!("octoscode: connection.retry: {e}"),
            }
            return;
        }
        let Some(conv) = conv else {
            makepad_widgets::log!("[octoscode] action dropped: no conversation (connect first)");
            return;
        };
        // Entry #29c: the stage-C screens own their action ids (the cards'
        // service-actions events); route them through the production client.
        if screens::research::owns(action) {
            let action = action.to_string();
            let store = store.clone();
            rt.spawn(async move {
                if let Err(e) =
                    screens::research::perform(&conv, &action, &store, None).await
                {
                    ::log::warn!("octoscode: screens: {action:?}: {e}");
                }
            });
            return;
        }
        // D3b — board-3 screens 5-8: the cards' NAV events write their store
        // seams (thinking prefs, folded-thinking rows, the resume gate).
        if screens::board3::owns(action) {
            let action = action.to_string();
            let store = store.clone();
            rt.spawn(async move {
                if let Err(e) = screens::board3::perform(&action, &store) {
                    ::log::warn!("octoscode: screens: {action:?}: {e}");
                }
            });
            return;
        }
        // P4f1: the history mutations (undo/rewind/fork) behind the history
        // dialog's three titles. The dialog itself is a design-flow surface;
        // these are the production paths it dispatches into.
        if screens::history::owns(action) {
            let action = action.to_string();
            let store = store.clone();
            rt.spawn(async move {
                if let Err(e) =
                    screens::history::perform(&conv, &action, &store, None).await
                {
                    ::log::warn!("octoscode: screens: {action:?}: {e}");
                }
            });
            return;
        }
        // P4d4: the media (attachment draft) and peers (roster axis) surfaces.
        // The dialogs themselves are design-flow surfaces; these own the state
        // and production paths they dispatch into.
        if screens::media::owns(action) || screens::peers::owns(action) {
            let action = action.to_owned();
            let store = store.clone();
            rt.spawn(async move {
                if let Err(e) = screens::perform_control(&conv, &action, &store).await {
                    ::log::warn!("octoscode: screens: {action:?}: {e}");
                }
            });
            return;
        }
        if screens::transcript::owns(action) {
            let store = store.clone();
            rt.spawn(async move {
                if let Err(e) = screens::transcript::perform(&conv, &store).await {
                    ::log::warn!("octoscode: screens: composer.copy_transcript: {e}");
                }
            });
            return;
        }
        if screens::models::owns(action) {
            let action = action.to_string();
            let store = store.clone();
            rt.spawn(async move {
                if let Err(e) = screens::models::perform(&conv, &action, &store).await {
                    ::log::warn!("octoscode: screens: {action:?}: {e}");
                }
            });
            return;
        }
        // #D1t: board 1's provider editor and workspace browser. Both cards
        // already mounted (`lib.rs:2459-2460` `lower_mounted`) but until this
        // dispatch existed their transport effects had NO executor, so
        // `prov.test` / `provider.save` / `browser.use` / `browser.create` were
        // reachable only from a test and RULES 3 scored them missing. The
        // pairing card is deliberately NOT here: it has no typed method (see
        // .peer/report-D1t.md §pairing).
        if screens::provider::is_action(action) || screens::browser::is_action(action) {
            // Apply to the LIVE state first; the transport effect (or None) is
            // what actually talks to the server. `perform_action` carries no
            // `value` (lib.rs:1513), so the input ids resolve with `None` here —
            // the live card's own fields are already in each module's state.
            let is_provider = screens::provider::is_action(action);
            let effect = if is_provider {
                screens::provider::perform(action, None)
                    .map(D1Effect::Provider)
            } else {
                screens::browser::perform(action, None).map(D1Effect::Browser)
            };
            let Some(effect) = effect else { return };
            // `action` is a method-lifetime `&str`, so the spawned task needs its
            // OWN copy (E0521) — it is only the log line's subject.
            let action = action.to_string();
            rt.spawn(async move {
                let out = match effect {
                    D1Effect::Provider(e) => {
                        screens::provider::perform_transport(&conv, e).await
                    }
                    D1Effect::Browser(e) => screens::browser::perform_transport(&conv, e).await,
                };
                if let Err(e) = out {
                    ::log::warn!("octoscode: screens: {action:?}: {e}");
                }
                // Repaint: the transport arm writes the card's own state.
                SignalToUI::set_ui_signal();
            });
            return;
        }
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
                        Err(e) => makepad_widgets::log!("[octoscode] new chat dropped: {e}"),
                    }
                });
            }
            actions::Effect::Submit => {
                rt.spawn(async move {
                    if let Err(e) = conv.submit_draft().await {
                        makepad_widgets::log!("[octoscode] submit dropped: {e}");
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
                // #34a row 190 — re-selecting the CURRENT thread must not
                // re-open the session: the web treats selecting the active
                // session as a no-op (runtime-recovery.spec.ts counts
                // session/open and asserts the transcript is never reset).
                // Re-opening here RESET the store's timeline from the
                // canonical hydrate, dropping the live turns (instrumented:
                // after a reselect the whole timeline emptied; sometimes the
                // reset raced the live rows — the #33b flake).
                let already_active = {
                    let b = self.bridge.lock().unwrap();
                    b.store.active_session().as_deref() == Some(session.as_str())
                };
                if already_active {
                    ::log::info!(
                        "octoscode: thread.open {session} — already active, no re-open"
                    );
                    return;
                }
                let cwd = std::env::var("OCTOS_WORKSPACE_CWD").ok();
                rt.spawn(async move {
                    match conv.open_session(&session, cwd).await {
                        Ok(id) => ::log::info!("octoscode: thread.open opened {id}"),
                        Err(e) => ::log::warn!("octoscode: thread.open: {e}"),
                    }
                });
            }
            // #29d — palette commands route to the EXISTING production effects
            // (fail closed: only commands with a native effect and an advertised
            // feature reach here, or they resolved to Unhandled).
            actions::Effect::Screen(crate::screens::palette::Effect::Run(effect_id, name)) => {
                match effect_id {
                    Some("session.refresh") => {
                        rt.spawn(async move {
                            if let Err(e) = conv.refresh_sessions().await {
                                ::log::warn!("octoscode: palette.run {name}: session.refresh: {e}");
                            }
                        });
                    }
                    // Fail closed with the command's own name: the table only
                    // ships effects the native client can actually perform, so
                    // this names a wiring gap, never a silent no-op.
                    other => ::log::warn!(
                        "octoscode: palette.run {name}: no native effect ({other:?})"
                    ),
                }
            }
            // Handled above / needs `cx` (copy).
            actions::Effect::ToggleTool(_)
            | actions::Effect::Unhandled(_)
            | actions::Effect::UiChrome(_)
            | actions::Effect::CopyAnswer => {}
            actions::Effect::Screen(other) => match other {
                crate::screens::palette::Effect::Run(..) => unreachable!("matched above"),
                crate::screens::palette::Effect::Move(_)
                | crate::screens::palette::Effect::QuerySet
                | crate::screens::palette::Effect::Unhandled(_) => {}
                // #35d: CopyReport returns from the UI-local arm above (it is
                // the one that writes), so this arm is only a no-op guard.
                crate::screens::palette::Effect::CopyReport(_) => {}
                crate::screens::palette::Effect::Retry => {}
            },
        }
    }

    /// #29a: run one board-2 screen action (`screens::connect::ACTIONS`).
    /// `value` carries an `input.*` payload (the field's live text) when the
    /// caller has one; without it the input effects no-op (the L0 input
    /// wiring lands with #28e's containers).
    fn perform_screen_action(&self, action: &str, value: Option<&str>) {
        // #32h: the terminal proof line (makepad macro — reaches logcat on
        // Android, unlike ::log::info!), fired for EVERY routed screen action.
        makepad_widgets::log!("[octoscode] screen action: {action}");
        let (bridge, store, conv, screens) = {
            let b = self.bridge.lock().unwrap();
            (self.bridge.clone(), b.store.clone(), b.conv.clone(), b.screens.clone())
        };
        let effect = screens::connect::resolve(action, value);
        if value.is_none() && matches!(effect, screens::connect::Effect::Input { .. }) {
            return;
        }
        // UI-local half: the field texts, live validation, the radio.
        let transport = {
            let mut ui = screens.lock().unwrap();
            screens::connect::apply(&mut ui, effect)
        };
        let Some(transport) = transport else { return };
        let Some(rt) = self.runtime.as_ref() else {
            ::log::warn!("octoscode: screen action {action}: no runtime");
            return;
        };
        // The shared connect-and-take-over path: a fresh `Conversation` on the
        // typed address, the event drain re-attached (the same shape as
        // `start`'s), the store's connection row and the screens' failure
        // classification kept current either way.
        let connect_now = move |handle: tokio::runtime::Handle,
                                bridge: Arc<Mutex<Bridge>>,
                                store: Arc<Store>,
                                screens: Arc<Mutex<screens::connect::ConnectUi>>,
                                server: String,
                                token: String,
                                profile: String,
                                discover: bool| {
            let cwd = std::env::var("OCTOS_WORKSPACE_CWD").ok();
            let waker: Arc<dyn Fn() + Send + Sync> = Arc::new(|| SignalToUI::set_ui_signal());
            handle.spawn(async move {
                // #32h: discover a REAL profile id BEFORE the upgrade — the
                // X-Profile-Id header is baked into TransportConfig at
                // connect time and the session is scoped to it: the outer
                // loop's curl shows ANY header gets the 101, but a
                // non-existent profile never answers session/open (the
                // silent submit). The solo login's user.id is a server-
                // verified top-level profile id; an explicit
                // OCTOS_PROFILE_ID (the desktop gate) is honored as-is; the
                // onboarding arm passes discover=false (its id already comes
                // from the server).
                let effective = if discover {
                    match std::env::var("OCTOS_PROFILE_ID") {
                        Ok(explicit) => explicit,
                        Err(_) => match Conversation::discover_solo_profile(&server).await {
                            Some(id) => {
                                makepad_widgets::log!("[octoscode] profile discovered: {id}");
                                id
                            }
                            None => {
                                makepad_widgets::log!(
                                    "[octoscode] profile discovery unavailable — falling back to {profile}"
                                );
                                profile
                            }
                        },
                    }
                } else {
                    profile
                };
                match Conversation::connect(&server, &token, &effective, cwd.clone(), Some(waker.clone())) {
                    Ok((conv, evt_rx)) => {
                        let conv = Arc::new(conv);
                        let mut evt_rx = evt_rx;
                        if let Ok(mut b) = bridge.lock() {
                            // #32h: swap ALL THREE, mirroring the start()
                            // path. The closure only set b.conv, so on the
                            // phone the composer's changed events wrote the
                            // draft into the PRE-CONNECT FlowUi while
                            // conv.submit_draft() read the conversation's own
                            // (empty) one: the silent empty-draft return —
                            // no drop log, draft kept, sessions 0 (server
                            // events folded into a store the labels never
                            // read). The conv store carries Live itself, as
                            // on the desktop path.
                            b.store = conv.store.clone();
                            b.ui = conv.ui();
                            b.conv = Some(conv.clone());
                        }
                        if let Ok(mut ui) = screens.lock() {
                            ui.failure = None;
                            ui.raw_error = None;
                            ui.endpoint_error = None;
                        }
                        // #32h: THIS is the path the phone's Connect tap takes
                        // (the startup path instrumented in 3f52566/156c321 is
                        // env-gated). c70f1ca's device run exposed the REAL
                        // freeze: open_workspace().await never resolved
                        // because the response needs the event drain, and the
                        // drain only started AFTER this block. The drain goes
                        // FIRST; ensure+open run in their own task with a
                        // 15 s timeout so a wedged request can never hang the
                        // connect path again (the outer loop's prescription).
                        let drv = conv.clone();
                        tokio::spawn(async move {
                            while let Some(evt) = evt_rx.recv().await {
                                let _ = drv.on_event(evt);
                                SignalToUI::set_ui_signal();
                            }
                        });
                        let conv2 = conv.clone();
                        tokio::spawn(async move {
                            makepad_widgets::log!(
                                "[octoscode] live: profile={effective} env_profile={}",
                                std::env::var_os("OCTOS_PROFILE_ID").is_some()
                            );
                            let open = async {
                                if let Err(e) = conv2.open_workspace(cwd).await {
                                    makepad_widgets::log!(
                                        "[octoscode] session/open failed: {e} — ensuring a profile"
                                    );
                                    match tokio::time::timeout(
                                        std::time::Duration::from_secs(15),
                                        conv2.create_profile(),
                                    )
                                    .await
                                    {
                                        Ok(Ok(id)) => {
                                            conv2.adopt_profile(id.clone());
                                            makepad_widgets::log!(
                                                "[octoscode] profile ready: {id}"
                                            );
                                        }
                                        Ok(Err(e)) => makepad_widgets::log!(
                                            "[octoscode] profile/local/create failed: {e}"
                                        ),
                                        Err(_) => makepad_widgets::log!(
                                            "[octoscode] profile/local/create: timed out"
                                        ),
                                    }
                                    match tokio::time::timeout(
                                        std::time::Duration::from_secs(15),
                                        conv2.open_workspace(None),
                                    )
                                    .await
                                    {
                                        Ok(Ok(_)) => makepad_widgets::log!(
                                            "[octoscode] workspace open: {}",
                                            conv2.session_id()
                                        ),
                                        Ok(Err(e)) => {
                                            makepad_widgets::log!("[octoscode] session/open: {e}")
                                        }
                                        Err(_) => {
                                            makepad_widgets::log!("[octoscode] session/open: timed out")
                                        }
                                    }
                                } else {
                                    makepad_widgets::log!(
                                        "[octoscode] workspace open: {}",
                                        conv2.session_id()
                                    );
                                }
                            };
                            if tokio::time::timeout(
                                std::time::Duration::from_secs(15),
                                open,
                            )
                            .await
                            .is_err()
                            {
                                makepad_widgets::log!(
                                    "[octoscode] session/open: timed out (15 s)"
                                );
                            }
                        });
                    }
                    Err(e) => {
                        store.set_connection("Offline".to_owned(), false);
                        makepad_widgets::log!("[octoscode] transport open failed: {e}");
                        if let Ok(mut ui) = screens.lock() {
                            ui.note_connect_error(&e, &screens::connect::clock_12h());
                        }
                    }
                }
                waker();
            });
        };
        let handle = rt.handle().clone();
        match transport {
            screens::connect::Effect::Connect { server, token } => {
                // #32h: ::log::info! is NOT routed to logcat on Android (the
                // outer loop's diagnosis — no connect line was ever visible
                // on the 6T); makepad_widgets::log! reaches the platform log.
                makepad_widgets::log!("[octoscode] connect: {server}");
                let profile =
                    std::env::var("OCTOS_PROFILE_ID").unwrap_or_else(|_| "octoscode".to_string());
                connect_now(handle, bridge, store, screens, server, token, profile, true);
            }
            // The web's `onConfigured` (`onboarding-submission.ts:126`): once
            // the provider is saved, open the canonical session — here a
            // reconnect under the server-assigned profile id.
            screens::connect::Effect::CreateProfile => {
                let Some(conv) = conv else {
                    if let Ok(mut ui) = screens.lock() {
                        ui.onboarding_error = Some("Connect to a server first.".to_owned());
                    }
                    return;
                };
                let (id, api_key, provider, server) = {
                    let ui = screens.lock().unwrap();
                    (
                        ui.profile_name.trim().to_owned(),
                        ui.api_key.clone(),
                        ui.provider,
                        ui.server.clone(),
                    )
                };
                let client = conv.client().clone();
                let screens2 = screens.clone();
                let store2 = store.clone();
                rt.spawn(async move {
                    match screens::connect::run_onboarding(
                        &client, &id, &id, &api_key, provider, None,
                    )
                    .await
                    {
                        Ok(out) => {
                            if let Ok(mut ui) = screens2.lock() {
                                ui.onboarding_error = None;
                                ui.created_profile = Some(out.profile_id.clone());
                            }
                            connect_now(
                                handle, bridge, store2, screens2, server,
                                String::new(), out.profile_id, false,
                            );
                        }
                        Err(e) => {
                            ::log::warn!("octoscode: create_profile: {e}");
                            if let Ok(mut ui) = screens2.lock() {
                                ui.onboarding_error = Some(e);
                            }
                            SignalToUI::set_ui_signal();
                        }
                    }
                });
            }
            // Consumed in `apply`; Unhandled logged there would be noise here.
            screens::connect::Effect::Retry
            | screens::connect::Effect::Input { .. }
            | screens::connect::Effect::SelectProvider(_) => {}
            screens::connect::Effect::Unhandled(id) => {
                ::log::warn!("octoscode: screen action unhandled {id:?}");
            }
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
        // #36c r1: the review header's +/- totals are the LIVE sums over the
        // fetched preview. `review.rs:341-348` folds `review.add` / `review.del`
        // from `st.files` — the same sum the web does over the same preview
        // (`DiffReviewDialog.tsx:34-41`). This used to hardcode the board's
        // static "+62 -5", so a review of any other files still showed 62/5.
        // Read the same way as `turn.active` below, and only when the sheet is
        // open: a hidden header must not churn every frame.
        {
            let b = self.bridge.lock().unwrap();
            let open =
                b.ui.lock().map(|u| u.review_open()).unwrap_or(false) || chrome_env().0;
            if open {
                let ctx = bindings::Ctx::new(&b.store, &b.ui);
                let s = |id: &str| {
                    bindings::query(&ctx, id)
                        .and_then(|v| v.as_str().map(str::to_owned))
                        .unwrap_or_default()
                };
                let (add, del) = (s("review.add"), s("review.del"));
                // With no preview folded the bindings resolve to None and the
                // authored placeholder stays (the web's empty state shows no
                // totals either, DiffReviewDialog.tsx:78).
                if !add.is_empty() || !del.is_empty() {
                    self.view
                        .label(cx, ids!(review_totals))
                        .set_text(cx, &format!("{add} {del}"));
                }
            }
        }
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
        // #32h TOP: external draft changes reach the EXISTING TextInput via
        // set_text — the lowered DSL no longer carries the draft, so typing
        // never remounts the composer (the mount cache hits: the DSL is
        // stable while focused).
        let store_draft = { self.bridge.lock().unwrap().ui.lock().unwrap().draft() };
        if self.composer_synced.as_deref() != Some(store_draft.as_str()) {
            if store_draft.is_empty() && self.composer_synced.is_none() {
                // Initial state: the authored empty text is already right.
                self.composer_synced = Some(store_draft);
            } else {
                self.view
                    .text_input(cx, &[live_id!(i0_composer_0)])
                    .set_text(cx, &store_draft);
                self.composer_synced = Some(store_draft);
            }
        }
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
        match self.mounts.mount(cx, &composer_splash, &composer) {
            Err(e) => makepad_widgets::log!("[octoscode] composer mount: {e}"),
            // #32h TOP: one line per REAL remount — the per-key typing test
            // asserts this fires only at the initial mount, never per char.
            Ok(true) => makepad_widgets::log!("[octoscode] composer remounted"),
            Ok(false) => {}
        }
        // #29d — the Stage C screens (board 2.8/2.11/2.12) mount into the review
        // column's temporary slot while #28e's shell (drawer + palette overlay)
        // is pending. OCTOSCODE_SCREEN=palette|error|loading names one; unset
        // leaves the screen exactly as before this card. The mount cache
        // compares the DSL string, so live-slot flips repaint exactly once.
        // A3 (board 12): read the active session's driver record once per
        // session when the server advertises `session/driver/get` (the web's
        // driver-inventory read, driver-inventory-snapshot.ts), and record a
        // FOREIGN holder for the held banner (seat-holder.ts:18-29).
        {
            let (store, conv) = {
                let b = self.bridge.lock().unwrap();
                (b.store.clone(), b.conv.clone())
            };
            let active = store.active_session();
            if active.is_some() && active != self.chrome.driver_probe {
                self.chrome.driver_probe = active.clone();
                let advertised = store
                    .domains
                    .config
                    .supported_methods()
                    .iter()
                    .any(|m| m == "session/driver/get");
                if let (true, Some(rt), Some(conv), Some(sid)) =
                    (advertised, self.runtime.as_ref(), conv, active)
                {
                    rt.spawn(async move {
                        let params = serde_json::json!({ "session_id": sid });
                        match conv.client().request("session/driver/get", params).await {
                            Ok(v) => {
                                let held = chrome::foreign_holder(&v, chrome::NATIVE_DRIVER_ID);
                                makepad_widgets::log!("[octoscode] driver/get {sid}: held={held:?}");
                                chrome::set_held(&sid, held);
                            }
                            Err(e) => makepad_widgets::log!("[octoscode] driver/get {sid}: {e}"),
                        }
                        SignalToUI::set_ui_signal();
                    });
                }
            }
        }
        // A3: the Model row and the new-chat defaults strip name the profile's
        // selected model — read the profile once per connection when the
        // store has no model list yet (the read `screens::models::refresh`
        // performs; before the chrome it ran only behind a dev flag).
        {
            let (store, conv) = {
                let b = self.bridge.lock().unwrap();
                (b.store.clone(), b.conv.clone())
            };
            if !self.chrome.models_requested
                && store.is_live()
                && store.domains.profile.llm_models().is_empty()
            {
                if let (Some(rt), Some(conv)) = (self.runtime.as_ref(), conv) {
                    self.chrome.models_requested = true;
                    rt.spawn(async move {
                        match screens::models::refresh(&conv, &conv.store).await {
                            Ok(n) => makepad_widgets::log!("[octoscode] chrome: {n} profile reads folded"),
                            Err(e) => makepad_widgets::log!("[octoscode] chrome profile reads: {e}"),
                        }
                        SignalToUI::set_ui_signal();
                    });
                }
            }
        }
        // A3: desktop notifications (General > Desktop notifications): a
        // settled turn or a new wait on the active session, while the window
        // is in the background, posts one OS notice (`Cx::show_notification`).
        {
            let store = { self.bridge.lock().unwrap().store.clone() };
            if let Some(now) = chrome::Attention::of(&store) {
                let title = store
                    .sessions()
                    .into_iter()
                    .find(|s| s.id == now.session)
                    .and_then(|s| s.label_stem())
                    .unwrap_or_else(|| "Your chat".to_owned());
                let notice = chrome::attention_notice(
                    self.chrome.attention.as_ref(),
                    &now,
                    &title,
                    screens::settings::snapshot().notifications,
                    self.chrome.unfocused,
                );
                if let Some((t, body)) = notice {
                    makepad_widgets::log!("[octoscode] notify: {t} — {body}");
                    cx.show_notification(&t, &body);
                }
                self.chrome.attention = Some(now);
            }
        }
        // A3: `+ Add workspace` docks the folder browser IN-APP (the web's
        // "Add workspace" opens the workspace picker, ProductSidebar.tsx:578).
        if screens::sidebar::take_add_request() {
            self.chrome.docked = Some("p4-08".to_owned());
            makepad_widgets::log!("[octoscode] dock: folder browser (workspace.add)");
        }
        if let Some(which) = self.docked_screen() {
            // #28e6: connect is NOT mounted here — on first run it mounts
            // into THIS dock via the !live path (below); the other two setup
            // screens have no home yet. The handler keeps the #29d screens.
            let screen_splash = self.view.splash(cx, ids!(screen_splash));
            let store = { self.bridge.lock().unwrap().store.clone() };
            // #M1: `models::lower` needs a `Ctx` (store + the flow's UI state).
            // The bridge carries both, and the bridge lock must be released
            // before the mount arms run, so clone the `ui` handle here — the
            // same `b.ui.clone()` the mounted screens already use (lib.rs:1520).
            let ui = { self.bridge.lock().unwrap().ui.clone() };
            // #30e — OCTOSCODE_THEME seeds the preference (system default),
            // and the theme-wired card names lower through screens::theme,
            // which selects the dark Stage B card or its light twin by the
            // CURRENT resolved preference. Unset theme = system = dark set,
            // byte-identical to the pre-#30e behaviour for palette|error|
            // loading names.
            if let Ok(pref) = std::env::var("OCTOSCODE_THEME") {
                screens::theme::set_preference(&pref);
            }
            // #32f item 1: the design tree roots in the app's OWN writable
            // storage — the host hands the files dir over here, before any
            // design read can trigger design::root() (HOME is usually unset
            // in an app process and temp_dir is unwritable on Android).
            if let Some(files) = cx.get_data_dir() {
                crate::design::set_host_dir(Some(files));
            }
            // #M1: the models/skills/context cards (setup-07 Model settings,
            // setup-09 Context panel, setup-10 Skills). These three had
            // handlers wired (refresh/owns/perform) but `models::lower` had
            // 0 call sites, so every skills/models/context row was
            // production-path yet user-UNREACHABLE — RULES 3's "exists but
            // nobody can get there". Mounted here, reusing the existing
            // accepted Stage-B cards; no new design.
            let lowered = if screens::models::card_for(&which).is_some() {
                let ctx = bindings::Ctx::new(&store, &ui);
                screens::models::lower(&which, &ctx)
            } else if screens::board3::card_for(&which).is_some() {
                // D3b — board-3 screens 5-8 mount where the web opens them.
                screens::board3::lower(&which)
            } else if screens::theme::card_for(&which).is_some() {
                screens::theme::lower(&which, &store)
            } else if screens::settings::is_screen(&which) {
                // #D2b: board 2's settings cards (the design record; the
                // product Settings is the native chrome).
                screens::settings::lower(&which)
            } else if let Some(screen) = screens::autonomy::Screen3::from_env() {
                // #P4e1c — the autonomy cards (autonomy-03/04/05: Goal, Loops,
                // Monitors). Before this the ONLY caller of
                // `autonomy::lower_screen` was the capture example
                // (`examples/screen_shot_autonomy.rs:223`): the module's whole
                // action table was live, but no user could reach any of it.
                // The web shows all three in ONE dialog as three stacked
                // sections (`AutonomyPanel.tsx:105` <h2>, then :114-115 Goal,
                // :235-236 Loops, :335-336 Monitors — no tabs), so there is no
                // tab bar to build; the three cards ARE the three sections.
                let st = screens::autonomy::state_snapshot();
                let dsl = match screens::autonomy::lower_screen(screen, &st) {
                    Ok(dsl) => dsl,
                    Err(e) => {
                        // The enclosing `if/else` is a value expression, not a
                        // fallible function body, so this arm yields the same
                        // shape rather than a `?`.
                        makepad_widgets::log!("[octoscode] autonomy mount: {e}");
                        return;
                    }
                };
                // The goal card's Pause/Stop are real Buttons, but their atlas
                // bounds drifted 4px/16px from the placements this card renders
                // — far past taps::POS_TOLERANCE — so the shared
                // `service-actions` path wires nothing. Wire them at the card's
                // OWN placements, and publish the unwireable controls in the
                // log so a control that is drawn but not clickable is recorded
                // rather than silently dead (the #35a defect class).
                let (dsl, report) = if screen == screens::autonomy::Screen3::Goal {
                    screens::taps::wire_events_at(&dsl, &screens::autonomy::goal_controls())
                } else {
                    (dsl, Vec::new())
                };
                let dead: Vec<&str> = report
                    .iter()
                    .filter(|(_, _, ok)| !ok)
                    .map(|(name, _, _)| name.as_str())
                    .collect();
                if !dead.is_empty() {
                    ::log::warn!(
                        "octoscode: autonomy {:?}: {} control(s) drawn but not clickable: {:?}",
                        screen,
                        dead.len(),
                        dead
                    );
                }
                Ok(dsl)
            } else if let Some(set) = phase4_screen(&which) {
                // #D1: the nine phase4 cards (pairing p4-01..05, the provider
                // editor p4-06/07, the workspace browser p4-08/09). They live
                // under design/stage-b/phase4, not stage-b/setup, so they miss
                // the theme/palette lookup above and need their own arm. WHICH
                // card of the set mounts is the set's own live state, so the
                // name only has to say who owns it.
                match set {
                    Phase4Set::Pairing => screens::pairing::lower_mounted(),
                    Phase4Set::Provider => screens::provider::lower_mounted(),
                    Phase4Set::Browser => screens::browser::lower_mounted(),
                }
            } else if screens::sidebar::card_for(&which).is_some() {
                // #D2a: board 2's sidebar half (screens 1-5). Without this arm
                // the five phase4n2 cards were reachable from NO mount path, so
                // per RULES §3 the 13 actions existed only in tests.
                screens::sidebar::lower(&which)
            } else {
                crate::screens::palette::lower_screen(&which, &store)
            };
            // #35b item 1: publish the mounted card's wired taps BEFORE the
            // mount, so the Event::Actions loop routes them. setup-08/11/12's
            // buttons carried no on_click at all before #35b, so they were
            // dead in the real app (the #35a audit: error.reload clicked to
            // nothing). Lowering twice is cheap and the mount cache dedupes.
            if let Ok(dsl) = &lowered {
                self.screen_taps = screens::taps::wired_taps(dsl)
                    .into_iter()
                    .map(|(n, e)| (LiveId::from_str(&n), e))
                    .collect();
            }
            let r = match lowered {
                Ok(dsl) => self.mounts.mount(cx, &screen_splash, &dsl),
                Err(e) => Err(e),
            };
            if let Err(e) = r {
                makepad_widgets::log!("[octoscode] screen mount: {e}");
            }
        }
        // #M2 — MOUNT the fleet card (autonomy-06). This is the call the card is
        // about: before it, `screens::fleet::lower` had ZERO call sites, so the
        // authored peers card was never evaluated into the live view tree even
        // though its handlers were already routed (`fleet::is_action` above ->
        // `resolve` -> `spawn` -> the production client).
        //
        // Shape copied from the docked review card (the `open` gate + env
        // override), NOT from a new design: it reuses the authored
        // `design/stage-b/autonomy/cards/autonomy-06` card as-is.
        //
        // The open gate: `OCTOSCODE_CHROME=fleet` is the deterministic
        // headless-capture opener (the same contract as review/settings/palette
        // — a headless run cannot click a toggle). There is no user tap for it
        // YET, so the card is reachable by that env but not by hand; that limit
        // is recorded in the matrix evidence rather than claimed as a click.
        //
        // Taps need no new wiring: the lowered card's `on_click: || { NAV(t: …) }`
        // handlers are published into `screen_taps` by the same
        // `taps::wired_taps` pass the screen dock uses, and `taps::owner_of`
        // routes them to `fleet::is_action`. So mounting is sufficient to make
        // the row controls live.
        {
            let fleet_open = chrome_env().3;
            self.view.widget(cx, ids!(fleet_dock)).set_visible(cx, fleet_open);
            if fleet_open {
                let dsl = {
                    let b = self.bridge.lock().unwrap();
                    let ctx = bindings::Ctx::new(&b.store, &b.ui);
                    screens::fleet::lower("autonomy-06", &ctx)
                };
                match dsl {
                    Ok(dsl) => {
                        let fleet_splash = self.view.splash(cx, ids!(fleet_splash));
                        if let Err(e) = self.mounts.mount(cx, &fleet_splash, &dsl) {
                            makepad_widgets::log!("[octoscode] fleet mount: {e}");
                        }
                    }
                    Err(e) => makepad_widgets::log!("[octoscode] fleet lower: {e}"),
                }
            }
        }
        // #28e4 item 2: the first-run card area mounts the REAL board-2
        // Connect screen (setup-01, #29a) — `screens::connect::lower_screen`
        // lowers the authored Stage B card with the ConnectUi copies applied,
        // so the label contrast is the design's own and Connect routes for
        // real (`screens::connect::is_action` -> `resolve`, handled in this
        // file's action path). The mount cache dedupes, so this is cheap
        // while the first run is showing.
        // #28e6: on first run the dock IS the card area — it mounts the REAL
        // board-2 Connect screen (setup-01, #29a) via
        // `screens::connect::lower_screen` (ConnectUi copies applied; Connect
        // routes for real via `is_action` -> `resolve` in the action path).
        // The approved swap probe (73ec4a2 + e53d5ec; /snap + PNG under
        // tmp/28e-evidence/28e6-*) proved every first-run-shaped slot
        // mis-seats the measured DSL to 133x700 while the dock seats and
        // renders it — so first-run mounts through the dock.
        let live = { self.bridge.lock().unwrap().store.is_live() };
        if !live {
            let key = {
                let b = self.bridge.lock().unwrap();
                let ui = b.screens.lock().unwrap();
                (ui.server.clone(), ui.token.clone())
            };
            if self.connect_key.as_ref() == Some(&key) {
                // copies unchanged: the mounted card is current.
            } else {
                let splash = self.view.splash(cx, ids!(screen_splash));
                let lowered = {
                    let b = self.bridge.lock().unwrap();
                    let ui = b.screens.lock().unwrap();
                    screens::connect::lower_screen(screens::connect::Screen::Connect, &ui)
                };
                match lowered {
                    Ok(dsl) => {
                        self.connect_key = Some(key);
                        // #32h item 1: keep the card's tap wiring — the Event::
                        // Actions loop routes these (L4). #35b: the same
                        // `screen_taps` slot every docked card publishes into.
                        self.screen_taps = screens::connect::wired_taps(&dsl)
                            .into_iter()
                            .map(|(n, e)| (LiveId::from_str(&n), e))
                            .collect();
                    // #31a item 3: centre the card in the first-run area (not
                    // over the sidebar header, no left clipping — the arm-A
                    // probe had it at x=12). A plain View wrapper carries the
                    // slot's Fill walk and centers the natural-size card via
                    // align; the lowered string itself stays byte-identical
                    // (the f21/f29a replay tests assert on it).
                    let centered = format!(
                        "View {{\nwidth: Fill height: Fill\nflow: Overlay\nalign: Align{{x: 0.5 y: 0.5}}\n{}\n}}",
                        dsl
                    );
                    if let Err(e) = self.mounts.mount(cx, &splash, &centered) {
                        makepad_widgets::log!("[octoscode] connect mount: {e}");
                    }
                }
                    Err(e) => makepad_widgets::log!("[octoscode] connect lower: {e}"),
                }
            }
        }
        // A3 (board 2): the sidebar's "New chat" row is native chrome
        // (chrome.rs `sb_new_chat`); the #16 `new-chat` component mount that
        // lived here is retired with its slot.
        self.sync_chrome(cx);
        ::log::info!("[octoscode] {text} | sessions: {sessions}");
    }

    /// A3 (#D2a's 0x0 root cause, fixed once): the ONE "a screen is docked"
    /// predicate — the mount arm and the dock's visibility both read it, so a
    /// mounted screen can never sit in a hidden dock. In-app docking (+ Add
    /// workspace) and the `OCTOSCODE_SCREEN` dev override share it.
    fn docked_screen(&self) -> Option<String> {
        self.chrome
            .docked
            .clone()
            .or_else(|| std::env::var("OCTOSCODE_SCREEN").ok())
            .filter(|v| chrome::screen_dockable(v))
    }

    /// A3 (board 2): perform the chrome's click intents through the one-owner
    /// tables (`screens::sidebar`, `screens::settings`, the router), one log
    /// line each — the click walk's receipts.
    fn handle_chrome(&mut self, cx: &mut Cx, actions: &Actions) {
        let (store, settings_open) = {
            let b = self.bridge.lock().unwrap();
            let open = b.ui.lock().map(|u| u.settings_open()).unwrap_or(false) || chrome_env().1;
            (b.store.clone(), open)
        };
        let mut rt = std::mem::take(&mut self.chrome);
        let intents = rt.clicks(cx, &self.view, &store, settings_open, actions);
        self.chrome = rt;
        if intents.is_empty() {
            return;
        }
        let was_renaming = screens::sidebar::renaming();
        for intent in intents {
            makepad_widgets::log!("[octoscode] chrome: {intent:?}");
            match intent {
                chrome::Intent::Action(a, i) => self.perform_action(cx, a, i),
                chrome::Intent::Menu(i, rect) => {
                    // The menu anchors under the clicked "⋯" (module-local).
                    let (ox, oy) = self.chrome.origin;
                    self.chrome.menu_anchor =
                        Some((rect.pos.x + rect.size.x - ox, rect.pos.y + rect.size.y - oy));
                    self.perform_action(cx, "workspace.menu", i);
                }
                chrome::Intent::Query(q) => {
                    screens::sidebar::set_query(&q);
                }
                chrome::Intent::RenameCommit(t) => {
                    if let Some(key) = screens::sidebar::rename_commit(&t) {
                        makepad_widgets::log!("[octoscode] sidebar rename {key:?} -> {t:?}");
                    }
                }
                chrome::Intent::OpenSettings => self.perform_action(cx, "settings.panel.open", 0),
                chrome::Intent::CloseSettings => self.perform_action(cx, "settings.panel.close", 0),
                chrome::Intent::ToggleReview => self.perform_action(cx, "review.toggle", 0),
                chrome::Intent::CloseMenu => {
                    screens::sidebar::close_menu();
                    if screens::sidebar::renaming().is_some() {
                        self.perform_action(cx, "workspace.rename.cancel", 0);
                    }
                }
                chrome::Intent::CloseDock => {
                    self.chrome.docked = None;
                    makepad_widgets::log!("[octoscode] dock closed");
                }
            }
        }
        // A rename just started: seed the field with the current name, focus it.
        let now_renaming = screens::sidebar::renaming();
        if now_renaming.is_some() && now_renaming != was_renaming {
            let label = {
                let p = screens::sidebar::snapshot();
                p.last_groups
                    .iter()
                    .find(|g| Some(&g.key) == now_renaming.as_ref())
                    .map(|g| g.label.clone())
                    .unwrap_or_default()
            };
            let field = self.view.text_input(cx, ids!(sb_rename_input));
            field.set_text(cx, &label);
            self.view.widget(cx, ids!(sb_rename_input)).set_key_focus(cx);
        }
        self.sync_chrome(cx);
        self.view.redraw(cx);
    }

    /// A3: Escape closes the top-most chrome surface (the web's onEscape on
    /// every ModalSurface): the Stop dialog, the workspace menu, Settings,
    /// then the drawer. Returns whether it consumed the key.
    fn escape_chrome(&mut self, cx: &mut Cx) -> bool {
        let settings_open = {
            let b = self.bridge.lock().unwrap();
            let open = b.ui.lock().map(|u| u.settings_open()).unwrap_or(false);
            open
        };
        let consumed = if screens::settings::snapshot().stop_pending {
            self.perform_action(cx, "server.stop.cancel", 0);
            true
        } else if screens::sidebar::menu_for().is_some() || screens::sidebar::renaming().is_some() {
            screens::sidebar::close_menu();
            self.perform_action(cx, "workspace.rename.cancel", 0);
            true
        } else if settings_open {
            self.perform_action(cx, "settings.panel.close", 0);
            true
        } else if screens::sidebar::drawer_open() {
            self.perform_action(cx, "drawer.close", 0);
            true
        } else {
            false
        };
        if consumed {
            makepad_widgets::log!("[octoscode] chrome: Escape closed the top surface");
            self.sync_chrome(cx);
            self.view.redraw(cx);
        }
        consumed
    }

    /// Card #28e — move the chrome state (FlowUi flags + store) onto the view:
    /// the first-run swap, the review panel / settings drawer / palette /
    /// dimmer visibility, and the GOALS/LOOPS/FLEET sidebar sections.
    fn sync_chrome(&mut self, cx: &mut Cx) {
        // Card #28e — the headless-capture gate: `OCTOSCODE_CHROME=review|
        // settings|palette|fleet` pre-opens that surface deterministically (a
        // headless run cannot click the toggle). Parsed once per process.
        // #M2: `fleet` is the 4th arm (see chrome_env).
        let (env_review, env_settings, env_palette, _env_fleet) = chrome_env();
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
        // #28e3 item 1: the authoritative window width. Hidden-window
        // captures never fire WindowGeomChange, and the shell sizes the
        // window FROM this env — so when present it IS the width; real
        // windows (no env) keep the event-tracked value instead.
        if let Some((w, h)) = env_frame() {
            self.window_w = w;
            self.window_h = h;
        }
        // #32h A: a phone that never resized (the app opens full-screen; no
        // WindowGeomChange arrives) kept window_w at 0.0 and got the DESKTOP
        // shell — the sidebar stayed at 384 px and the composer sat off the
        // right edge (device /snap: i0_composer_0 [295,512,89,48]). Seed the
        // width from the root view's LAID-OUT rect when no event/env ever
        // set it: the instrument numbers are the truth on every target.
        if self.window_w == 0.0 {
            let w = self.view.area().rect(cx).size.x;
            if w > 0.0 {
                self.window_w = w;
            }
        }
        // #28e3 item 1: the responsive layout. The center column keeps a
        // 420 px minimum: at wide windows (>= 1260 = 260 sidebar + 2x10
        // spacing + 420 center + 560 review) the docked panels reserve their
        // room via the columns spacers and the overlay docks paint exactly
        // over them; below that the spacers hide and the docks OVERLAY from
        // the right with the dimmer. Below 760 px the sidebar hides too
        // (Codex-style). A panel's dock wrapper hides with it (a visible
        // Fill/Fill overlay would shadow the composer's buttons).
        let wide = self.window_w == 0.0 || self.window_w >= 1260.0;
        self.view.widget(cx, ids!(base)).set_visible(cx, live);
        self.view.widget(cx, ids!(first_run)).set_visible(cx, !live);
        // A3 (board 2): the sidebar's seat — the desktop column over its
        // spacer, or the compact drawer (min(320, w-48)) over the scrim; the
        // header's menu trigger opens the drawer (the web's compact
        // `navigationTrigger`, App.tsx:2359-2369). `OCTOSCODE_DRAWER_OPEN`
        // is the capture seed for the open drawer.
        if self.sidebar_open {
            self.sidebar_open = false;
            screens::sidebar::set_drawer_open(true);
        }
        if self.window_h == 0.0 {
            self.window_h = self.view.area().rect(cx).size.y;
        }
        {
            let store = { self.bridge.lock().unwrap().store.clone() };
            let (w, h) = (self.window_w, self.window_h);
            let mut chrome = std::mem::take(&mut self.chrome);
            chrome.origin = {
                let r = self.view.area().rect(cx);
                (r.pos.x, r.pos.y)
            };
            chrome.sync(cx, &self.view, &store, live, settings, w, h);
            self.chrome = chrome;
        }
        self.view.widget(cx, ids!(review_dock)).set_visible(cx, review);
        self.view.widget(cx, ids!(review_panel)).set_visible(cx, review);
        self.view
            .widget(cx, ids!(columns_review_spacer))
            .set_visible(cx, wide && review);
        // A3: Settings is the centred dialog (desktop) / full sheet (compact)
        // over the dimmer — no docked column, so its spacer stays hidden.
        self.view.widget(cx, ids!(settings_dock)).set_visible(cx, settings && live);
        self.view.widget(cx, ids!(settings_drawer)).set_visible(cx, settings && live);
        self.view
            .widget(cx, ids!(columns_settings_spacer))
            .set_visible(cx, false);
        self.view.widget(cx, ids!(palette)).set_visible(cx, palette);
        self.view.widget(cx, ids!(palette_dock)).set_visible(cx, palette);
        self.view
            .widget(cx, ids!(dimmer))
            .set_visible(cx, palette || settings || (review && !wide));
        // #28e4: the #29d proof-screen dock is visible only when
        // OCTOSCODE_SCREEN names one of them — a visible Fill/Fill wrapper
        // shadows the clicks under it. `connect` is NOT docked: it mounts in
        // the first-run card area (sync_labels).
        // #28e6: on first run the dock IS the Connect card area (the swap
        // probe proved the first-run-shaped slots mis-seat the card). A
        // visible Fill/Fill wrapper shadows clicks under it, but on first run
        // only the first-run chrome is under it.
        // #D2a root cause, fixed ONCE (A3): the dock's visibility used its own
        // whitelist (palette|error|loading, later +goal|loops|monitors) while
        // the mount arm above accepted ANY non-connect name — every other
        // mounted card was lowered, wired and drawn into a HIDDEN dock (0x0
        // rects, no screen_splash in /d). Both now read `docked_screen()`.
        let screen_dock_shown = self.docked_screen().is_some() || !live;
        self.view
            .widget(cx, ids!(screen_dock))
            .set_visible(cx, screen_dock_shown);
        // The in-app dock carries its own close (the env dev mount does not).
        self.view
            .widget(cx, ids!(screen_dock_close_slot))
            .set_visible(cx, live && self.chrome.docked.is_some());

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
        // A3: the collapsed rail shows no sections (it is icons only).
        let rail = screens::sidebar::snapshot().rail && !self.chrome.compact;
        self.view.widget(cx, ids!(autonomy_sections)).set_visible(cx, any && !rail);
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
        // #28e2 item 3: the Blocked row carries the board's yellow status dot.
        for (i, dot) in [
            live_id!(fleet_dot_1),
            live_id!(fleet_dot_2),
            live_id!(fleet_dot_3),
        ]
        .iter()
        .enumerate()
        {
            let blocked = fleet.get(i).map(|t| t.contains("Blocked")).unwrap_or(false);
            self.view.widget(cx, &[*dot]).set_visible(cx, blocked);
        }
    }

}

/// Card #28e — the `OCTOSCODE_CHROME` capture gate, parsed once.
/// `OCTOSCODE_CHROME=review|settings|palette|fleet` pre-opens a surface
/// deterministically (a headless run cannot click the toggle).
///
/// #M2: `fleet` added — the fleet card (`autonomy-06`) had **no** opener at
/// all, which is why `screens::fleet::lower` had 0 call sites. Returns a 4-tuple;
/// BOTH call sites are migrated in the same commit as this change
/// (`lib.rs:2293` review, `lib.rs:2574` the chrome table) — the #P4f2 lesson
/// about a signature change riding alone.
/// A3: `OCTOSENSE_WINDOW_SIZE=WxH` — the phone-size check. The desktop shell
/// does not size its windows from it, so the module draws INSIDE a WxH frame
/// (draw_walk) and lays out for that width: a real 360-wide layout, not a
/// desktop window whose content merely believes it is narrow.
fn env_frame() -> Option<(f64, f64)> {
    static FRAME: std::sync::OnceLock<Option<(f64, f64)>> = std::sync::OnceLock::new();
    *FRAME.get_or_init(|| {
        let sz = std::env::var("OCTOSENSE_WINDOW_SIZE").ok()?;
        let (w, h) = sz.split_once('x')?;
        let (w, h) = (w.trim().parse::<f64>().ok()?, h.trim().parse::<f64>().ok()?);
        (w > 0.0 && h > 0.0).then_some((w, h))
    })
}

fn chrome_env() -> (bool, bool, bool, bool) {
    static CHROME: std::sync::OnceLock<(bool, bool, bool, bool)> = std::sync::OnceLock::new();
    *CHROME.get_or_init(|| {
        let v = std::env::var("OCTOSCODE_CHROME")
            .unwrap_or_default()
            .to_lowercase();
        (v == "review", v == "settings", v == "palette", v == "fleet")
    })
}

impl Widget for OctoscodeView {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // A3: the phone-size frame (`env_frame`) — the module draws in a
        // WxH box at the window's top-left, clipped to what the shell gives.
        let walk = match env_frame() {
            Some((w, h)) => Walk {
                width: Size::Fixed(w),
                height: Size::Fixed(h),
                ..walk
            },
            None => walk,
        };
        // The two lists share makepad's default item template, so a draw step's
        // widget uid is how it says WHICH list it is (captured once).
        if self.thread_uid == 0 {
            self.thread_uid = self.view.portal_list(cx, ids!(thread_list)).widget_uid().0;
            self.timeline_uid = self.view.portal_list(cx, ids!(timeline_list)).widget_uid().0;
            self.palette_uid = self.view.portal_list(cx, ids!(palette_list)).widget_uid().0;
            self.review_files_uid = self.view.portal_list(cx, ids!(review_files)).widget_uid().0;
            self.review_diff_uid = self.view.portal_list(cx, ids!(review_diff)).widget_uid().0;
        }
        let (thread_uid, timeline_uid, palette_uid, review_files_uid, review_diff_uid) = (
            self.thread_uid,
            self.timeline_uid,
            self.palette_uid,
            self.review_files_uid,
            self.review_diff_uid,
        );
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
                    // A3 (board 2 screens 1-5): the LEFT column is the native
                    // sidebar tree — workspace groups, session rows with their
                    // status and relative time, search hits — projected from
                    // the store by `screens::sidebar::project` (chrome.rs).
                    let store = { bridge.lock().unwrap().store.clone() };
                    chrome::draw_sidebar_list(cx, &mut list, &store, None);
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
                        // #28e2 item 5 -> #31e: the highlight follows the
                        // LIVE selection (↑/↓ move it; CommandPalette.tsx:66's
                        // roving selection), no longer hardcoded row 0.
                        item.widget(cx, ids!(palette_row_bg)).set_visible(
                            cx,
                            id == crate::screens::palette::selected_row(),
                        );
                        item.label(cx, ids!(palette_row_name)).set_text(cx, name);
                        item.label(cx, ids!(palette_row_desc)).set_text(cx, desc);
                        item.draw_all_unscoped(cx);
                    }
                } else if uid == review_files_uid {
                    // #36c: the review sheet's file rows, from the screen cache
                    // through `bindings::query` (bindings.rs:172-173 routes
                    // `review::query`, so `review.file{N}.path/add/del` resolve
                    // live — the same read path as `turn.active` above).
                    //
                    // The card's own bindings expose THREE file slots
                    // (`review.file1..3.*`), so the visible window is the
                    // intersection of what the cache holds and what is bound.
                    let (paths, adds, dels) = {
                        let b = bridge.lock().unwrap();
                        let ctx = bindings::Ctx::new(&b.store, &b.ui);
                        let s = |id: &str| {
                            bindings::query(&ctx, id)
                                .and_then(|v| v.as_str().map(str::to_owned))
                                .unwrap_or_default()
                        };
                        let files: Vec<(String, String, String)> = (1..=3)
                            .map(|n| {
                                (
                                    s(&format!("review.file{n}.path")),
                                    s(&format!("review.file{n}.add")),
                                    s(&format!("review.file{n}.del")),
                                )
                            })
                            .take_while(|(p, _, _)| !p.is_empty())
                            .collect();
                        (
                            files.iter().map(|f| f.0.clone()).collect::<Vec<_>>(),
                            files.iter().map(|f| f.1.clone()).collect::<Vec<_>>(),
                            files.iter().map(|f| f.2.clone()).collect::<Vec<_>>(),
                        )
                    };
                    list.set_item_range(cx, 0, paths.len());
                    while let Some(id) = list.next_visible_item(cx) {
                        let (Some(path), Some(add), Some(del)) =
                            (paths.get(id), adds.get(id), dels.get(id))
                        else {
                            continue;
                        };
                        let item = list.item(cx, id, id!(ReviewFileRowTpl));
                        item.label(cx, ids!(review_file_status))
                            .set_text(cx, if id == 0 { "M" } else { " " });
                        item.label(cx, ids!(review_file_path)).set_text(cx, path);
                        item.label(cx, ids!(review_file_add)).set_text(cx, add);
                        item.label(cx, ids!(review_file_del)).set_text(cx, del);
                        item.draw_all_unscoped(cx);
                    }
                } else if uid == review_diff_uid {
                    // #36c: the SELECTED file's diff — the eight
                    // `review.line0..7` / `review.num0..7` card slots. The
                    // window is the slots that actually resolve, so a short
                    // preview shows short rather than blank filler rows.
                    // #36e item 2: the third field is the line's KIND, read from
                    // the cache rather than a binding — `review.markN` exists
                    // only for N=2..6 and there is no `review.kindN`, so a
                    // binding-driven tint would tint the wrong rows.
                    // `Line::mark` (`review.rs:145`) already collapses the wire
                    // kinds to added/removed/context, the same collapse as the
                    // web's `diffKind` (`diff-presentation.ts:17-23`).
                    let lines: Vec<(String, String, &'static str)> = {
                        let b = bridge.lock().unwrap();
                        let ctx = bindings::Ctx::new(&b.store, &b.ui);
                        let s = |id: &str| {
                            bindings::query(&ctx, id)
                                .and_then(|v| v.as_str().map(str::to_owned))
                                .unwrap_or_default()
                        };
                        let marks: Vec<&'static str> = {
                            let st = crate::screens::review::ui();
                            (0..8)
                                .map(|i| {
                                    st.lines
                                        .get(i)
                                        .map(crate::screens::review::Line::mark)
                                        .unwrap_or("")
                                })
                                .collect()
                        };
                        (0..8)
                            .map(|i| {
                                (
                                    s(&format!("review.line{i}")),
                                    s(&format!("review.num{i}")),
                                    marks[i],
                                )
                            })
                            .collect()
                    };
                    let shown = lines
                        .iter()
                        .rposition(|(text, _, _)| !text.is_empty())
                        .map(|last| last + 1)
                        .unwrap_or(0);
                    list.set_item_range(cx, 0, shown);
                    while let Some(id) = list.next_visible_item(cx) {
                        let Some((text, num, mark)) = lines.get(id) else {
                            continue;
                        };
                        let item = list.item(cx, id, id!(ReviewLineRowTpl));
                        item.label(cx, ids!(review_line_num)).set_text(cx, num);
                        item.label(cx, ids!(review_line_text)).set_text(cx, text);
                        // The web's +/− tint (DiffReviewDialog.module.css:45-54):
                        // added green, removed red, context none. The four
                        // layers are HARD-CODED literals (two per palette) and
                        // this feed shows the one matching the resolved palette
                        // and the line's mark. Two measured facts forced this
                        // shape: the linked rev has no `set_bg_color`, so a
                        // colour cannot be set after mounting; and a `#(...)`
                        // spliced `draw_bg.color` delivers NOTHING (an A/B in
                        // one run: hardcoded `#00ff00` painted, the spliced
                        // layer beside it stayed the panel colour), so the hex
                        // has to be authored in the script.
                        let dark = crate::screens::theme::resolved() == "dark";
                        let (add_id, del_id) = if dark {
                            (
                                live_id!(review_line_bg_add_dark),
                                live_id!(review_line_bg_del_dark),
                            )
                        } else {
                            (
                                live_id!(review_line_bg_add_light),
                                live_id!(review_line_bg_del_light),
                            )
                        };
                        item.widget(cx, &[add_id]).set_visible(cx, *mark == "+");
                        item.widget(cx, &[del_id]).set_visible(cx, *mark == "-");
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
        // #36g device round: on Android the platform marks itself packaged
        // (package_root = Some("makepad"), android.rs:3455) but the packaged
        // script-resource branch is compiled OUT for android (res.rs,
        // #[cfg(not(target_os = "android"))]), so EVERY script resource —
        // the svgs among them — silently lands CxScriptResourceData::Error
        // (the res.rs tail) and DrawSvg draws 0x0 (the phone /snap: every
        // Svg is [0,0,0,0], the shell's own chevron included). package_root
        // is pub (cx.rs:67): clearing it routes loads through load_file_direct —
        // the materialized root's absolute paths, which exist in the app's
        // files dir (the fonts already load from there). A failed svg load
        // is retried on every draw (load_svg never caches a missing
        // handle), so the icons recover on the next frame after this runs.
        #[cfg(target_os = "android")]
        {
            static PKG_ROOT_CLEARED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
            PKG_ROOT_CLEARED.get_or_init(|| {
                if cx.package_root.is_some() {
                    cx.package_root = None;
                    makepad_widgets::log!(
                        "[octoscode] package_root cleared: script resources load from the materialized root (android's packaged branch is compiled out)"
                    );
                }
            });
        }
        self.view.handle_event(cx, event, scope);
        if !self.started {
            self.started = true;
            self.start(cx);
            self.sync_labels(cx);
        }
        match event {
            Event::Signal => {
                // #32h item 1: drain the card taps the NAV global enqueued
                // (eval thread) into the same router the native chrome uses.
                let taps: Vec<String> = NAV_QUEUE.lock().unwrap().drain(..).collect();
                for t in taps {
                    makepad_widgets::log!("[octoscode] nav route: {t}");
                    self.perform_screen_action(&t, None);
                }
                self.sync_labels(cx);
            }
            // #28e3 item 1: track the window width — the responsive layout
            // (center min 420, review overlay when narrow, sidebar hidden
            // below 760) re-derives in `sync_chrome`.
            Event::WindowGeomChange(ev) => {
                self.window_w = ev.new_geom.inner_size.x;
                self.window_h = ev.new_geom.inner_size.y;
                self.sync_chrome(cx);
            }
            // A3: desktop notifications fire only while in the background.
            Event::WindowLostFocus(_) => self.chrome.unfocused = true,
            Event::WindowGotFocus(_) => self.chrome.unfocused = false,
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
                    // #36g item 2: on Android the IME commits Enter as a
                    // newline character INSIDE the text (KEYCODE_ENTER rarely arrives as
                    // a key event — the device swallowed it). The web
                    // composer SENDS on bare Enter and only newlines via
                    // Alt+Enter / Ctrl+J (ComposerInput.tsx:237-243), so a
                    // trailing newline IS the send gesture: strip it and
                    // submit.
                    let (text, submit) = match text.strip_suffix('\n') {
                        Some(stripped) => (stripped.to_owned(), true),
                        None => (text, false),
                    };
                    // Card #28e item 5: "/" typed into an EMPTY composer opens
                    // the command palette (board 4 frame 3).
                    if text == "/" {
                        if let Ok(mut u) = self.bridge.lock().unwrap().ui.lock() {
                            u.set_palette_open(true);
                        }
                    }
                    self.bridge.lock().unwrap().ui.lock().unwrap().set_draft_inner(text.clone());
                    // The widget is the source here: remember what it holds
                    // so the external-sync below never writes back over it.
                    self.composer_synced = Some(text.clone());
                    makepad_widgets::log!("[octoscode] draft synced: {} chars", text.len());
                    if submit {
                        makepad_widgets::log!(
                            "[octoscode] composer newline -> submit (IME Enter)"
                        );
                        self.perform_action(cx, bindings::ACTION_SUBMIT, 0);
                    }
                }
                if self.view.button(cx, ids!(refresh)).clicked(actions) {
                    self.perform_action(cx, "session.refresh", 0);
                }
                // #32h item 1 (L4): route the mounted card's wired taps into
                // the screens' tables — the ButtonAction from inside the
                // splash was in this very actions vec, but nothing queried it.
                // #35b: dispatch each tap to the resolver that OWNS its id
                // (`taps::owner_of`). setup-01's ids belong to connect::resolve;
                // every other docked card authors conversation/chrome ids
                // (setup-11's `error.reload`/`error.copy_diagnostics`), which
                // `perform_screen_action` would resolve to Unhandled. The
                // one-owner routing inside perform_action is unchanged.
                // #35d: clone the tap list before the loop — the taps are
                // borrowed from self, and `perform_action` now takes &mut self
                // (it needs `cx` for the clipboard), so the immutable borrow
                // would otherwise span the call (E0502). The list is a handful
                // of (id, action) pairs and only changes on a remount.
                let screen_taps = self.screen_taps.clone();
                for (id, ev) in &screen_taps {
                    if self
                        .view
                        .button(cx, &[live_id!(screen_splash), *id])
                        .clicked(actions)
                    {
                        makepad_widgets::log!("[octoscode] card tap: {ev}");
                        match screens::taps::owner_of(ev) {
                            screens::taps::Owner::Connect => {
                                self.perform_screen_action(ev, None);
                            }
                            screens::taps::Owner::Action => {
                                // #FX1 — the row the tapped control addresses.
                                // It is NOT the widget name and NOT a guessed
                                // default: the shared wiring path stamped it
                                // onto the action id from the card's own control
                                // naming (`taps::row_of`), because `screen_taps`
                                // carries (widget, action) and no row at all.
                                // A control with no row suffix keeps the old
                                // behaviour (row 0), which is correct for every
                                // single-shot control.
                                let (base, row) = screens::taps::split_row(ev);
                                self.perform_action(cx, base, row.unwrap_or(0));
                            }
                        }
                    }
                }
                // The #16 `new-chat` component, and the host hit target laid over
                // it.
                //
                // #35c: the component draws its OWN Button over the host target
                // (`/d`: i0_newchat_1 [66,139,225,76] over new_chat_hit
                // [66,142,225,70]), and makepad gives the press to the first
                // widget in traversal order containing the point — so the host
                // target underneath never fires. #35b's "declare the host target
                // first" fix was measured NOT to work (2f69122, reverted
                // 795dcb2), so order is not the lever.
                //
                // The lever is that the host CAN query a widget inside the
                // mounted Splash: the composer already does exactly this
                // (`text_input(cx, &[live_id!(i0_composer_0)]).changed(...)`
                // below reads a component-owned widget's action). So route the
                // COMPONENT's own Button, the way #32h/#35b route card taps,
                // and keep the host target as the no-component fallback.
                let new_chat_clicked = self.view.button(cx, ids!(new_chat_hit)).clicked(actions)
                    // i0_newchat_0/_1 are the component root's own children
                    // (`/d` 68 nodes: i0_newchat > i0_newchat_0 > i0_newchat_1);
                    // query each by id so whichever one owns the press routes it.
                    || self.view.button(cx, &[live_id!(i0_newchat_0)]).clicked(actions)
                    || self.view.button(cx, &[live_id!(i0_newchat_1)]).clicked(actions)
                    || self.view.button(cx, &[live_id!(i0_newchat)]).clicked(actions);
                if new_chat_clicked {
                    makepad_widgets::log!("[octoscode] new_chat clicked");
                    self.perform_action(cx, bindings::ACTION_NEW_CHAT, 0);
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
                        self.perform_action(cx, bindings::ACTION_INTERRUPT, 0);
                    } else {
                        self.perform_action(cx, bindings::ACTION_SUBMIT, 0);
                    }
                }
                // #P4a1 — the approval pill: cycle the permission mode and
                // reflect the server's read-back (the web's
                // permission/profile/set, permissions-section.tsx:27-28;
                // #42a owns the protocol side).
                if self.view.button(cx, ids!(approval_pill_hit)).clicked(actions) {
                    let conv = {
                        let b = self.bridge.lock().unwrap();
                        b.conv.clone()
                    };
                    if let (Some(rt), Some(conv)) = (self.runtime.as_ref(), conv) {
                        screens::workspace::spawn(
                            screens::workspace::Effect::CyclePermissionMode,
                            rt,
                            conv,
                        );
                    }
                }
                // `+` (attach) and the mic are not wired to a protocol method
                // yet; they are present as hit targets so the component's own
                // chrome stays clickable and the ids exist for a later card.
                // Card #21 §3 — the per-item controls. A row click is routed
                // WITH its item id (the same `items_with_actions` contract the
                // makepad examples use).
                // A3 (board 2): every chrome click — the header (menu,
                // Review, Settings), the sidebar (New chat, search, modes,
                // sort, the tree's rows / groups / "⋯", Add workspace), the
                // workspace menu, Settings and the Stop dialog — routed
                // through the one-owner tables (`chrome::Intent`).
                self.handle_chrome(cx, actions);
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
                            self.perform_action(cx, action, row.index);
                        } else {
                            self.perform_action(cx, action, 0);
                        }
                    }
                }
                // Card #28e — the board-4 chrome controls. #40b: the sidebar
                // header's closed-state openers route the same toggles.
                // A3: the header's Review / Settings openers and the
                // Settings close route through `handle_chrome` above.
                if self.view.button(cx, ids!(review_toggle_hit)).clicked(actions)
                    || self.view.button(cx, ids!(review_close)).clicked(actions)
                {
                    self.perform_action(cx, "review.toggle", 0);
                }
                // #34a row 165 — the drawer's General section carries the
                // server connection action (product.spec.ts:1435). The native
                // Disconnect flips the store's connection row to Offline —
                // the same state the transport-loss path sets — and logs;
                // never a silent no-op.
                if self.view.button(cx, ids!(settings_disconnect)).clicked(actions) {
                    let b = self.bridge.lock().unwrap();
                    b.store.set_connection("Offline".to_owned(), false);
                    ::log::info!(
                        "octoscode: settings.disconnect — connection set Offline \
                         (reconnect via the connect screen)"
                    );
                }
                // #31a -> A3: the <760 menu trigger now lives in the header
                // and opens the drawer (`drawer.open`, handle_chrome).
                self.sync_labels(cx);
            }
            // Card #31e — the keyboard surface routes through the ONE
            // resolver (`screens::keys::resolve`); every rule cites its web
            // source there. #28e's chrome chords are preserved in the table.
            // #36g device round: the 6T never sees a key event for the
            // keyboard's Enter and the IME commits no trailing newline (the
            // /log shows draft synced 1..8 then send_hit clicked, no submit
            // line). The platform route that DOES fire:
            // performEditorAction (MakepadInputConnection.java:832) ->
            // onImeEditorAction -> android.rs constructs Event::ImeAction
            // (:1491-1492). The action button on the single-line composer IS
            // Enter (the web: bare Enter submits, ComposerInput.tsx:237-243).
            // Android-only: desktop submits via KeyDown ReturnKey already.
            #[cfg(target_os = "android")]
            Event::ImeAction(_) => {
                // Bind the value FIRST: the tail expression's temporary
                // FlowUi guard borrows through `b`, so it outlives the
                // inner block where `b` drops (E0597 on the android target
                // — the desktop cfg never compiled this arm).
                let palette_open = {
                    let b = self.bridge.lock().unwrap();
                    let open = b.ui.lock().unwrap().palette_open();
                    open
                };
                if palette_open {
                    makepad_widgets::log!("[octoscode] ime action ignored (palette open)");
                } else {
                    makepad_widgets::log!("[octoscode] ime action -> submit (IME Enter)");
                    self.perform_action(cx, bindings::ACTION_SUBMIT, 0);
                }
            }
            Event::KeyDown(e) => {
                // A3: Escape closes the top-most chrome surface first.
                if e.key_code == KeyCode::Escape && self.escape_chrome(cx) {
                    return;
                }
                let (ui, store, conv) = {
                    let b = self.bridge.lock().unwrap();
                    (b.ui.clone(), b.store.clone(), b.conv.clone())
                };
                let (palette_open, turn_active, draft_empty) = {
                    let u = ui.lock().unwrap();
                    (u.palette_open(), u.turn_active(), u.draft().is_empty())
                };
                // Y/S/N gate on the AUTHORITATIVE pending list — the store's
                // approval domain, where the client's `approval/requested`
                // handler lands pushed cards — or the flow's own flag
                // (module-driven transports). The FlowUi flag alone missed
                // server-pushed cards (the first live drive's dead Y).
                let approval_pending =
                    crate::screens::keys::oldest_pending_id(&store).is_some()
                        || ui.lock().unwrap().approval_pending();
                // #P4f2 row 7: the SHOWING approval's diff preview id, from the
                // same FIFO row the decision keys answer, so `D` and Y/S/N can
                // never act on different cards.
                let approval_preview = crate::screens::keys::preview_id(&store);
                let action = crate::screens::keys::resolve(
                    e.key_code,
                    e.modifiers.shift,
                    e.modifiers.control,
                    e.modifiers.alt,
                    e.modifiers.logo,
                    palette_open,
                    approval_pending,
                    approval_preview,
                    turn_active,
                    draft_empty,
                );
                use crate::screens::keys::KeyAction;
                // The per-binding receipt: /log names every resolved key (the
                // docs/keyboard.md table's live evidence).
                makepad_widgets::log!("[octoscode] key {:?} -> {:?}", e.key_code, action);
                let mut open_changed = false;
                match action {
                    KeyAction::PaletteClose => {
                        ui.lock().unwrap().set_palette_open(false);
                        open_changed = true;
                    }
                    KeyAction::PaletteToggle => {
                        ui.lock().unwrap().toggle_palette();
                        open_changed = true;
                    }
                    // "/"-open is IDEMPOTENT (set true): the composer's
                    // changed-action may perform the same open (#28e item 5).
                    KeyAction::PaletteOpen => {
                        ui.lock().unwrap().set_palette_open(true);
                        open_changed = true;
                    }
                    KeyAction::PaletteMove(dir) => {
                        // The screen's table owns the semantics; the shell
                        // only names the action id (one-owner).
                        let ctx = crate::bindings::Ctx::new(&store, &ui);
                        // usize::MAX casts to -1: the table's rem_euclid walk.
                        let idx = if dir < 0 { usize::MAX } else { 1 };
                        let _ = crate::screens::palette::resolve("palette.move", idx, &ctx);
                        open_changed = true;
                    }
                    KeyAction::PaletteRun => {
                        let ctx = crate::bindings::Ctx::new(&store, &ui);
                        let effect = crate::screens::palette::resolve(
                            "palette.run",
                            crate::screens::palette::selected_row(),
                            &ctx,
                        );
                        // `palette::resolve` returns the screen's own Effect
                        // (not the wrapped actions::Effect::Screen).
                        if let crate::screens::palette::Effect::Run(effect_id, name) = effect {
                            // The web closes the palette when a command runs
                            // (CommandPalette's run path dismisses the listbox).
                            ui.lock().unwrap().set_palette_open(false);
                            open_changed = true;
                            if let (Some(rt), Some(conv)) = (self.runtime.as_ref(), conv) {
                                match effect_id {
                                    Some("session.refresh") => {
                                        rt.spawn(async move {
                                            if let Err(e) = conv.refresh_sessions().await {
                                                ::log::warn!(
                                                    "octoscode: palette.run {name}: session.refresh: {e}"
                                                );
                                            }
                                        });
                                    }
                                    // Fail closed with the command's own name
                                    // (the #29d rule): never a silent no-op.
                                    other => ::log::warn!(
                                        "octoscode: palette.run {name}: no native effect ({other:?})"
                                    ),
                                }
                            }
                        }
                    }
                    KeyAction::ComposerSubmit => {
                        // :237-243 — the bare Enter sends the draft (the same
                        // production path the composer's send affordance takes).
                        if let (Some(rt), Some(conv)) = (self.runtime.as_ref(), conv) {
                            rt.spawn(async move {
                                if let Err(e) = conv.submit_draft().await {
                                    makepad_widgets::log!("[octoscode] submit dropped: {e}");
                                }
                            });
                        }
                    }
                    KeyAction::Interrupt => {
                        // :245-252 — Esc with a live turn interrupts it.
                        if let (Some(rt), Some(conv)) = (self.runtime.as_ref(), conv) {
                            let turn = ui.lock().unwrap().active_turn();
                            if let Some(turn) = turn {
                                rt.spawn(async move {
                                    if let Err(e) = conv.interrupt(&turn).await {
                                        ::log::warn!("octoscode: turn.interrupt: {e}");
                                    }
                                });
                            }
                        }
                    }
                    // ApprovalPanel.tsx:46-54 — the keyboard decides the
                    // pending approval over the production wire
                    // (`approval/respond`, the r5-turn recording's grammar).
                    KeyAction::ApprovalApproveRequest
                    | KeyAction::ApprovalApproveSession
                    | KeyAction::ApprovalDenyRequest => {
                        if let (Some(rt), Some(conv)) = (self.runtime.as_ref(), conv) {
                            let approval_id = crate::screens::keys::oldest_pending_id(&store);
                            if let Some(approval_id) = approval_id {
                                let body = crate::screens::keys::respond_body(
                                    &action,
                                    &conv.session_id(),
                                    &approval_id,
                                );
                                if let Some(body) = body {
                                    let client = conv.client().clone();
                                    rt.spawn(async move {
                                        if let Err(e) =
                                            client.request("approval/respond", body).await
                                        {
                                            ::log::warn!("octoscode: approval/respond: {e}");
                                        }
                                    });
                                }
                            } else {
                                ::log::warn!("octoscode: keyboard decision: no pending approval");
                            }
                        }
                    }
                    // #P4f2 row 7 — `ApprovalPanel.tsx:45`: D opens the diff
                    // review for the SHOWING approval. Hand the preview id to
                    // the review screen (the same `diff/preview/get` production
                    // path `ScopeCycle` uses), then reveal it. A non-diff
                    // approval never reaches here: `resolve` returns `Ignore`
                    // when the id is absent, like the web's `&& previewId`.
                    KeyAction::ApprovalReviewDiff(preview_id) => {
                        crate::screens::review::set_preview_id(preview_id);
                        if let (Some(rt), Some(conv)) = (self.runtime.as_ref(), conv) {
                            let conv = conv.clone();
                            rt.spawn(async move {
                                let e = crate::screens::review::Effect::ScopeCycle;
                                if let Err(err) = crate::screens::review::perform(e, &conv).await {
                                    ::log::warn!("octoscode: D diff review: {err}");
                                }
                            });
                        }
                        ui.lock().unwrap().toggle_review();
                    }
                    // registry.ts:614 — the parity shortcut resolves; the
                    // approval surface it reveals lands with the approval
                    // Stage-C screen (logged, never silent).
                    KeyAction::ShowApproval => {
                        makepad_widgets::log!(
                            "[octoscode] Alt+A show-approval (pending={approval_pending})"
                        );
                    }
                    KeyAction::ReviewToggle => {
                        ui.lock().unwrap().toggle_review();
                    }
                    KeyAction::SettingsToggle => {
                        ui.lock().unwrap().toggle_settings();
                    }
                    KeyAction::Ignore => {}
                }
                if open_changed {
                    self.sync_labels(cx);
                    // The palette's rows (and the highlight that follows the
                    // LIVE selection) draw in `draw_walk` — an explicit redraw
                    // or the move paints only on some future frame.
                    self.view.redraw(cx);
                }
            }
            _ => {}
        }
    }
}

// #32h item 1: the lowered cards' tap targets emit `on_click: || { NAV(t:
// "connect") }` (octoscript-makepad lib.rs:450) and NAV is "a global the
// host registers" (fork lib.rs:359) — nobody did. An unregistered global
// evaluates to NIL (kit.rs:141-143), so every card tap silently did
// nothing: no connection, no log (the phone symptom). The callback runs on
// the eval thread: log, enqueue, wake; the Signal arm drains into the SAME
// router the native chrome uses (perform_screen_action → the screens'
// table → Effect::Connect).
static NAV_QUEUE: Mutex<Vec<String>> = Mutex::new(Vec::new());

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
        // #32f: seed the design root from the host's files dir BEFORE any
        // design read — register() itself reads the component ledger below
        // (`components::log_resolutions`), and on the phone root() would
        // otherwise bake the unwritable temp fallback into the OnceLock
        // (the device log: "no HOME and no host files dir — falling back").
        // Same source OctoSense's ai-host uses (`Host::platform(
        // cx.get_data_dir())` -> /data/user/0/<pkg>/files/octos-home).
        crate::design::set_host_dir(vm.cx_mut().get_data_dir());
        // #32h item 1: the lowered cards' buttons emit `on_click: || { NAV(t:
        // "…") }` (fork lib.rs:450) and NAV is "a global the host registers"
        // (fork lib.rs:359) — nobody did. An unregistered global evaluates to
        // NIL (kit.rs:141-143), so every card tap silently did nothing: no
        // connection, no log (the phone symptom). The callback runs on the
        // eval thread: log + enqueue + wake; the Event::Signal arm drains
        // into the SAME router the native chrome taps use.
        let nav = octoscript_render::add_global_fn(
            vm,
            &[(live_id!(t), makepad_widgets::ScriptValue::NIL)],
            |vm, a| {
                let t = octoscript_render::string_prop(vm, a, live_id!(t)).unwrap_or_default();
                makepad_widgets::log!("[octoscode] nav tap: {t}");
                NAV_QUEUE.lock().unwrap().push(t);
                SignalToUI::set_ui_signal();
                makepad_widgets::ScriptValue::NIL
            },
        );
        vm.set_injected_global(live_id!(NAV), nav);
        // #M1: the recents store is installed from RUST, before the DSL is
        // evaluated — see `init_recents_persistence` for why it cannot live in
        // the `script_mod!` body (it is not DSL, and `#{ … }` fails too because
        // `script_mod!` splices a script expression, not a unit block).
        init_recents_persistence();
        // A3: the board-2 chrome templates (`mod.widgets.Oc*`) must exist
        // before the shell's own DSL below instantiates them.
        chrome::script_mod(vm);
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
        // The host honours these as grants (AppCard declares the same pair,
        // apps/appcard/module/src/lib.rs): the module holds the WebSocket to
        // the octos serve (net), and its state — sessions, settings, the
        // trace — lives on disk under the instance's storage jail (storage).
        &["storage", "net"]
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
            screens: Arc::new(Mutex::new(screens::connect::ConnectUi::default())),
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
