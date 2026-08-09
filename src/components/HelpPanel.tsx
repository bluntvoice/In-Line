import {Archive,BarChart3,CheckCircle2,Clock3,DatabaseBackup,Image,Keyboard,PauseCircle,Plus,TicketCheck} from "lucide-react";

const steps=[
  {icon:Plus,title:"新增取号",copy:"点击左侧“新增取号”，或使用软件设置中的全局快捷键，填写事项后保存；还可复制生成取号图片，方便发给对接人。"},
  {icon:TicketCheck,title:"按队列办理",copy:"待办队列按逾期、加急和人工调序展示；单击事项进入详情。"},
  {icon:CheckCircle2,title:"记录处理结果",copy:"每轮办理后选择已处理、已完成或对应暂缓状态，保留可统计的办理记录。"},
  {icon:BarChart3,title:"查看统计与报告",copy:"在统计中心选择周期，查看图表、导出 300 DPI 图片，或预览原生工作报告。"}
];

export default function HelpPanel(){
  return <section className="help-panel">
    <header><p>四步完成日常工作</p><h1>使用说明</h1><span>In Line 的核心流程是：取号 → 排队 → 办理 → 复盘。</span></header>
    <div className="quick-start-track">{steps.map(({icon:Icon,title,copy},index)=><article key={title}><span className="quick-start-number">{String(index+1).padStart(2,"0")}</span><Icon size={20}/><div><h2>{title}</h2><p>{copy}</p></div></article>)}</div>
    <section className="shortcut-note"><Keyboard size={22}/><div><strong>全局快捷新增：默认 Alt+I</strong><p>In Line 在后台或窗口隐藏时也能直接打开独立新增取号页面；可在“软件设置 → 全局快捷新增”中直接按键录入自定义组合。软件会先检查系统注册冲突，发生占用时提示原因并保留原设置。</p></div></section>
    <section className="help-tips"><h2>日常使用建议</h2><div><p><Image size={17}/><span><strong>发送取号图片：</strong>取号后可复制事项的取号图片，再粘贴到微信、邮件等工具，让对接人快速核对编号和基本信息。</span></p><p><Clock3 size={17}/><span><strong>及时设置期限：</strong>有明确时间要求的事项应填写截止日期；队列会优先提示逾期事项，减少遗漏。</span></p><p><PauseCircle size={17}/><span><strong>暂缓管理：</strong>等待材料、内部确认或对方确认的事项会进入暂缓事项，恢复办理时再重新排队。</span></p><p><CheckCircle2 size={17}/><span><strong>每轮都留记录：</strong>办理后及时选择处理结果并填写办理说明，原生报告和统计数据会更完整、更准确。</span></p><p><Archive size={17}/><span><strong>完成后再归档：</strong>已完成且暂时不需查看的事项可以归档；误删事项先去回收站恢复，永久删除前请确认无需保留。</span></p><p><BarChart3 size={17}/><span><strong>每周复盘：</strong>周末打开统计中心，先看逾期、事项类型和趋势，再选择原生报告，或通过 MCP 交给 AI 生成完整报告。</span></p><p><DatabaseBackup size={17}/><span><strong>定期备份：</strong>重要调整前或每周固定创建一次本地备份，并偶尔确认备份文件可以正常看到。</span></p><p><TicketCheck size={17}/><span><strong>保持队列聚焦：</strong>只把正在推进的事项放在待办队列，等待类事项使用暂缓状态，减少队列噪音。</span></p></div></section>
  </section>;
}
