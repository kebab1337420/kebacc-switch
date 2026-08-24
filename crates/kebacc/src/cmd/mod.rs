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
pub mod use_dir;
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
    pub reserve: Option<bool>,
    pub on_switch: Option<String>,
    pub dir: Option<String>,
    pub five_hour: Option<f64>,
    pub seven_day: Option<f64>,
    pub given: Vec<String>,
}

pub fn current<'a>(provider: &Provider, pool: &'a [Entry]) -> Option<&'a Entry> {
    let live = live::identity(provider)?;
    if let Some(email) = crate::jsonio::str_of(&live, "emailAddress") {
        let email = email.to_lowercase();
        return pool.iter().find(|e| e.email.to_lowercase() == email);
    }
    let uuid = crate::jsonio::str_of(&live, "accountUuid")?;
    pool.iter().find(|e| {
        e.identity
            .as_ref()
            .and_then(|id| crate::jsonio::str_of(id, "accountUuid"))
            .is_some_and(|saved| saved == uuid)
    })
}

pub fn chosen_index(answer: &str, count: usize) -> Option<usize> {
    let index: usize = answer.trim().parse().ok()?;
    (index >= 1 && index <= count).then(|| index - 1)
}
