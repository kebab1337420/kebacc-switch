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
pub mod set;
pub mod status;
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
    pub renew: bool,
    pub clean: bool,
    pub countdown: bool,
    pub midtask: bool,
    pub merge: bool,
    pub drop: bool,
    pub check: bool,
    pub spawned: bool,
    pub offline: bool,
    pub statusline: Option<bool>,
    pub updates: Option<bool>,
    pub tools_dir: Option<String>,
    pub binary: Option<String>,
    pub auto_switch: bool,
    pub no_profile_edit: bool,
    pub pool: bool,
    pub json: bool,
    pub fix: bool,
    pub rank: Option<i64>,
    pub five_hour: Option<f64>,
    pub seven_day: Option<f64>,
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
