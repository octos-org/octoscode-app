# Android build baseline: OctoSense phone packaging with the octoscode module

Card **#20**. Branch `task/20` (from `origin/main` @ `c2468a5`). Built in an APFS copy of
`ref/OctoSense` at `tmp/oS20` (never in `ref/`). Machine-readable rows: `docs/phase3/android-baseline.csv`.

Goal: build the OctoSense **phone** APK with the Phase-1 octoscode module (`app-octoscode`) linked, group
every error by cause, fix only **our** crates, and record the toolchain and results.

**Outcome: built, `aarch64`, two APKs (with and without the bundled octos kernel). Zero errors from our
crates; no Android fix to our crates was needed.**

## 1. Toolchain found

| Piece | Value | Evidence |
| --- | --- | --- |
| `cargo-makepad` | `~/.cargo/bin/cargo-makepad` (built from `Octoscript-AppCard/makepad/tools/cargo_makepad`) | `cargo makepad android --help` |
| Android rust target | `aarch64-linux-android` installed | `rustup target list --installed` |
| NDK | `28.2.13676358`, bundled under `.../cargo_makepad/android_33_macos_aarch64/ndk` | `ls …/android_33_macos_aarch64/ndk` |
| NDK clang | `aarch64-linux-android33-clang` (**versioned** — the NDK stopped shipping the unversioned name) | `ls …/llvm/prebuilt/darwin-x86_64/bin` |
| Bundled SDK | `platforms/android-33-ext4`, `build-tools/33.0.1`, `platform-tools`, `openjdk 17.0.2` | `ls …/android_33_macos_aarch64`; `…/openjdk/bin/java -version` |
| `JAVA_HOME` | `/Applications/Android Studio.app/Contents/jbr/Contents/Home` (JBR 21.0.10). `/usr/bin/java` is **absent** | `java -version` |
| `ANDROID_HOME` / `NDK_HOME` | **unset** — `cargo-makepad` reads its own bundled SDK; we pass `--sdk-path=` at it | `env | grep ANDROID` |
| Disk | 533 GB free before, 518 GB after (build targets 6.4 GB) | `df -g /Users` |

Nothing needed installing: the NDK/SDK/JDK are all in `cargo-makepad`'s bundled toolchain. The one gap —
the system `java` — is filled by the bundled `openjdk`, so `JAVA_HOME` was set to Android Studio's JBR
only for the java/javac steps the packager runs; the build never needed a system JDK.

## 2. Prep (all in `tmp/oS20`)

```sh
# pinned framework sources (.sources/): makepad, octoscript, octoscript-makepad
python3 tools/setup.py                       # rc=0; wrote native-runtime.lock.json + .sources/*
# registration recipe (i) — root Cargo.toml, crates/shell/{Cargo.toml,src/apps.rs}, desktop/Cargo.toml,
# phone/Cargo.toml + the new apps/octoscode/{client,store,module}
git apply --check integration/octosense-register.patch   # rc=0 (also clean on a pristine ref clone)
git apply          integration/octosense-register.patch
# D10a: the generic OutboundCommand::Request, applied IN-TREE (the tree path-deps the transport)
git apply --index patches/octosense/0001-transport-generic-request.patch   # rc=0
```

The register patch already forwarded `app-octoscode` to `desktop/Cargo.toml` but **not** to the phone
packaging; this card added the phone hunk (below) and regenerated the patch.

## 3. Commands run (verbatim results)

```sh
# our crates, cross-compiled — the card's "fix only our crates" gate
cargo check -p octoscode-module --target aarch64-linux-android
#   Finished `dev` profile … in 40.38s        (0 errors, 0 warnings from our crates)

# phone APK, no kernel
cargo makepad android --sdk-path=<bundled> build -p octosense-home --features app-octoscode
#   Finished `dev` profile … in 4m 56s
#   Building APK (package=dev.makepad.octosense, label=OctoSense, versionCode=1, versionName=0.1.0, debuggable=true)
#   APK Build completed            BUILD_EXIT=0

# the octos kernel (the supported bundle path)
python3 tools/kernel-artifact.py --sdk <bundled>
#   Finished `release` profile … in 13m 49s
#   octos kernel: …/target/octos-kernel/target/aarch64-linux-android/release/octos    KERNEL_EXIT=0

# phone APK, kernel bundled
MAKEPAD_ANDROID_EXTRA_LIBS=liboctos.so=<octos> cargo makepad android --sdk-path=<bundled> build -p octosense-home --features app-octoscode
#   Bundled extra native lib: liboctos.so (for arm64-v8a)
#   APK Build completed            APK2_EXIT=0
```

## 4. Result

| | no kernel | kernel bundled |
| --- | --- | --- |
| APK | `tmp/oS20/target/makepad-android-apk/octosense_home/apk/octo_sense.apk` | same path (rebuilt) |
| Size | 214,865,794 B (208 MB) | 257,484,745 B (256 MB) |
| `lib/arm64-v8a/` | `libmakepad.so`, `libstd-04b6e765d4ef8976.so` | + `liboctos.so` (129,905,784 B) |

`aapt dump badging` (kernel build):
```
package: name='dev.makepad.octosense' versionCode='1' versionName='0.1.0' compileSdkVersion='33' platformBuildVersionCode='33'
sdkVersion:'26'   targetSdkVersion:'35'
application-label:'OctoSense'   launchable-activity: name='dev.makepad.octosense.MakepadApp'
native-code: 'arm64-v8a'
```

The kernel binary: `tmp/oS20/target/octos-kernel/target/aarch64-linux-android/release/octos` — 129,905,784 B,
`ELF 64-bit LSB pie executable, ARM aarch64 …, interpreter /system/bin/linker64`.

**Not installed on a device** (card step 4): `adb devices` may list the operator's phone; installing is the
operator's call.

## 5. Errors, grouped by cause

| Cause | Exact error | Where | Fix |
| --- | --- | --- | --- |
| **makepad-Android — buildtool arg** | `Error: "Failed to get crate dir for: app-octoscode"` | `cargo makepad` | `cargo-makepad` takes the **first positional arg** as the build crate (`cargo_makepad/src/utils.rs:390`); pass `-p octosense-home` and `--features app-octoscode`. **Our invocation**, not a code bug. |
| **makepad-Android — cc-rs / NDK** | `cc-rs: failed to find tool "aarch64-linux-android-clang"` | `ring` build script | The NDK ships only versioned clang. Export the `CC_/CXX_/AR_/RANLIB_aarch64_linux_android` + linker vars at `…/bin/aarch64-linux-android33-clang` (exactly `apps/appcard/docs/BUILDING-ANDROID.md:71-75`). **Environment**, not a code bug. |
| **stdio transport on Android** | none observed | — | the transport cross-compiled clean |
| **octos-cli embedding / kernel** | none — `octos-cli` cross-built clean | `kernel-artifact.py` | none |
| **our crates** | none — 0 errors, 0 warnings | `cargo check -p octoscode-module --target aarch64-linux-android` | none needed |

Only two real build-setup errors, both green after the fix; **no product-code change to our crates**.

## 6. Ours vs upstream

- **Ours** (`apps/octoscode/{client,store,module}`): compile for `aarch64-linux-android` with **zero
  warnings and zero errors**. No Android-specific breakage.
- **Upstream / framework**: 2 dead-code warnings (`makepad-platform`, `makepad-filesystem-watcher`) and 4
  `javac` deprecation warnings inside `MakepadActivity.java` (GPS `LocationListener`, `Notification.Builder`,
  `CameraDevice.createCaptureSession`, `WebViewClient`). All pre-existing, none ours.
- **Kernel integration**: plain `cargo makepad` bundles only `libmakepad.so` + `libstd`; `liboctos.so` is
  added by the supported path `tools/kernel-artifact.py`, which exports `MAKEPAD_ANDROID_EXTRA_LIBS`
  (`kernel-artifact.py:20`). This card ran it to completion.

## 7. The patch this card extends

The register patch's phone hunk (now in `integration/octosense-register.patch`, applies clean to a pristine
`ref/OctoSense`):

```diff
--- a/phone/Cargo.toml
+++ b/phone/Cargo.toml
@@ -86,6 +86,8 @@ app-reference = ["dep:octosense-reference", "octosense-shell/app-reference"]
 app-sheets = ["octosense-shell/app-sheets"]
 app-photos = ["octosense-shell/app-photos"]
 app-appcard = ["octosense-shell/app-appcard"]
+# Phase-1 core: the octoscode client module (recipe (i) forwarded to the phone packaging).
+app-octoscode = ["octosense-shell/app-octoscode"]
 app-news = ["octosense-shell/app-news"]
 app-maps = ["octosense-shell/app-maps"]
 app-aichat = ["dep:makepad-aichat", "octosense-shell/app-aichat"]
```

## 8. Next steps

1. **Device run** (operator's call): install `octo_sense.apk` on an arm64 phone and confirm the
   `android-appcard-build.md` log lines — `octos: kernel service ready …` then
   `octos-core: starting kernel 1: …/lib/arm64/liboctos.so serve --stdio`. **Unverified on device.**
2. **Icons** (upstream): `cargo makepad` warns on missing launcher/app icons; add `mipmap-*/ic_launcher.png`
   or pass `--no-icon`. Not ours.
3. The phone hunk should travel with the rest of `integration/octosense-register.patch` when vendored.
