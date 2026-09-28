import { useEffect,useId,useState } from "react";
import { CheckCircle2,Layers3,X } from "lucide-react";
import type { LegalTask,SubtaskCompletionState } from "../types";

interface Props {
  task:LegalTask;
  state:SubtaskCompletionState;
  onClose:()=>void;
  onComplete:(includeSubtasks:boolean)=>Promise<void>;
}

export default function TaskCompletionDialog({task,state,onClose,onComplete}:Props){
  const [saving,setSaving]=useState<"parent"|"all"|null>(null);
  const [error,setError]=useState("");
  const titleId=useId();
  const remaining=Math.max(0,state.eligibleSubtasks-state.completedEligibleSubtasks);
  useEffect(()=>{
    const close=(event:KeyboardEvent)=>{if(event.key==="Escape"&&!saving)onClose();};
    window.addEventListener("keydown",close);
    return()=>window.removeEventListener("keydown",close);
  },[onClose,saving]);
  const complete=async(includeSubtasks:boolean)=>{
    setSaving(includeSubtasks?"all":"parent");setError("");
    try{await onComplete(includeSubtasks);}
    catch(reason){setError(reason instanceof Error?reason.message:String(reason));setSaving(null);}
  };
  return <div className="modal-layer completion-dialog-layer" role="presentation" onMouseDown={event=>event.target===event.currentTarget&&!saving&&onClose()}>
    <section className="completion-dialog" role="dialog" aria-modal="true" aria-labelledby={titleId}>
      <header><div><span>完成事项</span><h2 id={titleId}>{task.title}</h2></div><button type="button" disabled={Boolean(saving)} onClick={onClose} aria-label="关闭"><X size={17}/></button></header>
      <div className="completion-dialog-body">
        <div className="completion-dialog-mark"><Layers3 size={22}/></div>
        <div><strong>仍有 {remaining} 个子任务未完成</strong><p>父任务和子任务状态相互独立。请选择本次需要完成的范围。</p></div>
      </div>
      {error&&<p className="completion-dialog-error">{error}</p>}
      <footer><button type="button" className="button secondary" disabled={Boolean(saving)} onClick={onClose}>取消</button><button type="button" className="button secondary" disabled={Boolean(saving)} onClick={()=>void complete(false)}><CheckCircle2 size={15}/>{saving==="parent"?"处理中":"仅完成父任务"}</button><button autoFocus type="button" className="button primary" disabled={Boolean(saving)} onClick={()=>void complete(true)}><Layers3 size={15}/>{saving==="all"?"处理中":`同时完成 ${remaining} 个子任务`}</button></footer>
    </section>
  </div>;
}
