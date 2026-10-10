use in_line_lib::mcp::contract::*;
use in_line_lib::mcp::query_types::*;
use in_line_lib::mcp::write_types::*;
use rmcp::{
    handler::server::wrapper::Parameters, model::CallToolResult, tool, tool_router,
    transport::stdio, ServiceExt,
};
use serde_json::Value;
struct InLineMcp {
    credentials: Credentials,
}
impl InLineMcp {
    async fn call(&self, tool: &str, args: Value) -> CallToolResult {
        let id =
            in_line_lib::mcp::platform::random_secret().unwrap_or_else(|_| "unavailable".into());
        #[cfg(windows)]
        let result = in_line_lib::mcp::ipc::call(self.credentials.clone(), tool, args).await;
        #[cfg(not(windows))]
        let result = Err(McpError::new("unsupported"));
        let failed = result.is_err();
        let value =
            serde_json::to_value(Envelope::result(id, result)).expect("envelope serializable");
        if failed {
            CallToolResult::structured_error(value)
        } else {
            CallToolResult::structured(value)
        }
    }
}
#[tool_router]
impl InLineMcp {
    #[tool(
        description = "按用户明确指令单事项创建、局部编辑、状态、加急或记录真实办理。先检索疑似重复和准确目标，使用query_tasks的字段版本/原值或taskVersion；敏感字段不可回显。操作缘由必填，业务/权限冲突不重试，结果不明只用原幂等键核验，禁止换键盲目重放。",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn mutate_task(&self, Parameters(args): Parameters<MutateArgs>) -> CallToolResult {
        self.call("mutate_task", serde_json::to_value(args).unwrap())
            .await
    }
    #[tool(
        description = "读取或按明确用户指令调整白名单普通偏好（字体、缩放、编号色等），patch需写权限、原值与幂等键。禁止安全、恢复、更新和自启动设置。",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn manage_preferences(
        &self,
        Parameters(args): Parameters<PreferenceArgs>,
    ) -> CallToolResult {
        self.call("manage_preferences", serde_json::to_value(args).unwrap())
            .await
    }
    #[tool(
        description = "请求撤销本客户端AI操作，必须在In Line软件内由用户审阅前后变化并批准；此工具只提交申请，不执行逆向写入。",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn request_undo(&self, Parameters(args): Parameters<UndoArgs>) -> CallToolResult {
        self.call("request_undo", serde_json::to_value(args).unwrap())
            .await
    }
    #[tool(
        description = "认证后按白名单组合条件检索事项，常规/完整投影及限域强制执行；每页最多100，后续使用原条件和不透明cursor或snapshot。不得将关联推断保存为事实。",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn query_tasks(&self, Parameters(args): Parameters<QueryArgs>) -> CallToolResult {
        self.call("query_tasks", serde_json::to_value(args).unwrap())
            .await
    }
    #[tool(
        description = "认证后读取单事项完整可用时间线，包括状态、队列、办理、加急和原日志；敏感自由文本、日志、作废和回收站详细读取需要完整权限；稳定分页最多100。",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn query_task_history(
        &self,
        Parameters(args): Parameters<HistoryArgs>,
    ) -> CallToolResult {
        self.call("query_task_history", serde_json::to_value(args).unwrap())
            .await
    }
    #[tool(
        description = "认证后读取真实工作日历办理事件及队列区间，含授权范围内结构化汇总；日期结束含当天，每页100，固定快照分页，不创建未来日历事件。",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn query_work_calendar(
        &self,
        Parameters(args): Parameters<CalendarArgs>,
    ) -> CallToolResult {
        self.call("query_work_calendar", serde_json::to_value(args).unwrap())
            .await
    }
    #[tool(
        description = "当前客户端命名查询条件的list/get/save/delete；save/delete须用户明确要求且有日常写入权限，只保存查询定义。执行查询时再次鉴权，模板不能扩大权限。",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn manage_saved_query(
        &self,
        Parameters(args): Parameters<SavedQueryArgs>,
    ) -> CallToolResult {
        self.call("manage_saved_query", serde_json::to_value(args).unwrap())
            .await
    }

    #[tool(
        output_schema = rmcp::handler::server::common::schema_for_type::<Envelope>(),
        description = "查询当前客户端的认证状态、有效权限、实际工具与不支持能力；也需要认证。",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn get_capabilities(
        &self,
        Parameters(args): Parameters<CapabilityArgs>,
    ) -> CallToolResult {
        self.call("get_capabilities", serde_json::to_value(args).unwrap())
            .await
    }
    #[tool(
        output_schema = rmcp::handler::server::common::schema_for_type::<Envelope>(),
        description = "认证后读取日期范围内真实办理统计。常规读取权限；按部门及类型限域；结束日期包含当天。",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn get_report_summary(
        &self,
        Parameters(args): Parameters<DateRangeArgs>,
    ) -> CallToolResult {
        self.call("get_report_summary", serde_json::to_value(args).unwrap())
            .await
    }
    #[tool(
        output_schema = rmcp::handler::server::common::schema_for_type::<Envelope>(),
        description = "认证后分页读取有效办理事项；每页最多100条。按部门及类型限域；办理自由文本仅完整读取权限可见。不返回联系人、详情或内部备注。",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn list_report_items(
        &self,
        Parameters(args): Parameters<ReportItemsArgs>,
    ) -> CallToolResult {
        self.call("list_report_items", serde_json::to_value(args).unwrap())
            .await
    }
}
#[rmcp::tool_handler(
    name = "in-line",
    instructions = "所有调用均要求独立客户端环境凭证。先get_capabilities检查真实能力。本阶段只读；不得声称支持事项写入、永久删除、恢复数据库或附件操作。统一返回status/data/error，遇到权限错误交由用户在软件内处理，不得自行调整授权。"
)]
impl rmcp::ServerHandler for InLineMcp {}
#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        #[cfg(windows)]
        let result = if args.len() == 2 && args[0] == "--import-client" {
            let run = async {
                #[cfg(not(debug_assertions))]
                in_line_lib::mcp::onboarding::require_available("codex")?;
                let root = in_line_lib::mcp::platform::root()?;
                let home = in_line_lib::mcp::codex_import::codex_home()?;
                let report = in_line_lib::mcp::codex_import::import_at(
                    &root,
                    &home,
                    &args[1],
                    chrono::Utc::now().timestamp(),
                )
                .await?;
                serde_json::to_value(report).map_err(|_| McpError::new("import_failed"))
            };
            run.await
        } else {
            Err(McpError::new("import_invalid"))
        };
        #[cfg(not(windows))]
        let result: Result<Value, McpError> = Err(McpError::new("unsupported"));
        let failed = result.is_err();
        let response = Envelope::result(
            in_line_lib::mcp::platform::random_secret().unwrap_or_else(|_| "unavailable".into()),
            result,
        );
        println!(
            "{}",
            serde_json::to_string(&response).expect("redacted result serializable")
        );
        if failed {
            std::process::exit(1);
        }
        return;
    }
    let service = InLineMcp {
        credentials: Credentials::from_env(),
    };
    match service.serve(stdio()).await {
        Ok(server) => {
            let _ = server.waiting().await;
        }
        Err(_) => {
            eprintln!("In Line MCP协议连接失败，请检查客户端配置");
            std::process::exit(1);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn report_range_includes_end() {
        let (s, e, _) = report_range("2026-08-01", "2026-08-07").unwrap();
        assert!(s.starts_with("2026-08-01T"));
        assert!(e.starts_with("2026-08-08T"));
    }
    #[test]
    fn rejects_invalid_ranges() {
        assert!(report_range("2026-08-08", "2026-08-07").is_err());
        assert!(report_range("2025-01-01", "2026-08-07").is_err());
        assert!(report_range("2026-8-1", "2026-08-07").is_err());
    }
}
