# Build baseline: OctoSense + its octos-app client

Task #3. A known-red list for every build/test we gate on, the cheapest correct way for a **new repo**
(this one) to reuse `octos-app-transport` / `octos-app-store`, and what a new system app needs to be
registered and run in the OctoSense desktop shell.

Method: cloned the read-only ref at
`/Users/yuechen/home/oa.noindex/ref/OctoSense` (HEAD `6e9bfd4077cf8181878ac005ad586f908084a4bf`,
branch `main`) to `tmp/octosense` (`cp -c -R`, APFS clone), built **only** in `tmp/octosense` with
`CARGO_TARGET_DIR=$PWD/tmp/octosense-target`. Nothing was built inside `oa.noindex/ref/*`. Every
`cargo test` ran through the host slot wrapper `/Users/yuechen/home/octoscode-app/outer/scripts/ctest`.
Android / phone / ROM builds were **skipped** on purpose (the card says to).

## Host

| Probe | Verbatim result |
| --- | --- |
| `uname -a` | `Darwin Mac 25.5.0 Darwin Kernel Version 25.5.0: Mon Apr 27 20:39:42 PDT 2026; root:xnu-12377.121.6~1/RELEASE_ARM64_T6031 arm64` |
| `rustc -V` | `rustc 1.95.0 (59807616e 2026-04-14)` |
| `cargo -V` | `cargo 1.95.0 (f2d3ce0bd 2026-03-21)` |
| `python3 -V` | `Python 3.14.6` |

Toolchain note: cargo warns `kstring v2.0.5 (requires Rust 1.96.0)` during probe resolution; the pinned
graph builds fine on 1.95.0 because it does not select that version. **Not a blocker.**

## Build/test entry points (step 1, cited)

| Entry point | Where | Cites |
| --- | --- | --- |
| Prepare framework sources (`.sources/`) | `python3 tools/setup.py` from the repo root | `tools/setup.py:1-26`, `AGENTS.md:25` |
| Verify one-Makepad/one-octos graph (CI) | `python3 tools/setup.py --check --cargo` | `tools/setup.py:23-24`, `AGENTS.md:30` |
| AppCard crates check | `cargo check --locked -p octos-app` | `apps/appcard/app/README.md:34`, `apps/appcard/AGENTS.md:41-44` |
| AppCard tests (what CI runs) | `cargo test --locked -p octos-app-transport -p octos-app-store` | `apps/appcard/AGENTS.md:43-44` |
| AppCard tests (the app itself) | `cargo test --locked -p octos-app` | `apps/appcard/AGENTS.md:44` |
| `make check` | runs `cargo check --workspace` (root workspace now, not AppCard's) | `apps/appcard/app/Makefile:30-31`, `apps/appcard/app/README.md:40-42` |
| Desktop packaging check | `cargo check --locked -p octosense` (and `--features mobile-apps`) | `AGENTS.md:12`, `desktop/README.md` |
| Shell crate check | `cargo check --locked -p octosense-shell` | `AGENTS.md:12` |
| Shell graph (no foreign runtime crate) | `bash tools/check-shell-graph.sh -p octosense` | `AGENTS.md:12` |
| Desktop build with the octos-app client | `cargo build --features app-appcard -p octosense` | `desktop/Cargo.toml:66`, `crates/shell/Cargo.toml:92` |

`.sources/` did not exist in the clone; `python3 tools/setup.py` created it (200 s) and printed the
three pinned revisions: octoscript-makepad `6881fb6c…`, makepad `6cf03859…`, octoscript `68f6a9df…`
(`native-runtime.lock.json`, `runtime-patches.lock.json`).

## Step 2 — AppCard baseline

All commands run from `tmp/octosense` with `CARGO_TARGET_DIR=$PWD/../octosense-target` unless noted.

| Target | Command | Verbatim result | Pass / fail |
| --- | --- | --- | --- |
| `octos-app` (check) | `cargo check --locked -p octos-app` | ``Finished `dev` profile [optimized + debuginfo] target(s) in 14m 52s`` | PASS |
| `octos-app-store` + `octos-app-transport` (test) | `ctest --locked -p octos-app-transport -p octos-app-store` | `test result: ok. 45 passed; 0 failed; …` + `test result: ok. 30 passed; 0 failed; …` (+ 4 more targets) | PASS |
| `octos-app` (test) | `ctest --locked -p octos-app` | `test result: FAILED. 193 passed; 7 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.29s` | **FAIL (known-red)** |
| whole workspace | `make check` (from `apps/appcard/app`) | ``Finished `dev` profile [optimized + debuginfo] target(s) in 48.96s`` | PASS |

Per-target detail for the store/transport run (6 targets):

```
test result: ok. 45 passed; 0 failed; 0 ignored; ...   (octos-app-store lib)
test result: ok. 30 passed; 0 failed; 0 ignored; ...   (octos-app-transport lib)
test result: ok. 1 passed;  0 failed; 0 ignored; ...   (contract_tool_started)
test result: ok. 1 passed;  0 failed; 0 ignored; ...   (kernel_restart)
test result: ok. 0 passed;  0 failed; 1 ignored; ...   (live_smoke, ignored)
test result: ok. 0 passed;  0 failed; 0 ignored; ...   (x2 doc-test targets)
```

### Known-red list (base = untouched `main` clone)

These 7 `octos-app` lib tests fail **on a clean clone of `main`**, so they are pre-existing, not a
regression from this campaign. Record as known-red; do not gate a diff on them.

| Failing test | Failure line (verbatim) |
| --- | --- |
| `app::l0_card::capability_bridge::every_call_the_lowering_emits_has_a_helper` | `apps/appcard/app/app/src/app/l0_card.rs:2298:9: the lowering emits calls this app does not register: [ "convert", "dataset", "l0_math", "l0_ratio", "news_digest", "num", ]` |
| `app::l0_card::exemplar_drift::the_nav_exemplar_is_the_card_the_profile_tests_check` | `apps/appcard/app/app/src/app/l0_card.rs:1227:9: assertion left == right failed: the exemplar the model writes from has drifted from the fixture the tests check` |
| `app::l0_card::tests::l0_theme_axes_are_all_answered` | (see `tmp/test-octosapp.log`) |
| `app::l0_eval::tests::a_live_source_without_its_capability_is_visibly_wrong` | (see `tmp/test-octosapp.log`) |
| `app::l0_eval::tests::a_watch_row_reveals_its_own_remove` | (see `tmp/test-octosapp.log`) |
| `tests::nav_is_generated_from_an_l0_spec` | (see `tmp/test-octosapp.log`) |
| `tests::the_language_reference_lists_every_admitted_theme` | (see `tmp/test-octosapp.log`) |

```
error: test failed, to rerun pass `-p octos-app --lib`
```

## Step 3 — Desktop shell build with the octos-app client

Feature/package names confirmed: the desktop package is `octosense` (`desktop/Cargo.toml:24`) and it
forwards `app-appcard = ["octosense-shell/app-appcard"]` (`desktop/Cargo.toml:66`), which the shell
maps to `["dep:octosense-appcard", "octos-core"]` (`crates/shell/Cargo.toml:92`).

| Target | Command | Verbatim result | Pass / fail |
| --- | --- | --- | --- |
| desktop binary + octos-app | `cargo build --locked -p octosense --features app-appcard` | ``Finished `dev` profile [optimized + debuginfo] target(s) in 12m 23s`` | PASS |
| desktop, default features | `cargo check --locked -p octosense` | ``Finished `dev` profile [optimized + debuginfo] target(s) in 1m 09s`` | PASS |
| shell crate, default features | `cargo check --locked -p octosense-shell` | ``Finished `dev` profile [optimized + debuginfo] target(s) in 17.31s`` | PASS |

The `--features app-appcard` build linked and compiled `octosense-appcard`, `octos-app`,
`octos-app-store`, `octos-app-transport`, `octos-app-render` and `octosense-kernel` (see the build log).
Warnings in the build: 2 pre-existing `file … found to be present in multiple build targets`
(`desktop/src/main.rs`, `phone/src/main.rs`) and 1 upstream `makepad-platform` dead-code warning —
none from OctoSense product code.

Android / phone / OpenHarmony / ROM builds: **not run** (out of scope for this card).

## Step 4 — Dependency probe (new repo reusing the AppCard crates)

Two throwaway crates created **outside** the OctoSense workspace (`tmp/probe/`, each with its own
`[workspace]` so no parent workspace exists). Both depend on `octos-app-store` and
`octos-app-transport`, and their only body is
`let _ = std::any::type_name::<octos_app_store::AppState>();`.

| Approach | Manifest | Command | Verbatim result |
| --- | --- | --- | --- |
| **path** | `octos-app-store = { path = "../../octosense/apps/appcard/app/crates/octos-app-store" }` (same for transport) | `cargo check` | ``Finished `dev` profile [unoptimized + debuginfo] target(s) in 21.96s`` |
| **git** | `octos-app-store = { git = "https://github.com/OctoSense-org/OctoSense", rev = "6e9bfd4077cf8181878ac005ad586f908084a4bf" }` (same for transport) | `cargo check` | ``Finished `dev` profile [unoptimized + debuginfo] target(s) in 20.66s`` |

**Both pass with NO `[patch]` block and no workspace inheritance in the consumer.** The git run fetched
`octosense-kernel`, `octos-app-transport` and `octos-app-store` all at
`…?rev=6e9bfd4077cf8181878ac005ad586f908084a4bf#6e9bfd40`, plus `octos-core` at the pinned octos rev.

Why it works without a patch: the two crates use `workspace = true` internally
(`crates/octos-app-store/Cargo.toml:8-14`), but a path/git dependency **does not inherit the consumer's
workspace** — cargo reads the crate's *own* workspace root (OctoSense's root `Cargo.toml`) and resolves
`workspace = true` and the `[patch]` tables there (`crates/octos-app-transport/Cargo.toml:17-19`;
root `Cargo.toml:153-229` for the Makepad/octos patches). So the consumer needs no `[patch]` of its own.

### Recommendation

**Git dependency at a pinned rev** is the cheapest correct choice for this new repo:

```toml
[dependencies]
octos-app-store = { git = "https://github.com/OctoSense-org/OctoSense", rev = "<40-hex rev>" }
octos-app-transport = { git = "https://github.com/OctoSense-org/OctoSense", rev = "<40-hex rev>" }
```

No `[patch]` and no `makepad`/octos pins are required in the consumer (verified above). Path
dependencies also work but require a sibling checkout and break reproducibility; copy-with-attribution
duplicates code ADR 0001 says lives in one repository. Use **path** only for a local dev loop.

Caveat: pin to a rev that is **reachable from the public remote**. (Outer-loop correction, 2026-09-28: the ref
clone's HEAD `6e9bfd40` is an **ancestor** of public `main` `405139f8`, 7 commits behind, not ahead of it. `main`
simply advanced after the clone. `git merge-base --is-ancestor 6e9bfd4 origin/main` → true.) Pin by `rev`, never
`branch = "main"`, and bump the rev deliberately.

### Minimal working block (verbatim, from `tmp/probe/gitprobe/Cargo.toml`)

```toml
[package]
name = "gitprobe"
version = "0.1.0"
edition = "2021"

[workspace]            # standalone: no parent workspace

[dependencies]
octos-app-store = { git = "https://github.com/OctoSense-org/OctoSense", rev = "6e9bfd4077cf8181878ac005ad586f908084a4bf" }
octos-app-transport = { git = "https://github.com/OctoSense-org/OctoSense", rev = "6e9bfd4077cf8181878ac005ad586f908084a4bf" }
```

## Step 5 — Registering a new system app in the shell

Two shapes. Sources: `crates/shell/src/apps.rs`, `crates/shell/src/ext.rs`,
`crates/shell/src/module_host.rs`, `desktop/config/apps.json`, `apps/README.md`, `desktop/system-apps.json`.

### (i) Native Rust `AppModule` (like AppCard)

1. **Crate + `AppModule` impl.** Implement `makepad_app_module::AppModule` (`id`, `label`,
   `register`, `open_schema`, `capabilities`, `create`) and expose a `static … _MODULE`. Pattern:
   `apps/appcard/module/src/lib.rs:46` (`pub static APPCARD_MODULE`), `:147-172` (`impl AppModule`:
   `id() -> "appcard"` `:148`, `label() -> "AppCard"` `:149`, `register` `:153`, `create` `:172`);
   minimal reference module at `apps/reference/src/lib.rs:61-86`.
2. **Workspace member + root pin.** Add the crate to root `Cargo.toml` `members`
   (`Cargo.toml:26-37`, e.g. `"apps/appcard/module"` at `:35`) and to `[workspace.dependencies]` as a
   `path` (`Cargo.toml:105`: `octosense-appcard = { path = "apps/appcard/module" }`).
3. **Shell dep + feature.** `crates/shell/Cargo.toml`: optional dep
   (`:53` `octosense-appcard = { workspace = true, optional = true }`) + a feature
   (`:92` `app-appcard = ["dep:octosense-appcard", "octos-core"]`).
4. **Register in `linked_modules()`.** Add a `#[cfg(feature = "app-…")] out.push(&…_MODULE);`
   (`crates/shell/src/apps.rs:51` is `fn linked_modules()`; AppCard push at `:69`). Registration in the
   Script VM happens in `module_host.rs`: `module.register(vm)` at `:150`, re-registered per instance at
   `:190`; the shell's tests register it at `apps.rs:644` / `module_host.rs:374`.
5. **(optional) launcher row.** A bundled module becomes a launcher row automatically via
   `bundled_modules_catalog()` (`apps.rs:305-323`); desktop default hosting for a linked module is
   `Process` unless the person sets `~/.makepad/wm/apps.splash` or passes `--module <id>`
   (`apps.rs` module doc `:1-16`, `AppRegistry::load` `:338`, `hosting()` `:378`).
6. **Forward the feature in the packaging** (`desktop/Cargo.toml:66`
   `app-appcard = ["octosense-shell/app-appcard"]`) so `cargo build --features app-appcard` links it.

### (ii) Contained OctoScript app + optional Rust host service (like Mail)

1. **Bundle** `apps/<name>/bundle/`: `manifest.json` + `main.splash` (+ artwork). Example
   `apps/mail/bundle/manifest.json` (`id`, `schema`, `capabilities`, empty `integrity.bundle_blake3`)
   and `apps/mail/bundle/main.splash` (the script; `apps/README.md:11-13`, `:17-19`).
2. **List it in the packaging** so the App Hub shell crate packs it: `desktop/system-apps.json`
   `"apps": ["news","photos","maps","camera","mail","ai-providers"]` (`desktop/system-apps.json:4`;
   `apps/README.md:104-115`). App Hub reads it via `OCTOSENSE_SYSTEM_APPS`, set in root
   `.cargo/config.toml`; it packs each `bundle/` into the binary and fills in the digest.
3. **Host service (only if it needs a socket/credential/device).** A Rust crate under
   `apps/<name>/host-service/` (member in root `Cargo.toml:32`; workspace pin `Cargo.toml:102`), with a
   `register()` (real) and/or `register_demo()` entry — `apps/mail/host-service/src/lib.rs:126`
   (`register`), `:134` (`register_demo`), `:194` (`register_with`). The shell registers it once at
   startup: `crates/shell/src/apps.rs:157` `fn register_host_services()` (called from
   `system_card_apps()` `:131`; Mail/News registered `:161-176`). It is pulled in through the shell's
   `app-hub` feature (`crates/shell/Cargo.toml:83`, which lists `octosense-mail-service` and
   `octosense-news-service`).
4. **Capability grant.** The app calls `host.request("<family>.<method>", …)`; the isolate refuses it
   unless the manifest's `capabilities` grants the family (`apps/mail/bundle/main.splash` uses
   `mail.*`; `apps/README.md:250+`). No launcher row or `linked_modules()` edit is needed — system
   bundles are enumerated by `system_card_apps()` (`apps.rs:127-141`) and hosted by the linked `card`
   module (`module_host.rs:374`), not by a module of their own.

**Not implemented** (as the card says): neither recipe was built out; this is the documented step list
with citations only.

## Not run

- Android / phone / OpenHarmony / ROM builds — out of scope for this card.
- `cargo clippy` — not part of the acceptance commands; the README's clippy line is cited but not run.
- No live `octos serve` / no model spend.
- `bash tools/check-shell-graph.sh -p octosense` — not in the acceptance set; not run.
