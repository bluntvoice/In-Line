import {useEffect,useRef,useState} from "react";
import {listen} from "@tauri-apps/api/event";
import {api} from "../api";
type BatchState=Awaited<ReturnType<typeof api.mcpBatchState>>;
const fieldLabel:Record<string,string>={workEvents:"办理记录",plannedDate:"加入日期",ticketDate:"队列日期",dailySequence:"队列号码",isScheduled:"未来事项",requestedDeadline:"截止时间",status:"状态",hasActiveQueue:"有效入队",scheduleAction:"计划调整",relatedTask:"关联事项",title:"名称",parentTaskId:"所属任务",subtaskSortOrder:"子任务顺序",customSortOrder:"队列顺序"};
export default function McpBatchSetting(){
  const [state,setState]=useState<BatchState|null>(null),[error,setError]=useState(""),[confirm,setConfirm]=useState<number|null>(null),[limit,setLimit]=useState(20),[busy,setBusy]=useState(false);const lock=useRef(false);
  const load=async()=>{const value=await api.mcpBatchState();setState(value);setLimit(value.batchLimit);};
  useEffect(()=>{let active=true;let dispose:(()=>void)|undefined;const reload=()=>{if(active)void load().catch(e=>setError(String(e)));};reload();void listen("mcp-audit-changed",reload).then(fn=>{if(active)dispose=fn;else fn();});return()=>{active=false;dispose?.();};},[]);
  const run=async(operation:()=>Promise<unknown>)=>{if(lock.current)return;lock.current=true;setBusy(true);setError("");try{await operation();setConfirm(null);await load();}catch(e){setError(String(e));}finally{lock.current=false;setBusy(false);}};
  return <details className="mcp-security-setting"><summary>批量操作{state?.pending.length?` · ${state.pending.length} 待确认`:""}</summary>
    {error&&<p role="alert" className="mcp-security-error">{error}</p>}
    <p>混合操作整批成功或回滚。多事项改期需在这里审阅后一次确认；取消或拒绝不会修改事项。</p>
    <div className="mcp-security-controls"><label>每批事项上限 <input aria-label="每批事项上限" type="number" min={1} max={100} value={limit} onChange={e=>setLimit(Number(e.target.value))}/></label><button className="button secondary" disabled={busy||!Number.isInteger(limit)||limit<1||limit>100} onClick={()=>void run(()=>api.mcpSetBatchLimit(limit))}>保存</button></div>
    {state?.pending.map(r=><div className="mcp-issued" key={r.id}><strong>批量改期 · {r.request.operations.length} 项</strong><p>{r.request.reason}</p><details><summary>查看整批影响</summary>{r.preview.items.map((item,index)=><div key={index}><b>事项 {item.taskId}</b>{item.differences.map((d,n)=><p key={n}>{fieldLabel[d.field]??d.field}：{JSON.stringify(d.before)??"未设置"} → {JSON.stringify(d.after)??`事项 ${d.taskId}`}</p>)}</div>)}</details>{confirm===r.id?<div className="mcp-security-controls"><span>确认整批改期？旧号码作废且不回收；现状冲突时整批停止。</span><button className="button secondary danger" disabled={busy} onClick={()=>void run(()=>api.mcpResolveBatch(r.id,true))}>确认整批执行</button><button className="button secondary" disabled={busy} onClick={()=>setConfirm(null)}>取消</button></div>:<div className="mcp-security-controls"><button className="button secondary" disabled={busy} onClick={()=>setConfirm(r.id)}>审阅并确认</button><button className="button secondary" disabled={busy} onClick={()=>void run(()=>api.mcpResolveBatch(r.id,false))}>拒绝</button></div>}</div>)}
  </details>;
}
