import { useEffect,useState } from "react";
import { Archive,Check,CheckCircle2,Edit3,ListPlus,Pencil,PlayCircle,RotateCcw,Trash2,X } from "lucide-react";
import type { LegalTask,TaskLog,TaskView,TaskWorkEvent } from "../types";
import { api } from "../api";
import { formatDateTime,formatDeadline,isDeferredStatus,isOverdue,localizeStatusText,PRIORITY_LABELS,WORKLOAD_LABELS } from "../lib/task-utils";
import StatusBadge from "./StatusBadge";
import TicketNumber from "./TicketNumber";
import QueueDialog from "./QueueDialog";
import WorkEventDialog from "./WorkEventDialog";

const historyWarning="此操作将改变该事项的统计归属期间，并可能影响历史周报、月报或季度统计。是否继续？";
const sourceLabel=(source:string)=>source==="manual"?"手动记录":source==="quick_action"?"快捷处理":"状态变化自动记录";

export default function TaskDetail({task,view,onClose,onEdit,onChanged,notify=()=>undefined}:{task:LegalTask;view:TaskView;onClose:()=>void;onEdit:()=>void;onChanged:()=>void;notify?:(message:string)=>void}){
  const [logs,setLogs]=useState<TaskLog[]>([]);const [events,setEvents]=useState<TaskWorkEvent[]>([]);const [note,setNote]=useState("");
  const [editingLog,setEditingLog]=useState<number|null>(null);const [editingContent,setEditingContent]=useState("");
  const [workDialog,setWorkDialog]=useState<TaskWorkEvent|null|undefined>(undefined);const [queueDialog,setQueueDialog]=useState<"enqueue"|"reopen"|null>(null);
  const refresh=async()=>{const [nextLogs,nextEvents]=await Promise.all([api.getLogs(task.id),api.getWorkEvents(task.id)]);setLogs(nextLogs);setEvents(nextEvents);};
  useEffect(()=>{void refresh();},[task.id]);
  const add=async()=>{if(!note.trim())return;await api.addLog(task.id,note);setNote("");await refresh();};
  const saveLog=async()=>{if(editingLog===null||!editingContent.trim())return;await api.updateLog(editingLog,editingContent);setEditingLog(null);setEditingContent("");await refresh();};
  const removeLog=async(id:number)=>{if(!window.confirm("删除这条普通处理备注？"))return;await api.deleteLog(id);await refresh();};
  const removeEvent=async(event:TaskWorkEvent)=>{if(!event.canDelete)return;const message=event.isFirstValid?historyWarning:"作废这条结构化处理活动？原记录仍会保留在审计数据中。";if(!window.confirm(message))return;await api.voidWorkEvent(event.id,event.isFirstValid);await refresh();onChanged();};
  const process=async()=>{await api.processRound(task.id);notify("已记录本轮处理，事项已进入暂缓队列");onChanged();};
  const complete=async()=>{await api.completeRound(task.id);notify("已记录本轮完成，事项整体结束");onChanged();};
  const terminal=task.status==="completed"||task.status==="archived"||Boolean(task.archivedAt);
  const canWork=!terminal&&task.status!=="cancelled";
  return <aside className="detail-panel">
    <header><div><TicketNumber task={task} showPermanent/><h2>{task.title}</h2></div><button className="icon-button" onClick={onClose} aria-label="关闭"><X size={18}/></button></header>
    <div className="detail-actions">
      {view==="trash"?<button className="button primary" onClick={async()=>{await api.restoreTask(task.id);notify("事项已恢复并加入今日队列");onChanged();}}><RotateCcw size={16}/>恢复</button>:<>
        <button className="button secondary" onClick={onEdit}><Edit3 size={16}/>编辑</button>
        {terminal?<><button className="button primary" onClick={()=>setQueueDialog("reopen")}><RotateCcw size={16}/>重新开启并加入今日队列</button>{task.status==="completed"&&!task.archivedAt&&<button className="button secondary" onClick={async()=>{await api.archiveTask(task.id);onChanged();}}><Archive size={16}/>归档</button>}</>:<>
          {canWork&&<button className="button secondary" onClick={()=>void process()}><PlayCircle size={16}/>本轮已处理</button>}
          {canWork&&<button className="button primary" onClick={()=>void complete()}><CheckCircle2 size={16}/>本轮已完成</button>}
          {!task.hasActiveQueue&&isDeferredStatus(task.status)&&<button className="button secondary" onClick={()=>setQueueDialog("enqueue")}><ListPlus size={16}/>加入今日队列</button>}
          {(task.status==="completed"||task.status==="cancelled")&&<button className="button secondary" onClick={async()=>{await api.archiveTask(task.id);onChanged();}}><Archive size={16}/>归档</button>}
        </>}
        <button className="icon-button danger" onClick={async()=>{await api.deleteTask(task.id);onChanged();}} aria-label="移入回收站"><Trash2 size={16}/></button>
      </>}
    </div>
    <dl className="detail-grid">
      <div><dt>状态</dt><dd><StatusBadge status={task.status} overdue={isOverdue(task)}/></dd></div><div><dt>累计处理轮次</dt><dd>{task.processingRounds} 次</dd></div>
      <div><dt>优先级</dt><dd>{PRIORITY_LABELS[task.priority]}</dd></div><div><dt>预计工作量</dt><dd>{WORKLOAD_LABELS[task.workload]}</dd></div>
      <div><dt>部门 / 团队</dt><dd>{task.department}</dd></div><div><dt>对接人</dt><dd>{task.contact}</dd></div>
      <div><dt>事项类型</dt><dd>{task.taskType}</dd></div><div><dt>截止时间</dt><dd>{formatDeadline(task.requestedDeadline,task.requestedDeadlineLabel)}</dd></div>
      <div><dt>事项编号</dt><dd>{task.permanentNumber}</dd></div><div><dt>当前排队</dt><dd>{task.hasActiveQueue?"有效队列中":"未加入有效队列"}</dd></div>
    </dl>
    <section><h3>事项详情</h3><p className={task.details?"detail-copy":"muted"}>{task.details||"未填写"}</p></section>
    {task.isUrgent&&<section className="urgent-box"><h3>加急信息</h3><p><strong>{task.urgentRequester}</strong>：{task.urgentReason}</p></section>}
    {task.internalNotes&&<section><h3>内部备注</h3><p className="detail-copy">{task.internalNotes}</p></section>}
    {view!=="trash"&&<section className="work-events"><div className="section-heading"><div><h3>办理记录</h3><small>每完成一次办理就在此记录，并计入统计</small></div><button className="button secondary small" onClick={()=>setWorkDialog(null)}><ListPlus size={15}/>记录本次处理</button></div>
      {events.map(event=><article className="work-event" key={event.id}><div><StatusBadge status={event.resultStatus}/><time>{formatDateTime(event.handledAt)}</time><small>{sourceLabel(event.source)} · {event.taskTypeSnapshot}</small></div>{event.note&&<p>{event.note}</p>}<span className="timeline-actions"><button title="编辑处理活动" onClick={()=>setWorkDialog(event)}><Pencil size={14}/></button>{event.canDelete&&<button className="danger" title="作废处理活动" onClick={()=>void removeEvent(event)}><Trash2 size={14}/></button>}</span></article>)}
      {!events.length&&<p className="muted">暂无办理记录</p>}
    </section>}
    <section className="timeline"><h3>事项时间线</h3><div className="log-compose"><input value={note} onChange={e=>setNote(e.target.value)} placeholder="补充一条普通处理备注"/><button onClick={()=>void add()}>添加</button></div>
      {logs.map(log=><article key={log.id} className="timeline-entry"><div className="timeline-meta"><time>{formatDateTime(log.createdAt)}</time>{log.logType==="note"&&<span className="timeline-actions">{editingLog===log.id?<><button title="保存" onClick={()=>void saveLog()}><Check size={14}/></button><button title="取消" onClick={()=>{setEditingLog(null);setEditingContent("");}}><X size={14}/></button></>:<><button title="编辑" onClick={()=>{setEditingLog(log.id);setEditingContent(log.content);}}><Pencil size={14}/></button><button className="danger" title="删除" onClick={()=>void removeLog(log.id)}><Trash2 size={14}/></button></>}</span>}</div>{editingLog===log.id?<textarea className="log-edit" rows={3} maxLength={2000} value={editingContent} onChange={event=>setEditingContent(event.target.value)}/>:<p>{localizeStatusText(log.content)}</p>}</article>)}
      {!logs.length&&<p className="muted">暂无时间线记录</p>}
    </section>
    {workDialog!==undefined&&<WorkEventDialog task={task} event={workDialog??undefined} onClose={()=>setWorkDialog(undefined)} onSaved={()=>{setWorkDialog(undefined);notify("处理活动已保存");void refresh();onChanged();}}/>}
    {queueDialog&&<QueueDialog task={task} reopen={queueDialog==="reopen"} onClose={()=>setQueueDialog(null)} onSaved={()=>{setQueueDialog(null);notify(queueDialog==="reopen"?"事项已重新开启并加入今日队列":"事项已加入今日队列");onChanged();}}/>}
  </aside>;
}
