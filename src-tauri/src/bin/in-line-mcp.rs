use chrono::{Local, NaiveDate, TimeZone};
use in_line_lib::{
    database::Database,
    models::{ReportItemsPage, StatisticsResult},
};
use rmcp::{
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
    transport::stdio,
    ServiceExt,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
struct DateRangeArgs {
    /// 开始日期，格式为 YYYY-MM-DD。
    start_date: String,
    /// 结束日期，格式为 YYYY-MM-DD，包含当天。
    end_date: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
struct ReportItemsArgs {
    /// 开始日期，格式为 YYYY-MM-DD。
    start_date: String,
    /// 结束日期，格式为 YYYY-MM-DD，包含当天。
    end_date: String,
    /// 每页事项数，默认 100，最大 500。
    limit: Option<i64>,
    /// 从第几条事项开始，默认 0。
    offset: Option<i64>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
struct ReportSummaryResponse {
    start_date: String,
    end_date: String,
    timezone_offset_minutes: i32,
    statistics: StatisticsResult,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
struct ReportItemsResponse {
    start_date: String,
    end_date: String,
    timezone_offset_minutes: i32,
    page: ReportItemsPage,
}

struct InLineMcp {
    database: Database,
}

#[tool_router]
impl InLineMcp {
    #[tool(
        description = "读取指定日期范围内的 In Line 事项统计汇总，用于生成日报、周报、月报或年报。结束日期包含当天。只读，不返回联系人、内部备注或已删除事项。",
        annotations(
            title = "读取报告统计汇总",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    fn get_report_summary(
        &self,
        Parameters(args): Parameters<DateRangeArgs>,
    ) -> Result<Json<ReportSummaryResponse>, String> {
        let range = report_range(&args.start_date, &args.end_date)?;
        let statistics = self.database.statistics(
            range.start_time,
            range.end_time,
            range.timezone_offset_minutes,
        )?;
        Ok(Json(ReportSummaryResponse {
            start_date: args.start_date,
            end_date: args.end_date,
            timezone_offset_minutes: range.timezone_offset_minutes,
            statistics,
        }))
    }

    #[tool(
        description = "分页读取指定日期范围内有有效办理记录的事项，用于编写报告明细。返回标题、部门、类型、当前状态、工作量、完成时间和有效办理记录；不返回联系人、事项详情、内部备注、普通操作日志或已删除事项。",
        annotations(
            title = "读取报告事项明细",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    fn list_report_items(
        &self,
        Parameters(args): Parameters<ReportItemsArgs>,
    ) -> Result<Json<ReportItemsResponse>, String> {
        let range = report_range(&args.start_date, &args.end_date)?;
        let page = self.database.report_items(
            range.start_time,
            range.end_time,
            args.limit.unwrap_or(100),
            args.offset.unwrap_or(0),
        )?;
        Ok(Json(ReportItemsResponse {
            start_date: args.start_date,
            end_date: args.end_date,
            timezone_offset_minutes: range.timezone_offset_minutes,
            page,
        }))
    }
}

#[rmcp::tool_handler(
    name = "in-line-reports",
    instructions = "这是 In Line 的本地只读报告数据源。生成日报、周报、月报或年报时，先调用 get_report_summary 获取汇总，再按需调用 list_report_items 获取明细并根据 hasMore 继续分页。结束日期包含当天。不得声称可通过本服务修改事项；本服务不提供联系人、内部备注、事项详情、普通日志或回收站数据。"
)]
impl rmcp::ServerHandler for InLineMcp {}

struct ReportRange {
    start_time: String,
    end_time: String,
    timezone_offset_minutes: i32,
}

fn report_range(start: &str, end: &str) -> Result<ReportRange, String> {
    let start_date = NaiveDate::parse_from_str(start, "%Y-%m-%d")
        .map_err(|_| "开始日期必须使用 YYYY-MM-DD 格式".to_string())?;
    let end_date = NaiveDate::parse_from_str(end, "%Y-%m-%d")
        .map_err(|_| "结束日期必须使用 YYYY-MM-DD 格式".to_string())?;
    if end_date < start_date {
        return Err("结束日期不能早于开始日期".into());
    }
    if (end_date - start_date).num_days() > 370 {
        return Err("单次报告范围不能超过 371 天，请分段读取".into());
    }
    let end_exclusive = end_date.succ_opt().ok_or("结束日期超出支持范围")?;
    let start_time = Local
        .from_local_datetime(&start_date.and_hms_opt(0, 0, 0).expect("有效日期应包含零点"))
        .single()
        .ok_or("无法确定开始日期对应的本地时间")?;
    let end_time = Local
        .from_local_datetime(
            &end_exclusive
                .and_hms_opt(0, 0, 0)
                .expect("有效日期应包含零点"),
        )
        .single()
        .ok_or("无法确定结束日期对应的本地时间")?;
    Ok(ReportRange {
        start_time: start_time.to_rfc3339(),
        end_time: end_time.to_rfc3339(),
        timezone_offset_minutes: start_time.offset().local_minus_utc() / 60,
    })
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("In Line MCP 启动失败：{error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let database = Database::open_reporting().map_err(std::io::Error::other)?;
    let service = InLineMcp { database }.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_range_includes_the_end_date() {
        let range = report_range("2026-08-01", "2026-08-07").unwrap();
        assert!(range.start_time.starts_with("2026-08-01T00:00:00"));
        assert!(range.end_time.starts_with("2026-08-08T00:00:00"));
    }

    #[test]
    fn report_range_rejects_reversed_or_oversized_ranges() {
        assert!(report_range("2026-08-08", "2026-08-07").is_err());
        assert!(report_range("2025-01-01", "2026-08-07").is_err());
    }
}
