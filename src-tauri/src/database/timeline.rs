//! A synchronous domain action owns its log scope. Nested effects inherit it
//! on the same connection, and each original task gets its own source identity.
use super::*;
use std::cell::RefCell;

const LEGACY_PREFIX: &str = "timeline:v1:";
const OPERATION_PREFIX: &str = "timeline:v2:";

struct Scope {
    connection: *const Connection,
    nonce: Option<String>,
}

thread_local! {
    static SCOPES: RefCell<Vec<Scope>> = const { RefCell::new(Vec::new()) };
}

pub(super) struct Operation<'a> {
    connection: &'a Connection,
    owns_scope: bool,
}

pub(super) fn begin(connection: &Connection) -> Operation<'_> {
    let owns_scope = SCOPES.with_borrow_mut(|scopes| {
        if scopes
            .iter()
            .any(|scope| std::ptr::eq(scope.connection, connection))
        {
            return false;
        }
        scopes.push(Scope {
            connection,
            nonce: None,
        });
        true
    });
    Operation {
        connection,
        owns_scope,
    }
}

impl Drop for Operation<'_> {
    fn drop(&mut self) {
        if self.owns_scope {
            SCOPES.with_borrow_mut(|scopes| {
                scopes.retain(|scope| !std::ptr::eq(scope.connection, self.connection));
            });
        }
    }
}

pub(super) fn automatic(kind: &str) -> bool {
    matches!(
        kind,
        "created"
            | "updated"
            | "queue"
            | "status"
            | "urgent"
            | "work"
            | "archived"
            | "deleted"
            | "merged"
            | "relation"
            | "restored"
            | "import_conflict"
            | "scheduled_created"
            | "scheduled_reactivated"
            | "schedule_changed"
            | "scheduled_activated"
            | "scheduled_late"
            | "scheduled_early"
            | "scheduled_stopped"
            | "queue_number_voided"
            | "backup_number_remapped"
    )
}

pub(super) fn operation_id(
    connection: &Connection,
    task_id: i64,
    kind: &str,
) -> Result<Option<String>, String> {
    if !automatic(kind) {
        return Ok(None);
    }
    let _operation = begin(connection);
    let existing = SCOPES.with_borrow(|scopes| {
        scopes
            .iter()
            .find(|scope| std::ptr::eq(scope.connection, connection))
            .and_then(|scope| scope.nonce.clone())
    });
    let nonce = match existing {
        Some(nonce) => nonce,
        None => {
            let nonce: String = connection
                .query_row("SELECT lower(hex(randomblob(16)))", [], |row| row.get(0))
                .map_err(display_error)?;
            SCOPES.with_borrow_mut(|scopes| {
                if let Some(scope) = scopes
                    .iter_mut()
                    .find(|scope| std::ptr::eq(scope.connection, connection))
                {
                    scope.nonce = Some(nonce.clone());
                }
            });
            nonce
        }
    };
    Ok(Some(format!("{OPERATION_PREFIX}{nonce}:{task_id}")))
}

fn valid_identity(value: &str) -> bool {
    let Some((nonce, id)) = value.split_once(':') else {
        return false;
    };
    nonce.len() == 32
        && nonce.bytes().all(|byte| byte.is_ascii_hexdigit())
        && !id.is_empty()
        && id.bytes().all(|byte| byte.is_ascii_digit())
        && !id.starts_with('0')
        && id.parse::<i64>().is_ok()
}

pub(super) fn legacy_type(raw: &str) -> Option<(&str, &str)> {
    let tagged = raw.strip_prefix(LEGACY_PREFIX)?;
    let (identity, kind) = tagged.rsplit_once(':')?;
    (automatic(kind) && valid_identity(identity)).then_some((kind, identity))
}

pub(super) fn visible_log<'a>(raw: &'a str, stored: Option<&str>) -> (&'a str, Option<String>) {
    let legacy = legacy_type(raw);
    let kind = legacy.map_or(raw, |(kind, _)| kind);
    if automatic(kind) {
        if let Some(value) = stored.filter(|value| {
            value
                .strip_prefix(OPERATION_PREFIX)
                .is_some_and(valid_identity)
        }) {
            return (kind, Some(value.to_ascii_lowercase()));
        }
        if let Some((_, identity)) = legacy {
            return (
                kind,
                Some(format!("{LEGACY_PREFIX}{}", identity.to_ascii_lowercase())),
            );
        }
    }
    (kind, None)
}

pub(super) fn mcp_type(raw: &str) -> (&str, bool) {
    if let Some((kind, _)) = legacy_type(raw) {
        return (kind, kind.contains("relation") || kind.contains("merge"));
    }
    if raw
        .split_once(':')
        .is_some_and(|(namespace, _)| namespace.eq_ignore_ascii_case("timeline"))
    {
        return ("unknown", true);
    }
    (raw, raw.contains("relation") || raw.contains("merge"))
}

type LogKey = (String, String, String, String);

struct StoredLog {
    id: i64,
    raw_type: String,
    content: String,
    at: String,
    operation: Option<String>,
}

impl StoredLog {
    fn key(&self) -> Option<LogKey> {
        let (kind, operation) = visible_log(&self.raw_type, self.operation.as_deref());
        Some((
            kind.to_owned(),
            operation?,
            self.content.clone(),
            self.at.clone(),
        ))
    }
}

fn stored_logs(connection: &Connection, task_id: i64) -> Result<Vec<StoredLog>, String> {
    let mut query = connection.prepare(
        "SELECT id,log_type,content,created_at,operation_id FROM task_logs WHERE task_id=? ORDER BY id"
    ).map_err(display_error)?;
    let rows = query
        .query_map([task_id], |r| {
            Ok(StoredLog {
                id: r.get(0)?,
                raw_type: r.get(1)?,
                content: r.get(2)?,
                at: r.get(3)?,
                operation: r.get(4)?,
            })
        })
        .map_err(display_error)?;
    rows.collect::<Result<_, _>>().map_err(display_error)
}

#[derive(Default, PartialEq)]
struct ExistingLogs {
    associated: HashMap<LogKey, usize>,
    legacy: HashSet<(String, String, String)>,
}

impl ExistingLogs {
    fn load(connection: &Connection, task_id: i64) -> Result<Self, String> {
        let mut existing = Self::default();
        for log in stored_logs(connection, task_id)? {
            existing.insert(log);
        }
        Ok(existing)
    }

    fn insert(&mut self, log: StoredLog) {
        if let Some(key) = log.key() {
            *self.associated.entry(key).or_default() += 1;
        } else {
            self.legacy.insert((log.raw_type, log.content, log.at));
        }
    }

    fn matches(&mut self, log: &StoredLog, importing: bool) -> bool {
        if let Some(key) = log.key() {
            if let Some(remaining) = self.associated.get_mut(&key).filter(|count| **count > 0) {
                *remaining -= 1;
                return true;
            }
            return false;
        }
        let key = (log.raw_type.clone(), log.content.clone(), log.at.clone());
        if importing {
            !self.legacy.insert(key)
        } else {
            self.legacy.contains(&key)
        }
    }
}

pub(super) fn same_restore_history(
    source: &Connection,
    target: &Connection,
    source_id: i64,
    target_id: i64,
    remap_note: Option<&str>,
) -> Result<bool, String> {
    let source = ExistingLogs::load(source, source_id)?;
    let mut target_history = ExistingLogs::default();
    let mut unmatched_remap = false;
    for log in stored_logs(target, target_id)? {
        let (kind, operation) = visible_log(&log.raw_type, log.operation.as_deref());
        let local_source = match operation.as_deref() {
            Some(value) => value
                .rsplit_once(':')
                .is_some_and(|(_, id)| id.parse::<i64>().ok() == Some(target_id)),
            None => log.raw_type == kind && log.operation.is_none(),
        };
        let local_remap = kind == "backup_number_remapped"
            && local_source
            && remap_note == Some(log.content.as_str())
            && !log.key().map_or_else(
                || {
                    source.legacy.contains(&(
                        log.raw_type.clone(),
                        log.content.clone(),
                        log.at.clone(),
                    ))
                },
                |key| source.associated.contains_key(&key),
            );
        if local_remap {
            if unmatched_remap {
                return Ok(false);
            }
            unmatched_remap = true;
        } else {
            target_history.insert(log);
        }
    }
    Ok(source == target_history && unmatched_remap == remap_note.is_some())
}

pub(super) fn has_reviewed_conflict(connection: &Connection, task_id: i64) -> Result<bool, String> {
    let mut query = connection.prepare_cached(
        "SELECT log_type FROM task_logs WHERE task_id=? AND content='已人工复核并解除导入冲突标识'"
    ).map_err(display_error)?;
    let rows = query
        .query_map([task_id], |row| row.get::<_, String>(0))
        .map_err(display_error)?;
    for raw in rows {
        let raw = raw.map_err(display_error)?;
        if raw == "import_conflict"
            || legacy_type(&raw).is_some_and(|(kind, _)| kind == "import_conflict")
        {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) fn deduplicate_for_merge(
    connection: &Connection,
    source_id: i64,
    target_id: i64,
) -> Result<(), String> {
    let mut existing = ExistingLogs::load(connection, target_id)?;
    for log in stored_logs(connection, source_id)? {
        if existing.matches(&log, false) {
            connection
                .execute("DELETE FROM task_logs WHERE id=?", [log.id])
                .map_err(display_error)?;
        }
    }
    Ok(())
}

pub(super) fn copy_for_restore(
    source: &Connection,
    target: &Connection,
    source_id: i64,
    target_id: i64,
) -> Result<(), String> {
    let mut existing = ExistingLogs::load(target, target_id)?;
    for log in stored_logs(source, source_id)? {
        if !existing.matches(&log, true) {
            target.execute(
                "INSERT INTO task_logs(task_id,log_type,content,created_at,operation_id) VALUES(?,?,?,?,?)",
                params![target_id, log.raw_type, log.content, log.at, log.operation],
            ).map_err(display_error)?;
        }
    }
    Ok(())
}
