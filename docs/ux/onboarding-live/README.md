# Onboarding, LIVE on a fresh octos serve (integrator, 2026-10-02, build 26440b33)

`tools/judge/live_onboarding.py`: a private `octos serve` (a6ea8505) with an EMPTY data dir; the app's first launch with
isolated state and no profile id. 11/11 (`desktop/checks.txt`):

1. First launch: `launch/resolve` with no profile id answers `no_profile`; the onboarding panel shows with the live catalog
   (`01-onboarding.png`).
2. minimax / MiniMax-M3 / Official API picked by click; the operator's project MiniMax Token Plan key typed into the
   masked field (`02-filled-masked.png`: 125 bullets; no laid-out text carries the key).
3. Test, save & open: the server tested the route, saved it, and opened the coding Session (`03-session.png`: strip and
   seat read MiniMax-M3).
4. The next prompt streamed and rendered "2 + 3 = 5." (`04-answer.png`).

Protocol trace: `desktop/trace.jsonl`. The app scrubs the typed key from every traced frame (`trace::register_secret`,
test `a_registered_provider_key_never_reaches_the_trace_file`).

Earlier attempts on the same day, kept as facts, not evidence:
- One run sent the project pay-as-you-go MiniMax key to the Anthropic route, because the script picked the provider list
  wrong. The script now aborts before typing any key unless every selection landed.
- The pay-as-you-go key then reached MiniMax and got HTTP 402 "insufficient balance". The panel showed the failure and
  kept the created profile, so a retry would only repeat the test and save (row 95's branch, against a real server).

Secret handling (supervisor's rules):
- The key lived in a mode-600 file and was sent to the app by one direct instrument call. It was never printed.
- After the run, every text file the run wrote was scanned for the key and its first/last 8 characters: 0 hits. That
  covered the outputs, the app's isolated state and the harness logs.
- Captures: the header path is painted over.
- Trace: machine paths read `<scratch>` / `<home>`. Before committing it was re-checked for the key, its head and tail,
  and the serve token: none present.
- App-state dirs and harness logs are not committed. The serve's data dir, which holds the saved key in its profile
  config, was deleted with the key file.

Tooling note: the dev instrument's `/snap` reports a TextInput's raw buffer as `val`, the masked key included. That's an
instrument property, not product UI. Never save `/snap` JSON after typing a secret; this script saves none.
