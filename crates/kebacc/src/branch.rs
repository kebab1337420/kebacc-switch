pub struct Branch {
    pub key: &'static str,
    pub aliases: &'static [&'static str],
    pub label: &'static str,
    pub cli: &'static str,
    pub flag: &'static str,
    pub seal_account: &'static str,
    pub home_env: &'static str,
    pub home_env_shared: bool,
    pub home_default: &'static [&'static str],
    pub home_suffix: &'static [&'static str],
    pub store_env: &'static str,
    pub store_default: &'static str,
    pub cred_file: &'static str,
    pub cred_label: &'static str,
    pub config_files: &'static [ConfigAt],
    pub keychain_service: Option<&'static str>,
    pub keychain_on_macos: bool,
    pub uses_keyring: bool,
    pub renew: bool,
    pub token: Token,
    pub identity: Identity,
    pub quota: Quota,
}

pub enum ConfigAt {
    Home(&'static str),
    Dir(&'static str),
}

pub enum Token {
    Paths(&'static [&'static [&'static str]]),
    Antigravity,
    None,
}

pub enum Identity {
    ConfigMember(&'static str),
    Codex,
    Antigravity,
    Derived {
        emails: &'static [&'static str],
        ids: &'static [&'static str],
        hash: Hash,
    },
}

#[derive(Clone, Copy)]
pub enum Hash {
    Whole,
    Fields(&'static [&'static str]),
    None,
}

pub enum Quota {
    Get {
        url: &'static str,
        headers: &'static [(&'static str, &'static str)],
        root: &'static [&'static str],
        five_hour: &'static str,
        seven_day: &'static str,
        not_for_prefix: Option<&'static str>,
    },
    Antigravity,
    None,
}

pub const BRANCHES: &[Branch] = &[
    Branch {
        key: "claude",
        aliases: &["claudecode", "cl", "cc", "anthropic"],
        label: "Claude Code",
        cli: "claude",
        flag: "-claude",
        seal_account: "kebacc-switch",
        home_env: "CLAUDE_CONFIG_DIR",
        home_env_shared: false,
        home_default: &[".claude"],
        home_suffix: &[],
        store_env: "KEBACC_SWITCH_ACCOUNTS",
        store_default: ".kebacc-switch-accounts",
        cred_file: ".credentials.json",
        cred_label: "~/.claude/.credentials.json",
        config_files: &[
            ConfigAt::Home(".claude.json"),
            ConfigAt::Dir(".claude.json"),
        ],
        keychain_service: Some("Claude Code-credentials"),
        keychain_on_macos: true,
        uses_keyring: false,
        renew: true,
        token: Token::Paths(&[&["claudeAiOauth", "accessToken"]]),
        identity: Identity::ConfigMember("oauthAccount"),
        quota: Quota::Get {
            url: "https://api.anthropic.com/api/oauth/usage",
            headers: &[
                ("anthropic-version", "2023-06-01"),
                ("anthropic-beta", "oauth-2025-04-20"),
            ],
            root: &[],
            five_hour: "five_hour",
            seven_day: "seven_day",
            not_for_prefix: None,
        },
    },
    Branch {
        key: "codex",
        aliases: &["cx", "openai", "chatgpt", "gpt"],
        label: "Codex",
        cli: "codex",
        flag: "-codex",
        seal_account: "kebacc-switch",
        home_env: "CODEX_HOME",
        home_env_shared: false,
        home_default: &[".codex"],
        home_suffix: &[],
        store_env: "KEBACC_SWITCH_CODEX_ACCOUNTS",
        store_default: ".kebacc-switch-codex-accounts",
        cred_file: "auth.json",
        cred_label: "~/.codex/auth.json",
        config_files: &[],
        keychain_service: None,
        keychain_on_macos: false,
        uses_keyring: false,
        renew: false,
        token: Token::Paths(&[&["tokens", "access_token"], &["OPENAI_API_KEY"]]),
        identity: Identity::Codex,
        quota: Quota::Get {
            url: "https://chatgpt.com/backend-api/codex/usage",
            headers: &[],
            root: &["rate_limits"],
            five_hour: "primary",
            seven_day: "secondary",
            not_for_prefix: Some("sk-"),
        },
    },
    Branch {
        key: "antigravity",
        aliases: &["ag", "agy", "google", "gemini"],
        label: "Antigravity",
        cli: "agy",
        flag: "-ag",
        seal_account: "kebacc-antigravity",
        home_env: "ANTIGRAVITY_HOME",
        home_env_shared: false,
        home_default: &[".gemini", "antigravity-cli"],
        home_suffix: &[],
        store_env: "KEBACC_SWITCH_ANTIGRAVITY_ACCOUNTS",
        store_default: ".kebacc-switch-antigravity-accounts",
        cred_file: "antigravity-oauth-token",
        cred_label: "~/.gemini/antigravity-cli/antigravity-oauth-token",
        config_files: &[],
        keychain_service: None,
        keychain_on_macos: false,
        uses_keyring: true,
        renew: false,
        token: Token::Antigravity,
        identity: Identity::Antigravity,
        quota: Quota::Antigravity,
    },
];

pub fn of(id: crate::provider::ProviderId) -> &'static Branch {
    &BRANCHES[id.index()]
}

pub fn find(key: &str) -> Option<usize> {
    BRANCHES
        .iter()
        .position(|branch| branch.key == key || branch.aliases.contains(&key))
}
