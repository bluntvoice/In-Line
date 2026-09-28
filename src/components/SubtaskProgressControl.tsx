import { useEffect,useRef,useState } from "react";
import { createPortal } from "react-dom";
import { AlertTriangle,CheckCircle2,ExternalLink,Plus,RotateCcw,X } from "lucide-react";
import type { LegalTask } from "../types";
import { displayTicket,formatDeadline,isOverdue } from "../lib/task-utils";
import { subtaskProgress } from "../lib/subtask-progress";
import StatusBadge from "./StatusBadge";

interface Props {
  parent:LegalTask;
  subtasks:LegalTask[];
  onOpenTask:(task:LegalTask)=>void;
  onAddSubtask:(parent:LegalTask)=>void;
  onToggleCompletion:(task:LegalTask)=>Promise<void>;
}

export default function SubtaskProgressControl({parent,subtasks,onOpenTask,onAddSubtask,onToggleCompletion}:Props){
  const [position,setPosition]=useState<{left:number;top:number}|null>(null);
  const [busyId,setBusyId]=useState<number|null>(null);
  const [error,setError]=useState("");
  const panelRef=useRef<HTMLElement>(null);
  const progress=subtaskProgress(subtasks);

  useEffect(()=>{
    if(!position)return;
    const close=(event:KeyboardEvent)=>{if(event.key==="Escape")setPosition(null);};
    window.addEventListener("keydown",close);
    window.setTimeout(()=>panelRef.current?.focus(),0);
    return()=>window.removeEventListener("keydown",close);
  },[position]);

  const open=(event:React.MouseEvent<HTMLButtonElement>)=>{
    event.stopPropagation();
    const rect=event.currentTarget.getBoundingClientRect();
    const width=Math.min(360,window.innerWidth-16);
    const left=Math.max(8,Math.min(rect.left,window.innerWidth-width-8));
    const estimatedHeight=Math.min(430,100+subtasks.length*76);
    const below=rect.bottom+7;
    const top=Math.max(8,below+estimatedHeight<=window.innerHeight-8?below:rect.top-estimatedHeight-7);
    setError("");setPosition({left,top});
  };
  const toggle=async(task:LegalTask)=>{
    setBusyId(task.id);setError("");
    try{await onToggleCompletion(task);}
    catch(reason){setError(reason instanceof Error?reason.message:String(reason));}
    finally{setBusyId(null);}
  };

  return <>
    <button type="button" className="subtask-progress-trigger" onClick={open} aria-haspopup="dialog" aria-expanded={Boolean(position)} aria-label={`查看子任务，已完成 ${progress.completed} 项，共 ${progress.total} 项`} title={`子任务 ${progress.completed}/${progress.total}`}>
      <svg viewBox="0 0 24 24" aria-hidden="true"><circle className="progress-track" cx="12" cy="12" r="9"/><circle className="progress-value" cx="12" cy="12" r="9" pathLength="100" strokeDasharray={`${progress.percentage} 100`}/></svg>
      <span>子任务 <b>{progress.completed}/{progress.total}</b></span>
    </button>
    {position&&createPortal(<div className="subtask-popover-layer" onPointerDown={()=>setPosition(null)}>
      <section ref={panelRef} tabIndex={-1} className="subtask-popover" role="dialog" aria-label={`${parent.title}的子任务`} style={position} onPointerDown={event=>event.stopPropagation()}>
        <header><div><span>子任务进度</span><strong>{parent.title}</strong></div><button type="button" aria-label="关闭" onClick={()=>setPosition(null)}><X size={16}/></button></header>
        <div className="subtask-popover-summary"><span><b>{progress.completed}</b> 项已完成</span><span>共 {progress.total} 项</span></div>
        {error&&<div className="relation-error"><AlertTriangle size={14}/>{error}</div>}
        <div className="subtask-popover-list">
          {subtasks.map(task=>{const completed=task.status==="completed";const disabled=Boolean(task.deletedAt)||task.status==="cancelled"||task.status==="archived"||Boolean(task.archivedAt);return <article key={task.id}>
            <button type="button" className="popover-subtask-main" onClick={()=>{setPosition(null);onOpenTask(task);}} title={task.title}><strong>{task.title}</strong><span><b>{displayTicket(task)}</b><StatusBadge status={task.status} overdue={isOverdue(task)}/>{task.isUrgent&&<em>加急</em>}</span></button>
            <div className="popover-subtask-meta"><span>{formatDeadline(task.requestedDeadline,task.requestedDeadlineLabel)}</span><span>{task.hasActiveQueue?"今日队列中":"未加入队列"}</span></div>
            <div className="popover-subtask-actions"><button type="button" disabled={disabled||busyId===task.id} title={completed?"取消完成并恢复为待处理":"快速完成"} onClick={()=>void toggle(task)}>{completed?<RotateCcw size={14}/>:<CheckCircle2 size={14}/>}<span>{completed?"取消完成":"完成"}</span></button><button type="button" title="进入详情" onClick={()=>{setPosition(null);onOpenTask(task);}}><ExternalLink size={14}/></button></div>
          </article>})}
        </div>
        <footer><button type="button" onClick={()=>{setPosition(null);onAddSubtask(parent);}}><Plus size={15}/>添加子任务</button></footer>
      </section>
    </div>,document.body)}
  </>;
}
