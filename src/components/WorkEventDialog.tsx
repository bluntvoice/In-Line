import { useState, type FormEvent } from "react";
import { AlertTriangle, Check, X } from "lucide-react";
import { api } from "../api";
import type { LegalTask, TaskWorkEvent, WorkResult } from "../types";
import { fromDateTimeLocalValue, STATUS_LABELS, toDateTimeLocalValue } from "../lib/task-utils";

const results:WorkResult[]=["processed","completed","waiting_materials","waiting_confirmation","waiting_counterparty_confirmation"];
const historyWarning="此操作将改变该事项的统计归属期间，并可能影响历史周报、月报或季度统计。是否继续？";

export default function WorkEventDialog({task,event,onClose,onSaved}:{task:LegalTask;event?:TaskWorkEvent;onClose:()=>void;onSaved:()=>void}){
  const [handledAt,setHandledAt]=useState(()=>toDateTimeLocalValue(event?.handledAt??new Date().toISOString()));
  const [result,setResult]=useState<WorkResult>(event?.resultStatus??"processed");
  const [note,setNote]=useState(event?.note??"");
  const [syncStatus,setSyncStatus]=useState(!event&&task.status!=="completed"&&task.status!=="archived");
  const [saving,setSaving]=useState(false);const [error,setError]=useState("");
  const save=async(confirmHistoricalImpact=false)=>{
    const iso=fromDateTimeLocalValue(handledAt);if(!iso){setError("请选择有效的处理时间");return;}
    setSaving(true);setError("");
    try{
      if(event)await api.updateWorkEvent({id:event.id,resultStatus:result,handledAt:iso,note,confirmHistoricalImpact});
      else await api.recordWorkEvent({taskId:task.id,resultStatus:result,handledAt:iso,note,syncStatus});
      onSaved();
    }catch(reason){
      const message=reason instanceof Error?reason.message:String(reason);
      if(event&&!confirmHistoricalImpact&&message.includes("统计归属期间")&&window.confirm(historyWarning)){await save(true);return;}
      setError(message);
    }finally{setSaving(false);}
  };
  const submit=(e:FormEvent)=>{e.preventDefault();void save();};
  return <div className="modal-layer nested-modal" onMouseDown={e=>e.target===e.currentTarget&&onClose()}>
    <section className="compact-dialog" role="dialog" aria-modal="true">
      <header><div><span className="form-kicker">结构化处理活动</span><h2>{event?"编辑处理活动":"记录本次处理"}</h2></div><button className="icon-button" onClick={onClose}><X size={18}/></button></header>
      <form onSubmit={submit}>
        {error&&<div className="form-error"><AlertTriangle size={15}/>{error}</div>}
        <label><span>处理日期和时间</span><input type="datetime-local" value={handledAt} onChange={e=>setHandledAt(e.target.value)}/></label>
        <label><span>本次处理结果</span><select value={result} disabled={Boolean(event&&!event.canDelete)} onChange={e=>setResult(e.target.value as WorkResult)}>{results.map(value=><option value={value} key={value}>{STATUS_LABELS[value]}</option>)}</select></label>
        <label><span>处理说明</span><textarea rows={3} maxLength={2000} value={note} onChange={e=>setNote(e.target.value)} placeholder="选填：本轮工作内容、结论或后续安排"/></label>
        {!event&&<label className="check-row"><input type="checkbox" checked={syncStatus} onChange={e=>setSyncStatus(e.target.checked)}/><span>同步修改事项当前状态</span></label>}
        {event?.isFirstValid&&<p className="history-warning"><AlertTriangle size={14}/>这是该事项最早的有效处理活动，修改处理时间可能改变历史统计归属。</p>}
        <footer><button type="button" className="button secondary" onClick={onClose}>取消</button><button className="button primary" disabled={saving}><Check size={16}/>{saving?"保存中":"保存"}</button></footer>
      </form>
    </section>
  </div>;
}
