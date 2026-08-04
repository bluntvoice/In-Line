import { useMemo,useState } from "react";
import { GitMerge,X } from "lucide-react";
import { api } from "../api";
import type { LegalTask } from "../types";

export default function MergeTaskDialog({target,candidates,onClose,onMerged}:{target:LegalTask;candidates:LegalTask[];onClose:()=>void;onMerged:()=>void}){
  const options=useMemo(()=>candidates.filter(task=>task.id!==target.id&&!task.deletedAt).sort((a,b)=>b.updatedAt.localeCompare(a.updatedAt)),[candidates,target.id]);
  const [sourceId,setSourceId]=useState("");
  const [deduplicateRecords,setDeduplicateRecords]=useState(true);
  const [trashSource,setTrashSource]=useState(true);
  const [saving,setSaving]=useState(false);
  const [error,setError]=useState("");
  const source=options.find(task=>task.id===Number(sourceId));
  const submit=async()=>{
    if(!source){setError("请选择需要并入的重复事项");return;}
    setSaving(true);setError("");
    try{
      await api.mergeTasks({targetTaskId:target.id,sourceTaskId:source.id,deduplicateRecords,trashSource});
      onMerged();
    }catch(value){setError(String(value));setSaving(false);}
  };
  return <div className="modal-layer" role="dialog" aria-modal="true" aria-labelledby="merge-task-title">
    <div className="merge-task-panel">
      <header className="form-header"><div><span className="form-kicker">保留主事项及全部关联历史</span><h2 id="merge-task-title"><GitMerge size={19}/>合并重复事项</h2></div><button className="icon-button" onClick={onClose} aria-label="关闭"><X size={18}/></button></header>
      <div className="merge-task-body">
        {error&&<p className="form-error">{error}</p>}
        <section className="merge-target"><span>主事项（保留）</span><strong>{target.permanentNumber} · {target.title}</strong></section>
        <label className="form-field"><span>选择需要并入的重复事项</span><select value={sourceId} onChange={event=>setSourceId(event.target.value)} disabled={saving}><option value="">请选择事项</option>{options.map(task=><option key={task.id} value={task.id}>{task.permanentNumber} · {task.title}</option>)}</select></label>
        {source&&<p className="merge-explanation">“{source.title}”的办理记录、时间线、状态、加急和历次排队记录将转入主事项。主事项本身的标题、状态和当前编号不变。</p>}
        <label className="merge-check"><input type="checkbox" checked={deduplicateRecords} onChange={event=>setDeduplicateRecords(event.target.checked)} disabled={saving}/><span><strong>自动去除完全重复的记录</strong><small>办理时间、结果、来源和内容完全相同时只保留一份；其他记录全部保留。</small></span></label>
        <label className="merge-check"><input type="checkbox" checked={trashSource} onChange={event=>setTrashSource(event.target.checked)} disabled={saving}/><span><strong>合并后将重复事项移入回收站</strong><small>取消勾选时，重复事项会保留在历史归档中，可稍后手动删除。</small></span></label>
      </div>
      <footer className="form-actions"><button className="button secondary" onClick={onClose} disabled={saving}>取消</button><button className="button primary" onClick={()=>void submit()} disabled={saving||!source}>{saving?"正在合并…":"确认合并"}</button></footer>
    </div>
  </div>;
}
