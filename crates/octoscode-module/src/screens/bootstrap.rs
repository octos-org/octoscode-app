//! A21 — failing-first stub: main's behaviour behind the bootstrap API (the
//! transport always dials OCTOS_BASE_URL, else the built-in default).

/// What a launch does on its own.
#[derive(Clone, PartialEq, Eq)]
pub enum AutoStart {
    Restore { server: String, token: String },
    Harness { server: String, token: String },
    None,
}

impl std::fmt::Debug for AutoStart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AutoStart::Restore { server, .. } => write!(f, "Restore({server})"),
            AutoStart::Harness { server, .. } => write!(f, "Harness({server})"),
            AutoStart::None => write!(f, "None"),
        }
    }
}

#[derive(Clone, Default, PartialEq, Eq)]
pub struct Inputs {
    pub pairing_link: bool,
    pub env_base: Option<String>,
    pub env_bearer: Option<String>,
    pub env_profile: bool,
    pub remembered: Option<String>,
    pub token: Option<String>,
    pub restore_target: bool,
    pub auto_connect: Option<bool>,
}

pub fn decide(i: &Inputs) -> AutoStart {
    AutoStart::Harness {
        server: i.env_base.clone().unwrap_or_else(|| "http://127.0.0.1:50082".into()),
        token: i.env_bearer.clone().unwrap_or_default(),
    }
}

impl Inputs {
    pub fn current() -> Inputs {
        Inputs {
            env_base: std::env::var("OCTOS_BASE_URL").ok(),
            env_bearer: std::env::var("OCTOS_BEARER").ok(),
            ..Default::default()
        }
    }
}

pub struct Boot {
    pub server: String,
    pub token: Option<String>,
    pub auto: AutoStart,
}

pub fn default_endpoint() -> String {
    std::env::var("OCTOS_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:50082".into())
}

pub fn boot() -> Boot {
    let (server, token) = crate::credentials::prefill();
    Boot { server: server.unwrap_or_else(default_endpoint), token, auto: decide(&Inputs::current()) }
}

pub fn adopt_identity(_server: &str, _token: &str) {}
pub fn note_authenticated(_server: &str) {}
pub fn note_disconnect() {}
pub fn forget_tab(_server: &str) {}
