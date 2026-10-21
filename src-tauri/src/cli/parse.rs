use std::ffi::OsString;

#[derive(Clone, Copy)]
pub(super) enum Command {
    Help,
    Version,
    Doctor,
    Status,
}
pub(super) struct Options {
    pub command: Command,
    pub profile: Option<String>,
    pub input: Option<String>,
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
// Phase2 has four commands and five options: explicit matching avoids a new
// dependency and rejects unknown/duplicate options without echoing their values.
pub(super) fn parse(args: Vec<OsString>) -> Result<Options, ()> {
    if args.len() > 32 {
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
    let mut command = None;
    let mut profile = None;
    let mut input = None;
    let mut format = false;
    let mut no_color = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--help" | "--version" | "doctor" | "status" if command.is_none() => {
                command = Some(match arg.as_str() {
                    "--version" => Command::Version,
                    "doctor" => Command::Doctor,
                    "status" => Command::Status,
                    _ => Command::Help,
                });
            }
            "--format" if !format => {
                if !matches!(iter.next().map(String::as_str), Some("table" | "json")) {
                    return Err(());
                }
                format = true;
            }
            "--no-color" if !no_color => no_color = true,
            "--profile" if profile.is_none() => {
                let name = iter.next().ok_or(())?;
                if !valid_profile(name) {
                    return Err(());
                }
                profile = Some(name.clone());
            }
            "--input" if input.is_none() => {
                let path = iter.next().ok_or(())?;
                if path.is_empty() || (path.starts_with('-') && path != "-") {
                    return Err(());
                }
                input = Some(path.clone());
            }
            _ => return Err(()),
        }
    }
    let command = command.unwrap_or(Command::Help);
    if input.is_some() && !matches!(command, Command::Status) {
        return Err(());
    }
    if profile.is_some() && matches!(command, Command::Help | Command::Version) {
        return Err(());
    }
    Ok(Options {
        command,
        profile,
        input,
    })
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
