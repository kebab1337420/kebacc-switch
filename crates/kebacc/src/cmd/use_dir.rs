use super::Options;
use crate::pool::Pool;
use crate::provider::{self, Provider};
use crate::term::{say, Color};
use std::path::PathBuf;

pub fn run(provider: &Provider, opts: &Options) -> i32 {
    let entries = Pool::new(provider).entries();
    if entries.is_empty() {
        say(
            &format!("No {} account saved yet.", provider.label),
            Color::Red,
        );
        return 1;
    }
    let current = super::current(provider, &entries);
    let Some(entry) = super::switch::pick(&entries, current, opts.email.as_deref()) else {
        return 1;
    };
    let Some(creds) = entry.creds.as_deref() else {
        say(
            &format!(
                "The credentials for {} could not be read back.",
                entry.email
            ),
            Color::Red,
        );
        return 1;
    };

    let dir = match opts.dir.as_deref() {
        Some(given) => PathBuf::from(given),
        None => provider::home()
            .join(".kebacc-sessions")
            .join(provider.cli)
            .join(slug(&entry.email)),
    };
    if let Err(problem) = std::fs::create_dir_all(&dir) {
        say(
            &format!("Could not make {}: {problem}", dir.display()),
            Color::Red,
        );
        return 1;
    }
    provider::reprotect_dir(&dir);

    let target = Provider {
        cred_candidates: vec![dir.join(cred_name(provider))],
        config_candidates: vec![dir.join(".claude.json")],
        uses_keychain: false,
        keychain_service: None,
        ..provider::spec(provider.id)
    };
    if let Err(problem) = crate::live::set_creds_raw(&target, creds) {
        say(
            &format!("Could not write the credentials: {problem}"),
            Color::Red,
        );
        return 1;
    }
    if let Some(identity) = entry.identity.as_ref() {
        crate::live::set_identity(&target, identity);
    }

    if provider.uses_keychain {
        say(
            "This machine keeps the login in the keychain, which every session shares. A session directory cannot hold an account of its own here.",
            Color::Yellow,
        );
    }

    say(
        &format!("{} is set up in {}", entry.email, dir.display()),
        Color::Green,
    );
    say("Start the CLI from a shell that carries this:", Color::Dim);
    for line in export_lines(&dir) {
        println!("  {line}");
    }
    0
}

fn cred_name(provider: &Provider) -> String {
    provider
        .cred_candidates
        .first()
        .and_then(|path| path.file_name())
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| ".credentials.json".to_string())
}

fn export_lines(dir: &std::path::Path) -> Vec<String> {
    let dir = dir.display();
    if cfg!(windows) {
        vec![
            format!("$env:CLAUDE_CONFIG_DIR = '{dir}'"),
            format!("set CLAUDE_CONFIG_DIR={dir}"),
        ]
    } else {
        vec![format!("export CLAUDE_CONFIG_DIR='{dir}'")]
    }
}

fn slug(email: &str) -> String {
    email
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-') {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::slug;

    #[test]
    fn an_address_becomes_a_directory_name() {
        assert_eq!(slug("A.User+tag@Example.com"), "a.user_tag_example.com");
    }
}
