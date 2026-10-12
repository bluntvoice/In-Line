import {useEffect,useRef,useState} from "react";
import {listen} from "@tauri-apps/api/event";
import {api} from "../api";
const states:Record<string,string>={queued:"排队中",running:"执行中",paused:"限流暂停",recovering:"恢复核验中",cancelling:"取消中",cancelled:"已取消",publishing:"正在保存文件",completed:"已完成",failed:"已终止"};
const kinds:Record<string,string>={query_tasks:"事项查询",query_task_history:"事项时间线",query_work_calendar:"工作日历",get_report_summary:"统计汇总",export:"业务导出"};
export default function McpJobsSetting(){
  const [data,setData]=useState<Awaited<ReturnType<typeof api.mcpJobsState>>|null>(null),[error,setError]=useState(""),[busy,setBusy]=useState(false),[confirm,setConfirm]=useState<string|null>(null);const lock=useRef(false);
  useEffect(()=>{let active=true;let dispose:(()=>void)|undefined;const reload=()=>{if(active)void api.mcpJobsState().then(v=>{if(active)setData(v);}).catch(e=>{if(active)setError(String(e));});};reload();void listen("mcp-jobs-changed",reload).then(fn=>{if(active)dispose=fn;else fn();});return()=>{active=false;dispose?.();};},[]);
  const run=async(action:()=>Promise<unknown>)=>{if(lock.current)return;lock.current=true;setBusy(true);setError("");try{await action();setConfirm(null);setData(await api.mcpJobsState());}catch(e){setError(String(e));}finally{lock.current=false;setBusy(false);}};
  return <details className="mcp-security-setting"><summary>后台查询与导出{data?.items.some(j=>["queued","running","paused","recovering","cancelling","publishing"].includes(j.state))?" · 有任务进行中":""}</summary>
    {error&&<p role="alert" className="mcp-security-error">{error}</p>}
    <p>客户端断开后任务可以继续。取消在检查点生效；已保存的导出文件会保留。</p>
    {data?.items.map(j=><div className="mcp-issued" key={j.jobId}><strong>{kinds[j.kind]??j.kind} · {states[j.state]??j.state}</strong><p>{j.progress.processed} / {j.progress.total} 条{j.rounds.length>1?` · 第 ${j.rounds.length} 轮`:""}{j.errorCode?` · ${j.errorCode}`:""}</p>{["queued","running","paused","recovering"].includes(j.state)&&(confirm===j.jobId?<div className="mcp-security-controls"><span>取消整个任务？其他等待者也将停止等待。</span><button className="button secondary danger" disabled={busy} onClick={()=>void run(()=>api.mcpCancelJob(j.jobId))}>确认取消</button><button className="button secondary" disabled={busy} onClick={()=>setConfirm(null)}>继续任务</button></div>:<button className="button secondary" disabled={busy} onClick={()=>setConfirm(j.jobId)}>取消任务</button>)}</div>)}
    <div className="mcp-security-controls"><button className="button secondary" disabled={busy} onClick={()=>void run(()=>api.mcpCleanupJobs())}>清理过期任务数据</button><small>临时结果保留 7 天，摘要保留 30 天；进行中任务及正式导出不清理。</small></div>
  </details>;
}
