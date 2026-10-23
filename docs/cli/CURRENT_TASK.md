# CLI 当前任务：修复NSIS主程序类型标记校验，重新运行开发Action

2026-10-23。阶段6已按用户独立确认提交推送e0c20871bad92f3af4aa012bd76ffd35f63d223e，远端核对一致，开发Action已于10月22日触发。CI通过；开发Action测试/NSIS编译/MCP门禁通过，但干净提交检查失败、Artifact未上传。实机安装另安排。

## 权威与恢复

压缩后优先读[阶段6报告](PHASE-06-DELIVERY.md)、用户指南、Agent指南、CLI PRD、PHASE-00-ARCHITECTURE、COMMANDS-DRAFT及原CLI附件。用户于2026-10-22独立确认阶段6开发与本地检查，明确要求提交推送并立即运行已授权Action；实机验收另安排。确认记录见evidence/PHASE-06-user-acceptance.json，不再重复询问提交或构建许可。

## 实现及验证

- 三程序externalBin打包；GUI/MCP显式离线build-info；CLI保持version。IPC鉴权后、业务派发前混版拒绝，原限流/HMAC/16工具契约不变。六版本字段/精确提交/暂存hash/实际NSIS解包门禁加入CI/dev/release；首版不改PATH。
- 219项完整Rust（189库+2sidecar+28集成，真实CLI21），0忽略/过滤，完整会话74338已结束exit0；129前端/22文件、TS/Vite、fmt、debug/release三程序离线身份及16工具完整schema/annotations、匿名16工具+3次拒绝及限流均通过。
- Rust资源峰值提交62%，最小余量16.27GiB；jobs1、串行及资源守卫，不改系统/用户应用。
- 本地NSIS首次feature不同失败；补齐tauri/custom-protocol后发现链接PE时间戳漂移，最终beforeBundleCommand stage-only最后同步，不放宽hash门禁。真实tauri bundle及最终release/暂存逐字节匹配通过。会话17664、78815、16643均已结束，不能继续等待。证据保留first/second及link-drift。
- 完整Rust后最终改动只在打包/校验脚本、beforeBundle配置及包门禁测试；已用129前端、TS/Vite、Node语法及真实bundle/release核验覆盖，不无故重跑219。最终216输入哈希manifest位于release/CLI-Phase6-frozen-source.json，需最终回读。
- 本地包release/CLI-Phase6-local-20261022/In-Line_0.5.0-dev-phase6_x64-setup.exe，4,315,931字节，SHA256 E7A926EF2C6F3C8D4474C8530FA067137629D04F4081BB604E0027A038E5BD2E。基线4194f2b+dirty、exactCommit=null，不改标为阶段6提交；未执行/安装。已安装主程序/sidecar、Codex配置及授权文件哈希未变。

## 待执行及未验证

首次开发run 39737746725，CI run 39737737625。已在隔离副本实际复现Tauri 2.11.4初始化把Cargo.toml写成LF；core.autocrlf=true导致git status误报修改且规范化diff为空。新增.gitattributes仅固定该文件LF，重跑相同初始化后状态干净；未放宽dirty/目标提交/hash检查。详情见evidence/PHASE-06-action-first.json。首次run未到实际NSIS解包步骤，不称为包内校验通过。真实安装/旧库保留/GUI授权审批/当前聊天MCP均待另行安排。没有Tag/Release或生产数据操作。

9327d66414eca29b1430d48c4b9e07f50de6a5cd已推送，CI 39882628623通过；开发run 39882659422测试/NSIS编译/干净提交及暂存身份通过，实际解出三exe后GUI字节比较失败。定位Tauri会把GUI中唯一UNK bundle marker改为NSS再入包，结束后还原源exe。校验器现从源文件按同一固定变换重建完整预期字节再严格比较；其他字节及两个伴随程序均不能变，source/payload SHA分别记录。新增3项回归测试后包测试共7项通过、TS/Vite及脚本语法通过。10月23日从官方7-Zip MSI仅提取工具到ignored release（未安装、未执行MSI），官方asset digest核对；实际本地旧NSIS解包和修复后校验已通过，仍保留4194f2b+dirty身份。旧“本地未解包”记录是10月22日历史状态，不覆盖。准备提交此次校验修复并对新SHA重新构建。

下一步提交推送换行修复，再立即运行已授权dev-build.yml（精确远端新提交），等待真实成功、获取Artifact与SHA。不要再次要求提交或测试包许可。不要把workflow触发当作成功，也不要把隔离实例当作当前聊天MCP验证。旧final-checks和本地包报告保留构建时状态，不回写为后续提交或Action结果；远端执行证据另记实际构建SHA。
