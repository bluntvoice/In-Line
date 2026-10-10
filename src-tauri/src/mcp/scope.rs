use super::contract::McpError;
use serde::{Deserialize, Serialize};
#[derive(Clone, Serialize, Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Scope {
    pub departments: Option<Vec<String>>,
    pub task_types: Option<Vec<String>>,
}
impl Scope {
    pub fn allows(&self, stored_departments: &str, task_type: &str) -> bool {
        let names = serde_json::from_str::<Vec<String>>(stored_departments)
            .unwrap_or_else(|_| vec![stored_departments.into()]);
        let names = names
            .iter()
            .map(|x| x.trim())
            .filter(|x| !x.is_empty())
            .collect::<Vec<_>>();
        self.departments.as_ref().is_none_or(|allowed| {
            !names.is_empty() && names.iter().all(|name| allowed.iter().any(|a| a == name))
        }) && self
            .task_types
            .as_ref()
            .is_none_or(|allowed| allowed.iter().any(|a| a == task_type))
    }
    pub fn validate(&self) -> Result<(), McpError> {
        for list in [&self.departments, &self.task_types].into_iter().flatten() {
            if list.len() > 100
                || list.iter().any(|x| {
                    x.trim() != x
                        || x.is_empty()
                        || x.len() > 400
                        || x.chars().any(char::is_control)
                })
            {
                return Err(McpError::new("invalid_arguments"));
            }
        }
        Ok(())
    }
    pub fn register(&self, connection: &rusqlite::Connection) -> Result<(), String> {
        let scope = self.clone();
        connection
            .create_scalar_function(
                "mcp_scope",
                2,
                rusqlite::functions::FunctionFlags::SQLITE_UTF8
                    | rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC,
                move |ctx| Ok(scope.allows(&ctx.get::<String>(0)?, &ctx.get::<String>(1)?)),
            )
            .map_err(|e| e.to_string())
    }
}
