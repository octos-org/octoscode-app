"""Scrub a /snap tree before it is saved: the dev instrument reports a TextInput's raw buffer as `val`, masked fields
included. Every saver of snap JSON calls `scrub()`; crates/octoscode-client/tests/repo_hermetic.rs
(no_tracked_snap_carries_a_secret_field_value) fails any tracked snap that still carries a secret-like field's value."""

SECRET_PARTS = ("token", "apikey", "api_key", "credential", "secret", "password", "passwd")


def secret_like(wid: str) -> bool:
    w = (wid or "").lower()
    if "key_env" in w:
        return False
    return any(s in w for s in SECRET_PARTS)


def scrub(tree):
    """Blank `val` on secret-like fields (in place); returns the tree."""
    def walk(x):
        if isinstance(x, dict):
            if secret_like(str(x.get("i") or "")) and isinstance(x.get("val"), str):
                x["val"] = ""
            for v in x.values():
                walk(v)
        elif isinstance(x, list):
            for v in x:
                walk(v)
    walk(tree)
    return tree
