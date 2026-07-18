import { useEffect,useState } from "react";
import { Archive,Check,Edit3,Pencil,RotateCcw,Trash2,X } from "lucide-react";
import type { LegalTask,TaskLog,TaskStatus,TaskView } from "../types";
import { api } from "../api";
import { formatDateTime,formatDeadline,PRIORITY_LABELS,STATUS_LABELS,WORKLOAD_LABELS } from "../lib/task-utils";
import StatusBadge from "./StatusBadge";
import TicketNumber from "./TicketNumber";

export default function TaskDetail({task,view,onClose,onEdit,onChanged}:{task:LegalTask;view:TaskView;onClose:()=>void;onEdit:()=>void;onChanged:()=>void}){
  const [logs,setLogs]=useState<TaskLog[]>([]);const [note,setNote]=useState("");
  const [editingLog,setEditingLog]=useState<number|null>(null);const [editingContent,setEditingContent]=useState("");
  const refresh=async()=>setLogs(await api.getLogs(task.id));
  useEffect(()=>{void refresh();},[task.id]);
  const status=async(value:TaskStatus)=>{await api.setTaskStatus(task.id,value);onChanged();};
  const add=async()=>{if(!note.trim())return;await api.addLog(task.id,note);setNote("");await refresh();};
  const saveLog=async()=>{if(editingLog===null||!editingContent.trim())return;await api.updateLog(editingLog,editingContent);setEditingLog(null);setEditingContent("");await refresh();};
  const removeLog=async(id:number)=>{if(!window.confirm("删除这条手动处理记录？"))return;await api.deleteLog(id);await refresh();};
  return <aside className="detail-panel">
    <header><div><TicketNumber task={task}/><h2>{task.title}</h2></div><button className="icon-button" onClick={onClose} aria-label="关闭"><X size={18}/></button></header>
    <div className="detail-actions">
      {view==="trash"?<button className="button primary" onClick={async()=>{await api.restoreTask(task.id);onChanged();}}><RotateCcw size={16}/>恢复</button>:<>
        <button className="button secondary" onClick={onEdit}><Edit3 size={16}/>编辑</button>
        {task.status==="completed"||task.status==="cancelled"?<button className="button secondary" onClick={async()=>{await api.archiveTask(task.id);onChanged();}}><Archive size={16}/>归档</button>:<button className="button primary" onClick={()=>void status("completed")}>标记完成</button>}
        <button className="icon-button danger" onClick={async()=>{await api.deleteTask(task.id);onChanged();}} aria-label="移入回收站"><Trash2 size={16}/></button>
      </>}
    </div>
    <dl className="detail-grid">
      <div><dt>状态</dt><dd><StatusBadge status={task.status}/></dd></div><div><dt>优先级</dt><dd>{PRIORITY_LABELS[task.priority]}</dd></div>
      <div><dt>部门 / 团队</dt><dd>{task.department}</dd></div><div><dt>对接人</dt><dd>{task.contact}</dd></div>
      <div><dt>事项类型</dt><dd>{task.taskType}</dd></div><div><dt>预计工作量</dt><dd>{WORKLOAD_LABELS[task.workload]}</dd></div>
      <div><dt>截止时间</dt><dd>{formatDeadline(task.requestedDeadline,task.requestedDeadlineLabel)}</dd></div><div><dt>永久编号</dt><dd>{task.permanentNumber}</dd></div>
    </dl>
    <section><h3>事项详情</h3><p className={task.details?"detail-copy":"muted"}>{task.details||"未填写"}</p></section>
    {task.isUrgent&&<section className="urgent-box"><h3>加急信息</h3><p><strong>{task.urgentRequester}</strong>：{task.urgentReason}</p></section>}
    {task.internalNotes&&<section><h3>内部备注</h3><p className="detail-copy">{task.internalNotes}</p></section>}
    <section className="timeline"><h3>处理记录</h3><div className="log-compose"><input value={note} onChange={e=>setNote(e.target.value)} placeholder="补充一条处理记录"/><button onClick={()=>void add()}>添加</button></div>
      {logs.map(log=><article key={log.id} className="timeline-entry"><div className="timeline-meta"><time>{formatDateTime(log.createdAt)}</time>{log.logType==="note"&&<span className="timeline-actions">{editingLog===log.id?<><button title="保存" onClick={()=>void saveLog()}><Check size={14}/></button><button title="取消" onClick={()=>{setEditingLog(null);setEditingContent("");}}><X size={14}/></button></>:<><button title="编辑" onClick={()=>{setEditingLog(log.id);setEditingContent(log.content);}}><Pencil size={14}/></button><button className="danger" title="删除" onClick={()=>void removeLog(log.id)}><Trash2 size={14}/></button></>}</span>}</div>{editingLog===log.id?<textarea className="log-edit" rows={3} maxLength={2000} value={editingContent} onChange={event=>setEditingContent(event.target.value)}/>:<p>{log.content}</p>}</article>)}
      {!logs.length&&<p className="muted">暂无处理记录</p>}
    </section>
  </aside>;
}
