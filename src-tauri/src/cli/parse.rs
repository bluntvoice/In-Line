use std::{collections::BTreeMap, ffi::OsString};
#[derive(Clone)]
pub(super) enum Command {
    Help,
    Version,
    Doctor,
    Status,
    Import,
    Read(ReadCommand),
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
}
pub(super) struct Options {
    pub command: Command,
    pub profile: Option<String>,
    pub input: Option<String>,
    pub flags: BTreeMap<String, String>,
    pub all: bool,
    pub package: Option<String>,
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
// Every command is explicitly read-only; no generic tool dispatch.
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
                "--no-color" | "--all" | "--include-trash" | "--include-voided"
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
        ["stats", "summary"] | ["report", "daily" | "weekly" | "monthly"] => {
            Command::Read(ReadCommand::Summary)
        }
        ["report", "items"] => Command::Read(ReadCommand::ReportItems),
        ["data", "health"] => Command::Read(ReadCommand::Health),
        ["diagnostic", "get", id] => Command::Read(ReadCommand::Diagnostic((*id).into())),
        _ => return Err(()),
    };
    if flags
        .remove("--format")
        .is_some_and(|f| !matches!(f.as_str(), "table" | "json"))
    {
        return Err(());
    }
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
    let package = flags.remove("--package");
    if matches!(command, Command::Help | Command::Version) && profile.is_some() {
        return Err(());
    }
    if !matches!(command, Command::Status | Command::Read(_)) && input.is_some() {
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
    if !matches!(command, Command::Read(_)) && (!flags.is_empty() || all) {
        return Err(());
    }
    Ok(Options {
        command,
        profile,
        input,
        flags,
        all,
        package,
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
