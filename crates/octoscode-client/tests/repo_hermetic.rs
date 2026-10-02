//! Repo-wide guard (#31c): the public repo carries no machine paths. This is
//! the fixtures_hermetic.rs rule over EVERY tracked text file — a lane that
//! leaks a macOS home path, a var-folders temp path or a real /home/<user>/
//! fails the build instead of collecting a review note. The banned patterns
//! are assembled from fragments so this file itself stays free of the
//! literals it bans — the entry's user-path sweep must show nothing new
//! from this change, this guard included.
use std::fs;
use std::process::Command;

fn repo_root() -> std::path::PathBuf {
    // crates/octoscode-client -> repo root is TWO levels up.
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

fn tracked_files() -> Vec<std::path::PathBuf> {
    // The ctest slot can run with a minimal PATH; git lives in the system
    // dirs, so augment it instead of assuming the caller's environment.
    let path = std::env::var("PATH").unwrap_or_default();
    let augmented = format!(
        "{}:/usr/bin:/bin:/usr/local/bin:/opt/homebrew/bin",
        if path.is_empty() { String::from("/usr/bin:/bin") } else { path }
    );
    let out = Command::new("git")
        .args(["ls-files", "-z"])
        .env("PATH", augmented)
        .current_dir(repo_root())
        .output()
        .expect("git ls-files (run inside a checkout)");
    assert!(
        out.status.success(),
        "git ls-files failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout)
        .split('\0')
        .filter(|s| !s.is_empty())
        .map(|s| repo_root().join(s))
        .collect()
}

fn is_text(bytes: &[u8]) -> bool {
    !bytes.get(..4096).unwrap_or(bytes).contains(&0)
}

#[test]
fn no_machine_paths_or_secrets_anywhere_tracked() {
    // Assembled: the literals never appear in this source file.
    let users = format!("/{}s/", "User");
    let var_folders = format!("/var/{}/", "folders");
    // Product mock data keeps these home names: `~/home/octos/…` is the web's
    // own mock copy, /home/user is f2_profile's fixture mock, /home/profiles
    // and /home/apps ride recorded traffic, runner is the CI home the older
    // fixture guard already names.
    let mock_homes = ["octos", "user", "runner", "profiles", "apps"];
    // Files whose OWN patterns are the enforcement (the older guards) — their
    // literals are the point, not a leak. This file joins them.
    let guard_files = [
        "c24_replay.rs", "c24b_replay.rs", "capture23.rs", "capture23b.rs",
        "capture23c.rs", "fixtures_hermetic.rs", "r23_replay.rs",
        "r2_replay.rs", "r3_replay.rs", "r5_replay.rs",
        "record_autonomy.rs", "repo_hermetic.rs",
    ];

    let mut checked = 0usize;
    let mut violations: Vec<String> = Vec::new();
    for path in tracked_files() {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let rel = path
            .strip_prefix(repo_root())
            .unwrap()
            .to_string_lossy()
            .to_string();
        let Ok(bytes) = fs::read(&path) else {
            continue;
        };
        if !is_text(&bytes) {
            continue;
        }
        checked += 1;
        let text = String::from_utf8_lossy(&bytes).to_string();
        let is_guard = guard_files.contains(&name.as_str());
        if !is_guard {
            if text.contains(&users) {
                violations.push(format!("{rel}: literal {}", users));
            }
            if text.contains(&var_folders) {
                violations.push(format!("{rel}: literal {}", var_folders));
            }
        }
        for (i, line) in text.lines().enumerate() {
            for (j, _) in line.match_indices("/home/") {
                let tilde_form = j > 0 && line[..j].ends_with('~');
                let rest = &line[j + "/home/".len()..];
                let seg: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '_' || *c == '-')
                    .collect();
                if seg.is_empty() || mock_homes.contains(&seg.as_str()) {
                    continue;
                }
                if tilde_form {
                    violations.push(format!(
                        "{rel}:{}: real user's home in a display string: ~/home/{seg}",
                        i + 1
                    ));
                } else {
                    violations.push(format!("{rel}:{}: /home/{seg}/", i + 1));
                }
            }
            for w in line.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_')) {
                if w.starts_with("sk-") && w.len() >= 20 {
                    violations.push(format!("{rel}:{}: key-shaped token", i + 1));
                }
                // D10c: the makepad bridge's per-launch token, `mprt_` + 64 hex.
                if w.len() >= 69 && w.starts_with("mprt_") && w[5..69].bytes().all(|b| b.is_ascii_hexdigit()) {
                    violations.push(format!("{rel}:{}: makepad bridge token", i + 1));
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "machine paths/secrets in tracked files:\n{}",
        violations.join("\n")
    );
    assert!(checked > 500, "tracked-file scan suspiciously small: {checked}");
}

/// The dev instrument's `/snap` reports a TextInput's raw buffer as `val`, the
/// masked ones included (a typed token or provider key reads in clear). No
/// tracked snap may carry a value in a secret-like field, so a capture taken
/// after a secret was typed fails here instead of reaching the public repo.
#[test]
fn no_tracked_snap_carries_a_secret_field_value() {
    fn secret_like(id: &str) -> bool {
        let id = id.to_ascii_lowercase();
        if id.contains("api_key_env") || id.contains("key_env") {
            return false; // the NAME of an env var, not its value
        }
        ["token", "apikey", "api_key", "credential", "secret", "password", "passwd"]
            .iter()
            .any(|s| id.contains(s))
    }
    fn walk(v: &serde_json::Value, rel: &str, bad: &mut Vec<String>) {
        match v {
            serde_json::Value::Object(m) => {
                let id = m.get("i").and_then(|x| x.as_str()).unwrap_or("");
                if secret_like(id) {
                    let val = m.get("val").and_then(|x| x.as_str()).unwrap_or("").trim();
                    if !val.is_empty() {
                        bad.push(format!("{rel}: `{id}` carries a typed value ({} chars)", val.chars().count()));
                    }
                }
                for x in m.values() {
                    walk(x, rel, bad);
                }
            }
            serde_json::Value::Array(a) => a.iter().for_each(|x| walk(x, rel, bad)),
            _ => {}
        }
    }
    let mut bad = Vec::new();
    for path in tracked_files() {
        if !path.to_string_lossy().ends_with(".snap.json") {
            continue;
        }
        let rel = path.strip_prefix(repo_root()).unwrap().to_string_lossy().to_string();
        let Ok(text) = fs::read_to_string(&path) else { continue };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
        walk(&v, &rel, &mut bad);
    }
    assert!(bad.is_empty(), "secret-like snap fields with typed values:\n{}", bad.join("\n"));
}
