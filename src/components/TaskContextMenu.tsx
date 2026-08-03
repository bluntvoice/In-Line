import { Archive,CheckCircle2,Edit3,Eye,Flame,ListPlus,PlayCircle,RotateCcw,Trash2 } from "lucide-react";
import type { LegalTask,TaskView } from "../types";
import { isDeferredStatus } from "../lib/task-utils";

export interface ContextAction{type:"view"|"edit"|"status"|"urgent"|"process"|"complete"|"enqueue"|"reopen"|"archive"|"delete"|"restore";task:LegalTask}
export default function TaskContextMenu({task,view,x,y,onAction,onClose}:{task:LegalTask;view:TaskView;x:number;y:number;onAction:(action:ContextAction)=>void;onClose:()=>void}){
  const fire=(type:ContextAction["type"])=>{onAction({type,task});onClose();};
  const terminal=task.status==="completed"||task.status==="archived"||Boolean(task.archivedAt);
  const canWork=!terminal&&task.status!=="cancelled";
  return <div className="context-backdrop" onPointerDown={onClose} onContextMenu={event=>{event.preventDefault();onClose();}}>
    <div className="context-menu" style={{left:Math.min(x,window.innerWidth-250),top:Math.min(y,window.innerHeight-470)}} onPointerDown={event=>event.stopPropagation()} role="menu">
      {view==="trash"?<button onClick={()=>fire("restore")}><RotateCcw size={16}/>恢复并加入今日队列</button>:<>
        <button onClick={()=>fire("view")}><Eye size={16}/>查看详情</button>
        <button onClick={()=>fire("edit")}><Edit3 size={16}/>编辑事项</button>
        {!terminal&&<button onClick={()=>fire("status")}><CheckCircle2 size={16}/>修改状态</button>}
        {!terminal&&<button onClick={()=>fire("urgent")}><Flame size={16}/>{task.isUrgent?"取消加急":"设置加急"}</button>}
        <span/>
        {canWork&&<button onClick={()=>fire("process")}><PlayCircle size={16}/>本轮已处理</button>}
        {canWork&&<button onClick={()=>fire("complete")}><CheckCircle2 size={16}/>本轮已完成</button>}
        {!task.hasActiveQueue&&isDeferredStatus(task.status)&&<button onClick={()=>fire("enqueue")}><ListPlus size={16}/>加入今日队列</button>}
        {terminal&&<button onClick={()=>fire("reopen")}><RotateCcw size={16}/>重新开启并加入今日队列</button>}
        {(task.status==="completed"||task.status==="cancelled")&&<button onClick={()=>fire("archive")}><Archive size={16}/>归档</button>}
        <span/>
        <button className="danger" onClick={()=>fire("delete")}><Trash2 size={16}/>移入回收站</button>
      </>}
    </div>
  </div>;
}
