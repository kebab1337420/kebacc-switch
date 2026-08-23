pub mod add;
pub mod arm;
pub mod auto;
pub mod countdown;
pub mod doctor;
pub mod install;
pub mod list;
pub mod midtask;
pub mod refresh;
pub mod remove;
pub mod statusline;
pub mod switch;
pub mod uninstall;
pub mod update;
pub mod watch;
pub mod wire;

use crate::live;
use crate::pool::Entry;
use crate::provider::{Provider, Wanted};

#[derive(Default)]
pub struct Options {
    pub wanted: Wanted,
    pub email: Option<String>,
    pub quiet: bool,
    pub hook: bool,
    pub refresh: bool,
    pub yes: bool,
    pub protect: bool,
    pub adopt: bool,
    pub rollback: bool,
    /// `doctor -Renew`: ask the token endpoint for a new pair for every saved
    /// login whose own has run out, and keep what comes back.
    pub renew: bool,
    pub clean: bool,
    pub countdown: bool,
    pub midtask: bool,
    pub merge: bool,
    pub drop: bool,
    pub check: bool,
    pub spawned: bool,
    /// Decide on the snapshots already on disk and never call the quota API.
    /// What the session-start hook runs on: the terminal shows nothing until
    /// that hook answers.
    pub offline: bool,
    pub statusline: Option<bool>,
    pub updates: Option<bool>,
    /// Where the binary lives, for `install` and `uninstall`. Unset means
    /// ~/.claude-tools, and the tests in ci.yml are what set it.
    pub tools_dir: Option<String>,
    /// The binary `install` puts in place, when it is not the one running.
    pub binary: Option<String>,
    pub auto_switch: bool,
    pub no_profile_edit: bool,
    /// `uninstall -Pool`: delete the saved logins too.
    pub pool: bool,
}

pub fn current<'a>(provider: &Provider, pool: &'a [Entry]) -> Option<&'a Entry> {
    let live = live::identity(provider)?;
    let email = crate::jsonio::str_of(&live, "emailAddress")?.to_lowercase();
    pool.iter().find(|e| e.email.to_lowercase() == email)
}

pub fn chosen_index(answer: &str, count: usize) -> Option<usize> {
    let index: usize = answer.trim().parse().ok()?;
    (index >= 1 && index <= count).then(|| index - 1)
}
