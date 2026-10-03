# D10f. OctosCode runs standalone on macOS, built from a fresh clone: DECIDED (A33, 2026-10-02)

The operator: "pull the latest and test on another Mac", and "the app should be able to be ejected out of
octosense and run as standalone". Before A33 a fresh clone could not build the app at all: the module ran only
inside the OctoSense shell, wired in by an uncommitted diff in one local OctoSense checkout, with makepad,
fonts and design files read from this machine's paths.

## Decision

1. **A standalone host**, `crates/octoscode-desktop` (binary `octoscode`): a plain makepad app whose one window,
   titled OctosCode, holds the module's root. It hosts the module exactly as the shell's `module_host.rs` does: an
   isolate of its own (`alloc_splash_vm_with_network(false)`), `register` + `create` in one trusted entry, the root
   linked into the widget tree and drawn and fed events inside the isolate (`module_view.rs`, minus the WM focus
   gate). What the shell gives that the standalone host does not: the WM palette (`makepad_wm_theme::apply` is a
   no-op outside the WM; the module assigns its own theme roles), extra windows (the module opens none), the
   assistant bus (the module has no tools). Both hosts run the same module; `tests/standalone_host.rs` mounts it.
2. **makepad with our patches in this workspace too**: `tools/prepare-makepad-fork.sh` = OctoSense-org/makepad
   `6cf03859` + every `patches/makepad/*.patch` (one commit), and the root `Cargo.toml` `[patch]`es
   `makepad-widgets`, `makepad-app-module` and `makepad-script` to `.forks/makepad-fork`. The suite now compiles
   the notification API (`cfg(makepad_notifications)` is on), the D10c bridge guard and the D10e ellipsis fix.
   OctoSense's own runtime patch (`makepad-settings.patch`) is not taken: the module uses nothing it adds; it is
   Android/GLES and accessibility-readback code plus two widget helpers the module does not call.
3. **`patches/makepad/packaged-file-resource.patch`**: a packaged build (`MAKEPAD_PACKAGE_DIR`) still reads
   `file_resource(<absolute path>)` from the filesystem. Without it every SVG icon the module names by its
   materialized path drew nothing in the bundle (fonts were spared by a direct read in the text layer). Crate
   resources still come only from the package. Makepad unit test included; failing first.
4. **A self-contained bundle**: `tools/package-macos.sh` builds the `app-bundle` profile with
   `MAKEPAD=apple_bundle MAKEPAD_PACKAGE_DIR=makepad`, copies every crate `resources/` of the app's graph into
   `OctosCode.app/Contents/Resources/makepad/<crate>/resources`, writes the Info.plist (OctosCode,
   `org.octos.octoscode`), signs ad hoc and zips with `ditto`. The module's design tree and its own faces and icons
   ride in the binary (build.rs embed) and materialize under `$HOME/.octoscode/design`.
5. **Nothing reads the build machine in a packaged build**: the module records `Cx::package_root` at `register`
   (`design::set_packaged`) and then never uses the checkout's `design/` or its `resources/` (they were live-reload
   and last-resort paths). The kit faces are back in the embed: 66c865ad dropped the `ttf` arm of the whitelist, so a
   fresh HOME had no Inter / Noto Sans SC and fell back to the checkout (failing-first
   `design::tests::the_embed_carries_the_kit_faces`).
6. **The OctoSense-hosted variant from a fresh clone too**: the shell wiring is a tracked patch,
   `patches/octosense/0003-shell-octoscode-module.patch` (applied after 0001/0002 by
   `tools/prepare-octosense-fork.sh`), with its `Cargo.lock` hunk: the three crates' entries and the shell's new
   dependency. Every dependency they use was already pinned by OctoSense's lock, so the hunk only records the
   graph; with it a fresh tree builds exactly the integrator's graph, `cargo build --locked` passes, and a build
   leaves the tree clean. It needs a refresh only when the crates gain a dependency (a normal build still works:
   cargo adds the entry).
7. **One command**: `tools/build-macos.sh [--package] [--octosense] [--release] [--work <dir>]`;
   `docs/BUILD-macos.md` for the other Mac.

## Proof

`docs/ux/a33/clean-room.md`: a fresh `git clone` under another user path with its own HOME and cargo home,
`tools/build-macos.sh --package --octosense` from scratch (596 s, 10 GB; both variants built), times and disk; the
zipped app unzipped into that HOME's Downloads and launched with every build tree unreadable; both the from-source
build (10/10) and the unzipped app (13/13) stream one dsflash turn against a private octos serve
(`tools/walk/a33_other_mac_live.py`). The scripted check, repeatable on any Mac: `tools/check-fresh-clone-macos.sh
<empty dir>`.

Finding (pre-existing, not the host): the module compares the workspace it asked for with the server's canonical
`workspace_root` byte for byte (`flow.rs`, "the server opened another workspace"), so a workspace named through a
symlink (`/tmp/x` vs `/private/tmp/x`) loses its history read and permission seat; the turn itself runs.

## Exit

The `[patch]` sections go when the patches land upstream (D10a-e PR drafts); 0003 goes when OctoSense carries the
octoscode module itself (`docs/upstream/octosense-registration.md`).
