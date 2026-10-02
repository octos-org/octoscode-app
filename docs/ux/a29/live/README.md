# A29 live proof: the /btw aside lands only in the Session that asked

Parity row 6. A real `octos serve` answered one `session/btw` while the app showed another Session.

## How it ran

`python3 tools/judge/a29_live_btw.py <host-bin> 8606 50311`.

- **The serve.** A private `octos serve --solo` on `127.0.0.1:50311`.
  - Its data was a fresh copy of the live-gate data (the dsflash profile, model deepseek-v4-flash).
  - Its workspace was a fresh folder under the system temp directory.
  - Its token sat in a mode-600 file, set only in the serve's environment (`OCTOS_AUTH_TOKEN`) and the app's (`OCTOS_BEARER`).
  - Afterwards the serve was stopped, and the data copy, workspace and token file were deleted.
- **The app.** The host binary ran hidden on port 8606 with isolated state and `OCTOSCODE_TRACE_FILE` recording.
- **The steps.**
  1. Session X (`dsflash:main`) is open. `/btw <question>` is typed into X's composer and read back from the composer before Return.
  2. While the aside answers, a CLICK on the sidebar's **New chat** opens Session Y.
  3. The answer arrives while Y is on screen.
  4. A CLICK on the row that carries the `/btw` marker goes back to X.

## Result

`checks.txt` has 20 checks, all PASS. The key lines from `trace-btw.jsonl`, the app's own frame trace:

| at (ms) | dir | method | session_id |
|---|---|---|---|
| 0 | out | session/open | `dsflash:main` (X) |
| 4385 | out | session/btw | `dsflash:main`, with the question |
| 5201 | out | session/open | `dsflash:api:01a0fea2-…` (Y, the New chat) |
| 6494 | in | session/btw result | `dsflash:main`, with the answer and `model: deepseek-v4-flash` |
| later | out | session/open | `dsflash:main` (back to X) |

- **The call.** The request carried X's id. The reply echoed it, and it arrived 1.3 s **after** Y's open went out.
- **Where the answer landed.** `app-aside.log` shows `aside dsflash:main #1 -> Answered`. The panel stayed hidden in Y.
- **The sidebar.** Exactly one row carried the marker, and it was not Y's (the selected row).
- **Back on X.** The whole answer showed.
- **The token.** A grep of every saved text file for the token (whole, first 8, last 8) found 0 hits.

## Captures

- `1-x-answering.png`: X's panel, Answering…, above X's composer. The scope line names X's workspace and title.
- `2-y-marker-on-x.png`: Y on screen with no aside. X's row carries the `/btw` marker (answered, so no dot).
- `3-x-answered.png`: back on X with the live answer.
  - The panel is held to min(50 % of the window, 480 px): 302 px here.
  - Its body scrolls, so the "not saved" note sits below the fold.

## Model calls: 3 (within the cap)

1. **Run 1** (1 call: the btw). It proved the same ordering: btw out for `dsflash:main` at 4820 ms, Y opened at 5563 ms, the answer at 7080 ms.
   - Its captures rendered the workspace path, which included a home folder, so they were discarded.
   - Its script could not tell the two untitled "New chat" rows apart.
2. **Run 2** (1 call: a one-line titling prompt). The open into the `/tmp` workspace was refused by the app's exact-workspace check (row 204):
   - macOS `/tmp` is `/private/tmp`, and the server answers with the canonical root.
   - So the capabilities were never recorded. The script now uses the resolved path.
3. **Runs 3 and 4** (0 calls). The first was refused as above. The second hit the bridge's "app busy" timeout on typing.
   - The script now waits until the app is responsive.
   - It reads the typed command back before pressing Return, so a queued bridge command is never sent twice.
4. **Run 5** (1 call: the btw). This is the evidence above.

No `/snap` JSON is saved here, because the instrument reports text fields' raw values.
