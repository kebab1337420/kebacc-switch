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
    let entry = match (entries.len(), opts.email.as_deref()) {
        (1, None) => Some(&entries[0]),
        _ => super::switch::pick(&entries, current, opts.email.as_deref()),
    };
    let Some(entry) = entry else {
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

    let branch = provider.id.branch();
    let mut inner = dir.clone();
    for part in branch.home_suffix {
        inner.push(part);
    }
    if inner != dir {
        if let Err(problem) = std::fs::create_dir_all(&inner) {
            say(
                &format!("Could not make {}: {problem}", inner.display()),
                Color::Red,
            );
            return 1;
        }
        provider::reprotect_dir(&inner);
    }
    let config_candidates = branch
        .config_files
        .iter()
        .map(|at| match at {
            crate::branch::ConfigAt::Home(name) | crate::branch::ConfigAt::Dir(name) => {
                inner.join(name)
            }
        })
        .collect();
    let target = Provider {
        cred_candidates: vec![inner.join(branch.cred_file)],
        config_candidates,
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
    for line in export_lines(branch.home_env, &dir) {
        println!("  {line}");
    }
    0
}

fn export_lines(variable: &str, dir: &std::path::Path) -> Vec<String> {
    let dir = dir.display();
    if cfg!(windows) {
        vec![
            format!("$env:{variable} = '{dir}'"),
            format!("set {variable}={dir}"),
        ]
    } else {
        vec![format!("export {variable}='{dir}'")]
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
    use super::{export_lines, slug};

    #[test]
    fn the_shell_line_names_the_variable_the_cli_reads() {
        let lines = export_lines("CODEX_HOME", std::path::Path::new("/tmp/one"));
        assert!(lines.iter().all(|line| line.contains("CODEX_HOME")));
    }

    #[test]
    fn an_address_becomes_a_directory_name() {
        assert_eq!(slug("A.User+tag@Example.com"), "a.user_tag_example.com");
    }
}
