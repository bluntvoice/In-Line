import {useEffect,useRef,useState} from "react";
import {listen} from "@tauri-apps/api/event";
import {api} from "../api";
import {auditDiff,auditLabel} from "../lib/mcp-audit";
import type {McpAuditState} from "../lib/mcp-audit";
export default function McpAuditSetting(){
  const [state,setState]=useState<McpAuditState|null>(null),[error,setError]=useState(""),[busy,setBusy]=useState(false),[confirm,setConfirm]=useState<number|null>(null);
  const lock=useRef(false);
  const load=()=>api.mcpAuditState().then(setState).catch(e=>setError(String(e)));
  useEffect(()=>{let alive=true;const reload=()=>{if(alive)void load();};reload();let dispose:(()=>void)|undefined;void listen("mcp-audit-changed",reload).then(fn=>{if(alive)dispose=fn;else fn();});return()=>{alive=false;dispose?.();};},[]);
  const resolve=async(id:number,approve:boolean)=>{if(lock.current)return;lock.current=true;setBusy(true);setError("");try{await api.mcpResolveUndo(id,approve);setConfirm(null);await load();}catch(e){setError(String(e));}finally{lock.current=false;setBusy(false);}};
  return <details className="mcp-security-setting"><summary>AI 操作记录与撤销申请{state?.pending.length?` · ${state.pending.length} 待确认`:""}</summary>
    {error&&<p role="alert" className="mcp-security-error">{error}</p>}
    <p>撤销须由你批准。已发生的办理保留原记录并作废；编号不回收，重新入队按现有规则取号。若现状冲突，整项撤销停止。</p>
    <button className="button secondary" disabled={busy} onClick={()=>void load()}>刷新记录</button>
    {state?.pending.map(request=>{const audit=state.audits.find(a=>a.id===request.auditId);return <div key={request.id} className="mcp-issued"><strong>撤销申请 #{request.auditId}{audit?` · ${auditLabel(audit.action)}`:""}</strong><p>{request.reason}</p>{audit&&<details><summary>查看原操作前后变化</summary>{auditDiff(audit).map(diff=><p key={diff.key}><b>{diff.key}</b><br/>{JSON.stringify(diff.before)??"未设置"} → {JSON.stringify(diff.after)??"未设置"}</p>)}</details>}{confirm===request.id?<div className="mcp-security-controls"><span>确认撤销？历史统计可能改变。</span><button className="button secondary danger" disabled={busy} onClick={()=>void resolve(request.id,true)}>确认撤销</button><button className="button secondary" disabled={busy} onClick={()=>setConfirm(null)}>取消</button></div>:<div className="mcp-security-controls"><button className="button secondary" disabled={busy||!audit} onClick={()=>setConfirm(request.id)}>审阅并撤销</button><button className="button secondary" disabled={busy} onClick={()=>void resolve(request.id,false)}>拒绝</button></div>}</div>;})}
    <details><summary>最近操作与待确认记录</summary>{state?.audits.map(audit=><details key={audit.id}><summary>#{audit.id} · {auditLabel(audit.action)} · {audit.taskId?`事项 ${audit.taskId}`:"软件偏好"} · {audit.createdAt}</summary><p>{audit.intent} · {audit.reason}</p>{auditDiff(audit).map(diff=><p key={diff.key}><b>{diff.key}</b><br/>{JSON.stringify(diff.before)??"未设置"} → {JSON.stringify(diff.after)??"未设置"}</p>)}</details>)}</details>
  </details>;
}
