import { useState, type FormEvent } from "react";
import { Check, X } from "lucide-react";
import { api } from "../api";
import type { LegalTask } from "../types";

export default function QueueDialog({task,reopen,onClose,onSaved}:{task:LegalTask;reopen:boolean;onClose:()=>void;onSaved:()=>void}){
  const [inheritDeadline,setInheritDeadline]=useState(false);const [reason,setReason]=useState("");
  const [saving,setSaving]=useState(false);const [error,setError]=useState("");
  const submit=async(e:FormEvent)=>{e.preventDefault();setSaving(true);setError("");try{
    const input={id:task.id,inheritDeadline,reason};if(reopen)await api.reopenTask(input);else await api.enqueueTask(input);onSaved();
  }catch(value){setError(value instanceof Error?value.message:String(value));}finally{setSaving(false);}};
  return <div className="modal-layer nested-modal" onMouseDown={e=>e.target===e.currentTarget&&onClose()}>
    <section className="compact-dialog" role="dialog" aria-modal="true"><header><div><span className="form-kicker">每日排队</span><h2>{reopen?"重新开启并加入今日队列":"加入今日队列"}</h2></div><button className="icon-button" onClick={onClose}><X size={18}/></button></header>
      <form onSubmit={submit}>{error&&<div className="form-error">{error}</div>}
        <p className="dialog-copy">事项编号 <strong>{task.permanentNumber}</strong> 将保持不变，系统会分配新的今日序号。</p>
        <label className="check-row"><input type="checkbox" checked={inheritDeadline} onChange={e=>setInheritDeadline(e.target.checked)}/><span>继承上一轮截止时间</span></label>
        <small className="muted">默认不继承；不继承时新一轮没有截止时间，也不会因上一轮截止时间而逾期。</small>
        {reopen&&<label><span>重新开启原因</span><input maxLength={200} value={reason} onChange={e=>setReason(e.target.value)} placeholder="选填，将记录到事项时间线"/></label>}
        <footer><button type="button" className="button secondary" onClick={onClose}>取消</button><button className="button primary" disabled={saving}><Check size={16}/>{saving?"处理中":"确认加入"}</button></footer>
      </form>
    </section>
  </div>;
}
