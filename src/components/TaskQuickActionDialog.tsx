import { useEffect,useId,useState,type FormEvent,type KeyboardEvent } from "react";
import { AlertTriangle,Check,Flame,X } from "lucide-react";
import { api } from "../api";
import type { LegalTask,TaskStatus } from "../types";
import { isDeferredStatus,STATUS_LABELS } from "../lib/task-utils";
import StatusBadge from "./StatusBadge";

export type QuickActionMode="status"|"urgent";

const SWITCHABLE_STATUSES:TaskStatus[]=[
  "pending","processing","waiting_materials","waiting_confirmation",
  "waiting_counterparty_confirmation","paused","processed","completed","cancelled"
];

export default function TaskQuickActionDialog({task,mode,onClose,onSaved}:{task:LegalTask;mode:QuickActionMode;onClose:()=>void;onSaved:()=>void}){
  const [status,setStatus]=useState<TaskStatus>(task.status);
  const [requester,setRequester]=useState(task.urgentRequester);
  const [reason,setReason]=useState(task.urgentReason);
  const [saving,setSaving]=useState(false);
  const [error,setError]=useState("");
  const titleId=useId();
  const urgentTitle=task.isUrgent?"取消加急":"设置加急";

  useEffect(()=>{
    const close=(event:globalThis.KeyboardEvent)=>{if(event.key==="Escape")onClose();};
    window.addEventListener("keydown",close);
    return()=>window.removeEventListener("keydown",close);
  },[onClose]);

  const submit=async(event?:FormEvent)=>{
    event?.preventDefault();
    setSaving(true);setError("");
    try{
      if(mode==="status")await api.setTaskStatus(task.id,status);
      else await api.setTaskUrgent(task.id,!task.isUrgent,requester,reason);
      onSaved();
    }catch(reasonValue){
      setError(reasonValue instanceof Error?reasonValue.message:String(reasonValue));
    }finally{setSaving(false);}
  };
  const enterToSave=(event:KeyboardEvent<HTMLFormElement>)=>{
    if(event.key!=="Enter"||event.shiftKey||(event.target as HTMLElement).tagName==="BUTTON")return;
    if(saving||(mode==="status"&&status===task.status)||(mode==="urgent"&&!task.isUrgent&&(!requester.trim()||!reason.trim())))return;
    event.preventDefault();void submit();
  };
  const statusChanged=status!==task.status;
  const leavesQueue=isDeferredStatus(status)||["completed","cancelled"].includes(status);
  const entersQueue=["pending","processing"].includes(status)&&!task.hasActiveQueue;
  const clearsUrgent=task.isUrgent&&(isDeferredStatus(status)||status==="completed");
  const urgentReady=task.isUrgent||Boolean(requester.trim()&&reason.trim());

  return <div className="modal-layer quick-action-layer" role="presentation" onMouseDown={event=>event.target===event.currentTarget&&onClose()}>
    <section className="quick-action-panel" role="dialog" aria-modal="true" aria-labelledby={titleId}>
      <header className="form-header">
        <div><span className="form-kicker">快捷操作</span><h2 id={titleId}>{mode==="status"?"修改状态":urgentTitle}</h2></div>
        <button className="icon-button" type="button" onClick={onClose} aria-label="关闭"><X size={18}/></button>
      </header>
      <form onSubmit={submit} onKeyDown={enterToSave}>
        {error&&<div className="form-error"><AlertTriangle size={15}/>{error}</div>}
        <div className="quick-task-summary"><strong>{task.title}</strong><span>{task.permanentNumber}</span></div>
        {mode==="status"?<div className="quick-action-content">
          <div className="quick-current"><span>当前状态</span><StatusBadge status={task.status}/></div>
          <label><span>目标状态</span><select autoFocus value={status} onChange={event=>setStatus(event.target.value as TaskStatus)}>
            {SWITCHABLE_STATUSES.map(value=><option value={value} key={value}>{STATUS_LABELS[value]}</option>)}
          </select></label>
          {statusChanged&&(entersQueue||leavesQueue||clearsUrgent)&&<div className="quick-action-note">
            {entersQueue&&<span>确认后会按现有规则加入今日队列，并保留当前截止时间。</span>}
            {leavesQueue&&task.hasActiveQueue&&<span>确认后会退出当前队列。</span>}
            {clearsUrgent&&<span>该状态会同时取消事项加急。</span>}
          </div>}
        </div>:<div className="quick-action-content">
          <div className="quick-current"><span>当前状态</span><b className={task.isUrgent?"urgent-state on":"urgent-state"}>{task.isUrgent?"已加急":"未加急"}</b></div>
          {task.isUrgent?<div className="urgent-summary"><div><span>加急申请人</span><strong>{task.urgentRequester||"未记录"}</strong></div><div><span>加急原因</span><strong>{task.urgentReason||"未记录"}</strong></div><p>确认后将取消加急标识，并保留事项的其他信息。</p></div>:<>
            <label><span>加急申请人 *</span><input autoFocus maxLength={100} value={requester} onChange={event=>setRequester(event.target.value)} placeholder="填写申请人"/></label>
            <label><span>加急原因 *</span><input maxLength={500} value={reason} onChange={event=>setReason(event.target.value)} placeholder="填写加急原因"/></label>
          </>}
        </div>}
        <footer className="quick-action-footer"><button type="button" className="button secondary" onClick={onClose}>取消</button><button autoFocus={mode==="urgent"&&task.isUrgent} className={`button primary${mode==="urgent"&&task.isUrgent?" danger-primary":""}`} disabled={saving||(mode==="status"?!statusChanged:!urgentReady)}>{mode==="urgent"?<Flame size={15}/>:<Check size={15}/>} {saving?"处理中":mode==="status"?"确认修改":urgentTitle}</button></footer>
      </form>
    </section>
  </div>;
}
