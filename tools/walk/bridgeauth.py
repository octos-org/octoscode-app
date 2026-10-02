"""D10c: every request to the makepad remote bridge carries its per-launch token.

The bridge writes the token to <tmp>/makepad-remote/port-<PORT>.token (file 0600,
directory 0700) when it starts and refuses any request without it. Importing this
module installs a urllib opener that sends it as `X-Makepad-Token` on every
request to a loopback port that has a token file, so the walk and judge scripts
keep calling urllib.request.urlopen as before. The token is read per request (a
relaunch on the same port writes a new one) and is never printed, logged or saved.
"""
import os
import tempfile
import urllib.parse
import urllib.request

HEADER = "X-Makepad-Token"
LOOPBACK = {"127.0.0.1", "localhost", "::1"}


def _token_dirs():
    dirs = [os.environ.get("TMPDIR"), tempfile.gettempdir()]
    try:
        dirs.append(os.confstr("CS_DARWIN_USER_TEMP_DIR"))
    except (AttributeError, ValueError, OSError):
        pass
    seen = []
    for d in dirs:
        if d and d not in seen:
            seen.append(d)
    return seen


def token_for(port):
    """The running bridge's token for `port`, or None (no bridge there, or an old one)."""
    for d in _token_dirs():
        try:
            with open(os.path.join(d, "makepad-remote", f"port-{int(port)}.token")) as f:
                token = f.read().strip()
        except OSError:
            continue
        if token:
            return token
    return None


class _TokenHandler(urllib.request.BaseHandler):
    handler_order = 100

    def http_request(self, req):
        url = urllib.parse.urlsplit(req.full_url)
        if url.hostname in LOOPBACK and url.port and not req.has_header(HEADER.capitalize()):
            token = token_for(url.port)
            if token:
                req.add_unredirected_header(HEADER, token)
        return req

    https_request = http_request


INPUT_ROUTES = ("/click", "/t?", "/k?", "/m?")


def input_was_queued(path, err):
    """True only when the bridge APPLIED an input but could not acknowledge its frame.

    The bridge answers every error as HTTP 404 with a JSON body. For an input
    route with wait=1 two bodies mean the input was applied (or is queued and
    will be) but no frame acknowledgement came:
    - `{"err":"timeout (app busy ...)"}`: no frame within 5 s (remote.rs ask);
    - `{"err":"requested input frame could not be submitted; retry"}`: the
      input ran (`apply` precedes it) but the wait frame could not be
      presented (remote.rs drain). "retry" means retry the WAIT; re-sending
      the input would apply it twice.
    A caller may continue; the step's own effect assertion decides whether
    the control worked, so a dead control still fails there. Any other error
    (no windows, a gone window, bad parameters, unknown route: nothing was
    applied) is a real failure and must raise.
    """
    if not str(path).startswith(INPUT_ROUTES) or getattr(err, "code", None) != 404:
        return False
    try:
        body = err.read().decode(errors="replace")
    except Exception:  # noqa: BLE001
        body = ""
    applied = "timeout (app busy" in body or "input frame could not be submitted" in body
    if not applied:
        # A real input failure: say what the bridge said (its JSON error,
        # never a token) before the caller raises.
        import sys
        print(f"[bridgeauth] {str(path).split('?')[0]} -> {err.code} {body[:160]!r}", file=sys.stderr)
    return applied


def install():
    urllib.request.install_opener(urllib.request.build_opener(_TokenHandler()))


install()
