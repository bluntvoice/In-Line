import { CheckCircle2,X } from "lucide-react";
import { useState } from "react";
import type { LegalTask } from "../types";

export default function SubtaskCompletionNotice({parent,onClose,onCompleteParent}:{parent:LegalTask;onClose:()=>void;onCompleteParent:()=>Promise<void>}){
  const [saving,setSaving]=useState(false);
  const complete=async()=>{setSaving(true);try{await onCompleteParent();}finally{setSaving(false);}};
  return <aside className="subtask-completion-notice" role="status">
    <span><CheckCircle2 size={19}/></span>
    <div><strong>所有子任务均已完成</strong><p title={parent.title}>{parent.title}</p></div>
    <button type="button" className="notice-close" disabled={saving} onClick={onClose} aria-label="关闭"><X size={15}/></button>
    <button type="button" className="notice-complete-parent" disabled={saving} onClick={()=>void complete()}>{saving?"处理中":"完成父任务"}</button>
  </aside>;
}
