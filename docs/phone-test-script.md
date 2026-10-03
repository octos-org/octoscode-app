# OctosCode phone test (operator, on a real Android phone)

Everything in this build was verified on the Mac only: the desktop window, and phones through the 360x780 simulator.
Past phone-only defects showed up only on a real device (card taps, the composer re-mounting on every keystroke, the
keyboard's Enter, dark-mode contrast), so this script targets exactly those. About 20 minutes.

Before you start:
- Install the APK handed over with this build (`octos_code.apk`, Android arm64). It is the daily build: no remote
  control port is open on the phone (decision D10c).
- Have an Octos server the phone can reach: its address and access token, or a pairing link.

Write **OK** or a short note (a screenshot helps) next to each step.

| # | Step | What should happen | Result |
|---|---|---|---|
| 1 | Open OctosCode for the first time. | The Connect card: Server, Access token, Connect, "Pair with a link instead". Nothing is cut off. | |
| 2 | Connect with your server and token (or pair with a link). | The app opens a Session: header, composer, ☰ menu. | |
| 3 | Tap ☰, then a Session in the list. | The drawer opens and closes cleanly, and the Session's history shows. | |
| 4 | Tap the composer and type a sentence with the on-screen keyboard. | Every keystroke appears at once, with no flicker, jump or lost letter. The composer stays above the keyboard. | |
| 5 | Press the keyboard's Enter key, then the send arrow (→). | Enter does what the composer shows (newline or send). The prompt sends, and the answer streams in. | |
| 6 | Tap a tool row, then "Worked for …", then a code block's Copy. | Each one opens and closes, and Copy shows "Copied". Taps land on the first try. | |
| 7 | Send a long prompt and tap Stop while it answers. | The answer stops, and the composer is ready again. | |
| 8 | Start a turn in Session X, open Session Y, and press Stop there. | X keeps running: Stop only acts on the Session you are in. | |
| 9 | Settings ⚙ → Permissions → "Ask for approval". Ask the model to run a shell command (e.g. "run ls"). | An approval card appears. "Approve once" runs it. Repeat with "Deny", which refuses it. | |
| 10 | Settings → General → Desktop notifications → turn on (allow the system prompt). | The toggle reads on. | |
| 11 | Send a prompt, then go to the phone's home screen before it finishes. | A notification appears when the turn ends. Tapping it opens OctosCode in that Session. | |
| 12 | Switch the phone to dark mode (or the theme row at the bottom of the ☰ drawer). | All text is readable, including the send arrow, chips and dialogs. Nothing is invisible. | |
| 13 | Settings → Preferences → Language → 简体中文, then back to English. | The UI switches live, with no clipped Chinese text. It switches back. | |
| 14 | Rotate to landscape and back, once with the keyboard open. | The layout adapts, nothing is cut off, and the composer stays usable. | |
| 15 | On a Session with file changes, tap ± (Review), then type `/btw what changed?` and send. | The diff opens full-screen with highlighted word changes and scrolls sideways. The aside answers above the composer. | |

When you are done, paste the table (or just the failing step numbers with notes) back into the chat. The integrator
records each result in `docs/parity-matrix.csv` (`device_verified`) and fixes what fails.
