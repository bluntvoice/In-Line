# 授权存储兼容修复：开发 Action 交付

日期：2026-10-26。用户明确确认提交推送、生成开发测试安装包并保存为 Actions Artifact，不创建 Tag 或 GitHub Release。本机安装、真实授权迁移和当前会话认证仍分别待确认及验证。

## 初次提交与云端发现

- 修复及诊断提交：`685a8d872a084f112bec5fe8f323e01e856d3138`，已推送 `feat/scheduled-items`。
- [CI 40287775269](https://github.com/bluntvoice/In-Line/actions/runs/40287775269) 与[开发 Action 40287792461](https://github.com/bluntvoice/In-Line/actions/runs/40287792461) 均在 Rust 检查失败，未生成安装包。前端检查通过；Rust lib 为 126 通过、75 失败，失败集中于 `storage_redirected`。
- 本机用 Profile 下独立合成目录复现了路径校验缺陷：同一文件通过长路径及 Windows 8.3 别名访问，文件 ID 完全相同，但旧比较方式误判为重定向。新增回归在修复前实际失败，未通过更换测试目录或跳过断言绕开。

## 补充修复

路径文本不等时，通过 Windows `GetLongPathNameW` 展开逻辑名称，再与实际句柄路径比较；不把两边都解析成物理句柄目标，避免掩盖真正的 MSIX 重定向。[微软 API 文档](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getlongpathnamew)说明该接口将短名称转成长名称。

实际 sidecar 回归覆盖通过真实 8.3 别名读取合成授权，诊断中不再误报重定向，且两个 verified 字段保持 false。对现场旧授权仅做名称元数据探针，展开后仍是原逻辑 AppData 路径，与已观测的 Codex 私有物理文件不同；没有读取凭证字节、改写文件、改动授权或重试旧导入。

本地检查快照及原本地安装包见[兼容修复报告](MSIX-STORAGE-COMPATIBILITY.md)，该包早于此次别名补丁，不作为最终 Action 交付包。后续以本报告记录的成功 Action、确定提交和安装包摘要为准。

补丁提交前 236 项 Rust、格式及 16 工具契约检查全部通过，真实短名称别名分支已在本机执行；[检查快照](MSIX-STORAGE-SHORT-PATH-checks.json)保留修复前失败、修复后检查及只读元数据证据，后续云端构建状态另记。

## 最终构建与下载

补丁检查和新的 Action 构建结果完成后追加于此；本段未完成前不能视为已交付安装包。
