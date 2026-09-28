import { useEffect,useState } from "react";
import { AlertTriangle,Archive,ArrowDown,ArrowUp,Check,CheckCircle2,Edit3,GitMerge,GripVertical,ListPlus,MoreHorizontal,Pencil,PlayCircle,Plus,RotateCcw,ShieldCheck,Trash2,X } from "lucide-react";
import type { LegalTask,TaskLog,TaskView,TaskWorkEvent } from "../types";
import { api } from "../api";
import { displayTicket,formatDateTime,formatDeadline,isDeferredStatus,isOverdue,localizeStatusText,PRIORITY_LABELS,WORKLOAD_LABELS } from "../lib/task-utils";
import { moveById,reorderById } from "../lib/subtask-ui";
import StatusBadge from "./StatusBadge";
import TicketNumber from "./TicketNumber";
import QueueDialog from "./QueueDialog";
import MergeTaskDialog from "./MergeTaskDialog";
import type { QuickActionMode } from "./TaskQuickActionDialog";

const historyWarning="这是该事项最早的有效办理记录，删除后可能改变历史周报、月报或季度统计。";
const sourceLabel=(source:string)=>source==="manual"?"手动记录":source==="quick_action"?"快捷处理":"状态变化自动记录";

interface Props {
  task:LegalTask;
  view:TaskView;
  mergeCandidates:LegalTask[];
  relationRefreshKey?:number;
  onClose:()=>void;
  onEdit:()=>void;
  onChanged:()=>void;
  onRelationshipChanged:()=>void;
  onAddSubtask:(parent:LegalTask)=>void;
  onOpenTask:(task:LegalTask)=>void;
  onQuickAction:(task:LegalTask,mode:QuickActionMode)=>void;
  onOpenContext:(task:LegalTask,x:number,y:number)=>void;
  notify?:(message:string)=>void;
}

export default function TaskDetail({task,view,mergeCandidates,relationRefreshKey=0,onClose,onEdit,onChanged,onRelationshipChanged,onAddSubtask,onOpenTask,onQuickAction,onOpenContext,notify=()=>undefined}:Props){
  const [logs,setLogs]=useState<TaskLog[]>([]);const [events,setEvents]=useState<TaskWorkEvent[]>([]);const [note,setNote]=useState("");
  const [editingLog,setEditingLog]=useState<number|null>(null);const [editingContent,setEditingContent]=useState("");
  const [queueDialog,setQueueDialog]=useState<"enqueue"|"reopen"|null>(null);const [mergeDialog,setMergeDialog]=useState(false);
  const [subtasks,setSubtasks]=useState<LegalTask[]>([]);const [parentCandidates,setParentCandidates]=useState<LegalTask[]>([]);const [parentTask,setParentTask]=useState<LegalTask|null>(null);
  const [relationParentId,setRelationParentId]=useState<number|null>(task.parentTaskId);const [relationError,setRelationError]=useState("");
  const [relationLoading,setRelationLoading]=useState(true);const [relationSaving,setRelationSaving]=useState(false);const [orderSaving,setOrderSaving]=useState(false);const [draggedTaskId,setDraggedTaskId]=useState<number|null>(null);
  const refresh=async()=>{const [nextLogs,nextEvents]=await Promise.all([api.getLogs(task.id),api.getWorkEvents(task.id)]);setLogs(nextLogs);setEvents(nextEvents);};
  const refreshRelations=async()=>{
    setRelationLoading(true);setRelationError("");
    try{
      const [nextSubtasks,nextCandidates,nextParent]=await Promise.all([
        api.listSubtasks(task.id),
        view==="trash"?Promise.resolve([]):api.listParentTaskCandidates(task.id),
        task.parentTaskId===null?Promise.resolve(null):api.getTask(task.parentTaskId).catch(()=>null)
      ]);
      setSubtasks(nextSubtasks);setParentCandidates(nextCandidates);setParentTask(nextParent);setRelationParentId(task.parentTaskId);
    }catch(error){setRelationError(error instanceof Error?error.message:String(error));}
    finally{setRelationLoading(false);}
  };
  useEffect(()=>{void refresh();},[task.id]);
  useEffect(()=>{void refreshRelations();},[task.id,task.parentTaskId,view,relationRefreshKey]);
  const add=async()=>{if(!note.trim())return;await api.addLog(task.id,note);setNote("");await refresh();};
  const saveLog=async()=>{if(editingLog===null||!editingContent.trim())return;await api.updateLog(editingLog,editingContent);setEditingLog(null);setEditingContent("");await refresh();};
  const removeLog=async(id:number)=>{if(!window.confirm("删除这条普通处理备注？"))return;await api.deleteLog(id);await refresh();};
  const removeEvent=async(event:TaskWorkEvent)=>{const impact=event.isFirstValid?`${historyWarning}\n\n`:"";if(!window.confirm(`${impact}删除这条办理记录？删除后将不再计入处理轮次和统计，但不会自动回退事项当前状态。`))return;await api.voidWorkEvent(event.id,event.isFirstValid);notify("办理记录已删除，事项当前状态未改变");await refresh();onChanged();};
  const process=async()=>{await api.processRound(task.id);notify("已记录本轮处理，事项已进入暂缓队列");onChanged();};
  const complete=async()=>{await api.completeRound(task.id);notify("已记录本轮完成，事项整体结束");onChanged();};
  const changeParent=async(value:string)=>{
    const nextId=value?Number(value):null;
    setRelationSaving(true);setRelationError("");
    try{
      await api.setParentTask(task.id,nextId);
      setRelationParentId(nextId);
      setParentTask(nextId===null?null:parentCandidates.find(candidate=>candidate.id===nextId)??parentTask);
      notify(nextId===null?"已解除所属任务":"所属任务已更新");
      onRelationshipChanged();
    }catch(error){setRelationError(error instanceof Error?error.message:String(error));}
    finally{setRelationSaving(false);}
  };
  const persistSubtaskOrder=async(next:LegalTask[])=>{
    if(next===subtasks)return;
    const previous=subtasks;setSubtasks(next);setRelationError("");setOrderSaving(true);
    try{await api.reorderSubtasks({parentTaskId:task.id,taskIds:next.map(child=>child.id)});notify("子任务顺序已更新");onRelationshipChanged();}
    catch(error){setSubtasks(previous);setRelationError(error instanceof Error?error.message:String(error));}
    finally{setOrderSaving(false);}
  };
  const moveSubtask=(childId:number,direction:"up"|"down")=>void persistSubtaskOrder(moveById(subtasks,childId,direction));
  const dropSubtask=(targetId:number)=>{
    if(draggedTaskId===null)return;
    const next=reorderById(subtasks,draggedTaskId,targetId);setDraggedTaskId(null);void persistSubtaskOrder(next);
  };
  const terminal=task.status==="completed"||task.status==="archived"||Boolean(task.archivedAt);
  const canWork=!terminal&&task.status!=="cancelled";
  const relationOptions=parentTask&&!parentCandidates.some(candidate=>candidate.id===parentTask.id)?[parentTask,...parentCandidates]:parentCandidates;
  const canManageRelations=view!=="trash";
  return <aside className="detail-panel">
    <header><div><TicketNumber task={task}/><h2>{task.title}</h2></div><button className="icon-button" onClick={onClose} aria-label="关闭"><X size={18}/></button></header>
    {task.isImportConflict&&<div className="import-conflict-banner"><AlertTriangle size={18}/><div><strong>导入冲突待复核</strong><span>此事项由备份导入，存在同名但内容不同的当前事项。可以使用“合并”处理重复数据；如果两项都应保留，请在核对后解除标识。</span></div><button className="button secondary small" onClick={async()=>{if(!window.confirm("确认此事项与当前同名事项无需合并，并解除“导入冲突”标识？"))return;await api.resolveImportConflict(task.id);notify("导入冲突标识已解除");onChanged();}}><ShieldCheck size={15}/>已复核</button></div>}
    <div className="detail-actions">
      {view==="trash"?<><button className="button primary" onClick={async()=>{await api.restoreTask(task.id);notify("事项已恢复并加入今日队列");onChanged();}}><RotateCcw size={16}/>恢复</button><button className="button secondary danger" onClick={async()=>{if(!window.confirm(`永久删除“${task.title}”？事项及其全部办理记录将不可恢复。`))return;await api.permanentlyDeleteTasks([task.id]);notify("事项已永久删除");onChanged();}}><Trash2 size={16}/>永久删除</button></>:<>
        <button className="button secondary" onClick={onEdit}><Edit3 size={16}/>编辑</button><button className="button secondary" onClick={()=>setMergeDialog(true)}><GitMerge size={16}/>合并</button>
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
    <section className="task-relations" aria-busy={relationLoading}>
      <div className="section-heading relation-heading"><div><h3>{subtasks.length?`子任务（${subtasks.length}）`:"所属任务"}</h3><small>{subtasks.length?"拖拽事项条可调整同一父任务内的显示顺序":"父子归属不影响状态、截止时间或真实排队顺序"}</small></div>{subtasks.length>0&&canManageRelations&&<button type="button" className="button secondary small" onClick={()=>onAddSubtask(task)}><Plus size={14}/>添加子任务</button>}</div>
      {relationError&&<div className="relation-error"><AlertTriangle size={14}/>{relationError}</div>}
      {!subtasks.length&&<div className="parent-relation-card">
        {relationParentId!==null&&<button type="button" className="parent-task-link" disabled={!parentTask} onClick={()=>parentTask&&onOpenTask(parentTask)} title={parentTask?.title}>
          <span>当前所属</span><strong>{parentTask?.title??`事项 #${relationParentId}`}</strong>{parentTask&&<><TicketNumber task={parentTask}/><StatusBadge status={parentTask.status}/></>}
        </button>}
        {canManageRelations?<label className="parent-task-select"><span>{relationParentId===null?"设置所属任务":"更换或解除所属"}</span><select value={relationParentId??""} disabled={relationSaving||relationLoading} onChange={event=>void changeParent(event.target.value)}><option value="">不设置所属任务</option>{relationOptions.map(candidate=><option value={candidate.id} key={candidate.id}>{candidate.title} · {candidate.permanentNumber}{candidate.archivedAt||candidate.status==="archived"?" · 已归档":""}</option>)}</select></label>:relationParentId===null&&<p className="muted">当前事项没有所属任务。</p>}
        {!relationLoading&&relationParentId===null&&parentCandidates.length===0&&canManageRelations&&<small className="relation-empty">暂无可选父任务；只有未进入回收站的顶层事项可作为父任务。</small>}
      </div>}
      {subtasks.length>0&&<div className="subtask-list">
        {subtasks.map((child,index)=><article key={child.id} className={`subtask-strip${draggedTaskId===child.id?" dragging":""}${child.deletedAt?" deleted":""}`} draggable={canManageRelations&&!orderSaving&&!child.deletedAt} onDragStart={event=>{setDraggedTaskId(child.id);event.dataTransfer.effectAllowed="move";event.dataTransfer.setData("text/plain",String(child.id));}} onDragEnd={()=>setDraggedTaskId(null)} onDragOver={event=>{if(draggedTaskId!==null){event.preventDefault();event.dataTransfer.dropEffect="move";}}} onDrop={event=>{event.preventDefault();dropSubtask(child.id);}} onContextMenu={event=>{event.preventDefault();onOpenContext(child,event.clientX,event.clientY);}}>
          <div className="subtask-order" title="拖拽调整顺序"><GripVertical size={15}/><b>{index+1}</b></div>
          <button type="button" className="subtask-main" onClick={()=>onOpenTask(child)} title={child.title}><strong>{child.title}</strong><span><TicketNumber task={child}/><StatusBadge status={child.status} overdue={isOverdue(child)}/>{child.isUrgent&&<em>加急</em>}{child.deletedAt&&<em className="neutral">回收站</em>}</span></button>
          <div className="subtask-facts"><span>{formatDeadline(child.requestedDeadline,child.requestedDeadlineLabel)}</span><span>{child.hasActiveQueue?`已入队 · ${displayTicket(child)}`:"未加入队列"}</span></div>
          <div className="subtask-actions">
            <button type="button" disabled={Boolean(child.deletedAt)} title="修改状态" aria-label={`修改“${child.title}”的状态`} onClick={()=>onQuickAction(child,"status")}><CheckCircle2 size={15}/></button>
            <button type="button" disabled={!canManageRelations||orderSaving||index===0||Boolean(child.deletedAt)} title="上移" aria-label={`上移“${child.title}”`} onClick={()=>moveSubtask(child.id,"up")}><ArrowUp size={15}/></button>
            <button type="button" disabled={!canManageRelations||orderSaving||index===subtasks.length-1||Boolean(child.deletedAt)} title="下移" aria-label={`下移“${child.title}”`} onClick={()=>moveSubtask(child.id,"down")}><ArrowDown size={15}/></button>
            <button type="button" title="更多操作" aria-label={`打开“${child.title}”的操作菜单`} onClick={event=>{const rect=event.currentTarget.getBoundingClientRect();onOpenContext(child,rect.right,rect.bottom);}}><MoreHorizontal size={16}/></button>
          </div>
        </article>)}
      </div>}
      {!relationLoading&&subtasks.length===0&&relationParentId===null&&canManageRelations&&<button type="button" className="add-first-subtask" onClick={()=>onAddSubtask(task)}><Plus size={15}/><span><strong>把当前事项作为父任务</strong><small>添加第一个子任务</small></span></button>}
    </section>
    <section><h3>事项详情</h3><p className={task.details?"detail-copy":"muted"}>{task.details||"未填写"}</p></section>
    {task.isUrgent&&<section className="urgent-box"><h3>加急信息</h3><p><strong>{task.urgentRequester}</strong>：{task.urgentReason}</p></section>}
    {task.internalNotes&&<section><h3>内部备注</h3><p className="detail-copy">{task.internalNotes}</p></section>}
    {view!=="trash"&&<section className="work-events"><div className="section-heading"><div><h3>办理记录</h3></div></div>
      {events.map(event=><article className="work-event" key={event.id}><div><StatusBadge status={event.resultStatus}/><time>{formatDateTime(event.handledAt)}</time><small>{sourceLabel(event.source)} · {event.taskTypeSnapshot}</small></div>{event.note&&<p>{event.note}</p>}<span className="timeline-actions"><button className="danger" title="删除办理记录" onClick={()=>void removeEvent(event)}><Trash2 size={14}/></button></span></article>)}
      {!events.length&&<p className="muted">暂无办理记录</p>}
    </section>}
    <section className="timeline"><h3>事项时间线</h3><div className="log-compose"><input value={note} onChange={e=>setNote(e.target.value)} placeholder="补充一条普通处理备注"/><button onClick={()=>void add()}>添加</button></div>
      {logs.map(log=><article key={log.id} className="timeline-entry"><div className="timeline-meta"><time>{formatDateTime(log.createdAt)}</time>{log.logType==="note"&&<span className="timeline-actions">{editingLog===log.id?<><button title="保存" onClick={()=>void saveLog()}><Check size={14}/></button><button title="取消" onClick={()=>{setEditingLog(null);setEditingContent("");}}><X size={14}/></button></>:<><button title="编辑" onClick={()=>{setEditingLog(log.id);setEditingContent(log.content);}}><Pencil size={14}/></button><button className="danger" title="删除" onClick={()=>void removeLog(log.id)}><Trash2 size={14}/></button></>}</span>}</div>{editingLog===log.id?<textarea className="log-edit" rows={3} maxLength={2000} value={editingContent} onChange={event=>setEditingContent(event.target.value)}/>:<p>{localizeStatusText(log.content)}</p>}</article>)}
      {!logs.length&&<p className="muted">暂无时间线记录</p>}
    </section>
    {queueDialog&&<QueueDialog task={task} reopen={queueDialog==="reopen"} onClose={()=>setQueueDialog(null)} onSaved={()=>{setQueueDialog(null);notify(queueDialog==="reopen"?"事项已重新开启并加入今日队列":"事项已加入今日队列");onChanged();}}/>}
    {mergeDialog&&<MergeTaskDialog target={task} candidates={mergeCandidates} onClose={()=>setMergeDialog(false)} onMerged={()=>{setMergeDialog(false);notify("事项已合并，相关记录已保留");onChanged();}}/>}
  </aside>;
}
