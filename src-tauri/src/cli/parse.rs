use std::{collections::BTreeMap, ffi::OsString};
#[derive(Clone)]
pub(super) enum Command {
    Help,
    Version,
    Doctor,
    Status,
    Import,
    Read(ReadCommand),
    Write(WriteCommand),
    Advanced(AdvancedCommand),
}
#[derive(Clone)]
pub(super) enum AdvancedCommand {
    Report(String),
    Trace,
    Compare,
    Query(String),
    Batch(bool),
    Changes,
    Jobs(String),
    Cache,
    Data(String),
    Preferences(bool),
}
impl AdvancedCommand {
    pub fn requires_execute(&self) -> bool {
        matches!(self, Self::Query(a) if a == "save" || a == "delete")
            || matches!(
                self,
                Self::Batch(true) | Self::Data(_) | Self::Preferences(true)
            )
            || matches!(self, Self::Jobs(a) if a == "start" || a == "cancel" || a == "cancel-waiter")
    }
}
#[derive(Clone)]
pub(super) enum ReadCommand {
    Tasks,
    Get(i64),
    Search(String),
    Ticket(i64),
    Children(i64),
    History(i64),
    Queue,
    Future,
    QueueHistory(i64),
    Calendar,
    Summary,
    ReportItems,
    Health,
    Diagnostic(String),
    ParentCandidates(i64),
    Receipt,
}
#[derive(Clone)]
pub(super) enum WriteCommand {
    Create,
    Update(i64),
    Status(i64, String),
    Complete(i64),
    Urgent(i64, bool),
    Archive(i64),
    Trash(i64),
    Restore(i64),
    Reopen(i64),
    Reschedule(i64),
    ChildCreate(i64),
    ParentSet(i64, Option<i64>),
    Reorder(i64),
    Enter(i64),
    Move(i64, String),
    Process(i64),
    CompleteRound(i64),
    Record(i64),
    Correct(i64),
    Void(i64),
    Undo,
}
pub(super) struct Options {
    pub command: Command,
    pub profile: Option<String>,
    pub input: Option<String>,
    pub flags: BTreeMap<String, String>,
    pub all: bool,
    pub execute: bool,
    pub package: Option<String>,
    pub text: bool,
}
pub(super) fn requests_json(args: &[OsString]) -> bool {
    args.windows(2)
        .any(|p| p[0] == "--format" && p[1] == "json")
}
pub(super) fn valid_profile(name: &str) -> bool {
    let bytes = name.as_bytes();
    if bytes.is_empty()
        || bytes.len() > 48
        || !bytes[0].is_ascii_lowercase()
        || !bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_-".contains(b))
    {
        return false;
    }
    !matches!(name, "con" | "prn" | "aux" | "nul")
        && !(name.len() == 4
            && (name.starts_with("com") || name.starts_with("lpt"))
            && bytes[3].is_ascii_digit())
}
// Fixed command grammar only. Writes require --execute; no arbitrary dispatch.
pub(super) fn parse(args: Vec<OsString>) -> Result<Options, ()> {
    if args.len() > 128 {
        return Err(());
    }
    let args: Vec<String> = args
        .into_iter()
        .map(|s| s.into_string().map_err(|_| ()))
        .collect::<Result<_, _>>()?;
    if args
        .iter()
        .any(|s| s.len() > 32768 || s.chars().any(char::is_control))
    {
        return Err(());
    }
    let mut words = Vec::new();
    let mut flags = BTreeMap::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg.starts_with("--") && !matches!(arg.as_str(), "--help" | "--version") {
            let boolean = matches!(
                arg.as_str(),
                "--no-color"
                    | "--all"
                    | "--include-trash"
                    | "--include-voided"
                    | "--execute"
                    | "--confirmed-real-work"
                    | "--explicit-user-request"
                    | "--allow-possible-duplicate"
                    | "--replace-whole-text"
                    | "--include-eligible-subtasks"
                    | "--parent-only"
                    | "--include-subtasks"
                    | "--include-completed-subtasks"
                    | "--inherit-deadline"
                    | "--confirm-historical-impact"
                    | "--sync-status"
                    | "--explicit-intent"
                    | "--explicit-requery"
                    | "--allow-latest-after-invalidation"
            );
            let value = if boolean {
                "true".into()
            } else {
                let value = iter.next().ok_or(())?;
                if value.is_empty() || value.starts_with("--") {
                    return Err(());
                }
                value.clone()
            };
            if flags.insert(arg.clone(), value).is_some() {
                return Err(());
            }
        } else {
            words.push(arg.as_str());
        }
    }
    let command = match words.as_slice() {
        [] | ["--help"] => Command::Help,
        ["--version"] => Command::Version,
        ["doctor"] => Command::Doctor,
        ["status"] | ["capabilities"] | ["client", "status"] => Command::Status,
        ["client", "import"] => Command::Import,
        ["task", "list"] => Command::Read(ReadCommand::Tasks),
        ["task", "get", id] => Command::Read(ReadCommand::Get(positive_id(id)?)),
        ["task", "search", text] if !text.is_empty() => {
            Command::Read(ReadCommand::Search((*text).into()))
        }
        ["task", "ticket", id] => Command::Read(ReadCommand::Ticket(positive_id(id)?)),
        ["task", "children", id] => Command::Read(ReadCommand::Children(positive_id(id)?)),
        ["task", "history", id] => Command::Read(ReadCommand::History(positive_id(id)?)),
        ["queue", "list"] => Command::Read(ReadCommand::Queue),
        ["queue", "future"] => Command::Read(ReadCommand::Future),
        ["queue", "history", id] => Command::Read(ReadCommand::QueueHistory(positive_id(id)?)),
        ["calendar", "list"] => Command::Read(ReadCommand::Calendar),
        ["stats", "summary"] => Command::Read(ReadCommand::Summary),
        ["report", period @ ("daily" | "weekly" | "monthly")] => {
            Command::Advanced(AdvancedCommand::Report((*period).into()))
        }
        ["stats", "trace"] => Command::Advanced(AdvancedCommand::Trace),
        ["stats", "compare"] => Command::Advanced(AdvancedCommand::Compare),
        ["query", action @ ("list" | "get" | "save" | "delete")] => {
            Command::Advanced(AdvancedCommand::Query((*action).into()))
        }
        ["batch", "preview"] => Command::Advanced(AdvancedCommand::Batch(false)),
        ["batch", "execute"] => Command::Advanced(AdvancedCommand::Batch(true)),
        ["changes", "list"] => Command::Advanced(AdvancedCommand::Changes),
        ["jobs", action @ ("start" | "list" | "get" | "result" | "cancel-waiter" | "cancel")] => {
            Command::Advanced(AdvancedCommand::Jobs((*action).into()))
        }
        ["data", "cache"] => Command::Advanced(AdvancedCommand::Cache),
        ["data", action @ ("backup" | "export")] => {
            Command::Advanced(AdvancedCommand::Data((*action).into()))
        }
        ["preferences", "get"] => Command::Advanced(AdvancedCommand::Preferences(false)),
        ["preferences", "patch"] => Command::Advanced(AdvancedCommand::Preferences(true)),
        ["report", "items"] => Command::Read(ReadCommand::ReportItems),
        ["data", "health"] => Command::Read(ReadCommand::Health),
        ["diagnostic", "get", id] => Command::Read(ReadCommand::Diagnostic((*id).into())),
        ["task", "parent", "candidates", id] => {
            Command::Read(ReadCommand::ParentCandidates(positive_id(id)?))
        }
        ["receipt", "get"] => Command::Read(ReadCommand::Receipt),
        ["task", "create"] => Command::Write(WriteCommand::Create),
        ["task", "update", id] => Command::Write(WriteCommand::Update(positive_id(id)?)),
        ["task", "status", "set", id, status] => {
            Command::Write(WriteCommand::Status(positive_id(id)?, (*status).into()))
        }
        ["task", "complete", id] => Command::Write(WriteCommand::Complete(positive_id(id)?)),
        ["task", "urgent", "set", id] => {
            Command::Write(WriteCommand::Urgent(positive_id(id)?, true))
        }
        ["task", "urgent", "clear", id] => {
            Command::Write(WriteCommand::Urgent(positive_id(id)?, false))
        }
        ["task", "archive", id] => Command::Write(WriteCommand::Archive(positive_id(id)?)),
        ["task", "trash", id] => Command::Write(WriteCommand::Trash(positive_id(id)?)),
        ["task", "restore", id] => Command::Write(WriteCommand::Restore(positive_id(id)?)),
        ["task", "reopen", id] => Command::Write(WriteCommand::Reopen(positive_id(id)?)),
        ["task", "reschedule", id] => Command::Write(WriteCommand::Reschedule(positive_id(id)?)),
        ["task", "child", "create", id] => {
            Command::Write(WriteCommand::ChildCreate(positive_id(id)?))
        }
        ["task", "parent", "set", id, parent] => Command::Write(WriteCommand::ParentSet(
            positive_id(id)?,
            Some(positive_id(parent)?),
        )),
        ["task", "parent", "clear", id] => {
            Command::Write(WriteCommand::ParentSet(positive_id(id)?, None))
        }
        ["task", "children", "reorder", id] => {
            Command::Write(WriteCommand::Reorder(positive_id(id)?))
        }
        ["queue", "enter", id] => Command::Write(WriteCommand::Enter(positive_id(id)?)),
        ["queue", "move", id, direction] if matches!(*direction, "up" | "down") => {
            Command::Write(WriteCommand::Move(positive_id(id)?, (*direction).into()))
        }
        ["work", "process-round", id] => Command::Write(WriteCommand::Process(positive_id(id)?)),
        ["work", "complete-round", id] => {
            Command::Write(WriteCommand::CompleteRound(positive_id(id)?))
        }
        ["work", "record", id] => Command::Write(WriteCommand::Record(positive_id(id)?)),
        ["work", "correct", id] => Command::Write(WriteCommand::Correct(positive_id(id)?)),
        ["work", "void", id] => Command::Write(WriteCommand::Void(positive_id(id)?)),
        ["undo", "request"] => Command::Write(WriteCommand::Undo),
        _ => return Err(()),
    };
    let format = flags.remove("--format");
    if format
        .as_deref()
        .is_some_and(|f| !matches!(f, "table" | "json"))
    {
        return Err(());
    }
    let text = match flags.remove("--render").as_deref() {
        None => false,
        Some("text")
            if format.as_deref() != Some("json")
                && matches!(&command, Command::Advanced(AdvancedCommand::Report(p)) if p == "daily") =>
        {
            true
        }
        _ => return Err(()),
    };
    flags.remove("--no-color");
    let profile = flags.remove("--profile");
    if profile.as_deref().is_some_and(|p| !valid_profile(p)) {
        return Err(());
    }
    let input = flags.remove("--input");
    if input
        .as_deref()
        .is_some_and(|p| p.starts_with('-') && p != "-")
    {
        return Err(());
    }
    let all = flags.remove("--all").is_some();
    let execute = flags.remove("--execute").is_some();
    let requires_execute = matches!(command, Command::Write(_))
        || matches!(&command, Command::Advanced(c) if c.requires_execute());
    if requires_execute != execute || (execute && all) {
        return Err(());
    }
    let package = flags.remove("--package");
    if matches!(command, Command::Help | Command::Version) && profile.is_some() {
        return Err(());
    }
    if !matches!(
        command,
        Command::Status | Command::Read(_) | Command::Write(_) | Command::Advanced(_)
    ) && input.is_some()
    {
        return Err(());
    }
    if matches!(command, Command::Import) {
        if package.as_deref().is_none_or(|p| {
            !(p.len() == 64
                && p.bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()))
        }) || profile.is_some()
        {
            return Err(());
        }
    } else if package.is_some() {
        return Err(());
    }
    if !matches!(
        command,
        Command::Read(_) | Command::Write(_) | Command::Advanced(_)
    ) && (!flags.is_empty() || all)
    {
        return Err(());
    }
    Ok(Options {
        command,
        profile,
        input,
        flags,
        all,
        execute,
        package,
        text,
    })
}
fn positive_id(s: &str) -> Result<i64, ()> {
    let id = s.parse::<i64>().map_err(|_| ())?;
    if id <= 0 || id.to_string() != s {
        Err(())
    } else {
        Ok(id)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn check(s: &[&str]) -> bool {
        parse(s.iter().map(OsString::from).collect()).is_ok()
    }
    #[test]
    fn strict_command_flag_values_duplicates_and_profile_paths() {
        for args in [
            vec![],
            vec!["--version", "--format", "json"],
            vec![
                "--format",
                "json",
                "status",
                "--profile",
                "codex-work",
                "--input",
                "中文 路径.json",
                "--no-color",
            ],
        ] {
            assert!(check(&args));
        }
        for args in [
            vec!["task", "create"],
            vec!["--token", "hidden"],
            vec!["status", "--profile", "../other"],
            vec!["status", "--profile", "CON"],
            vec!["status", "--profile", "com1"],
            vec!["status", "--profile", "other.dpapi"],
            vec!["--format", "ndjson"],
            vec!["status", "--format", "json", "--format", "table"],
            vec!["doctor", "--input", "-"],
            vec!["doctor", "status"],
            vec!["--profile"],
            vec!["--no-color", "--no-color"],
            vec!["status", "--input", "--token"],
            vec!["--version", "--profile", "hidden"],
        ] {
            assert!(!check(&args), "{args:?}");
        }
    }
}
