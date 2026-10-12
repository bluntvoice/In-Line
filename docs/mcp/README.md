当前P8统一验收准备：16工具/schema14，各阶段开发已独立提交推送；[P8报告](acceptance/PHASE-08.md)、[统一验收清单](acceptance/UNIFIED-P8-CHECKLIST.md)、[集成规程](integration-and-regression.md)。严格Codex当前会话自动导入/重载仍未通过，入口关闭。P1–P8未用户验收，未进入P9/Release。下文旧进度为历史。

# In-Line MCP 阶段开发

2026-10-09：P0已验收，C1–C4均选A；已推送提交`f2a97b34410f850f3da8c19d4ec6e21c7d63048f`及固定Tag`mcp-phase-00-accepted`。P1本地实现已加入，等待实机/用户验收，未提交或发布。

## 阅读顺序

1. [三份权威输入](inputs/03-In-Line-MCP-Codex-总执行提示词.md)、[144项PRD](inputs/01-In-Line-MCP-PRD-144项完整需求.md)、[阶段方案](inputs/02-In-Line-MCP-分阶段开发与验收方案.md)。
2. [P1验收报告](acceptance/PHASE-01.md)、[接入与迁移指南](connection-guide.md)、[执行检查点](CURRENT_TASK.md)。
3. [144项矩阵](requirements-traceability.md)及[完整CSV](requirements-traceability.csv)。
4. [架构](architecture.md)、[API](api-draft.md)、[阶段计划](phase-plan.md)、[测试设计](test-plan.md)。
5. [P0历史验收](acceptance/PHASE-00.md)、[权威输入校验](inputs/source-manifest.json)。

输入副本逐字保留，当前实现进度不改写源需求。压缩后先回读D:/Downloads三原件，再回读本目录当前检查点；副本只在原件不可用时使用。

## 阶段门禁

按最新用户授权，各阶段独立开发Commit/Push，P8集中验收后停止等待；开发提交不建accepted Tag。旧P1等待限制已由最新推进授权调整。正式Release需独立授权；本地NSIS构建不代表安装升级或用户验收。
