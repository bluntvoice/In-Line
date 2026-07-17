import { Archive,CheckCircle2,Edit3,Eye,Flame,RotateCcw,Trash2 } from "lucide-react";
import type { LegalTask,TaskView } from "../types";

export interface ContextAction{type:"view"|"edit"|"status"|"urgent"|"complete"|"archive"|"delete"|"restore";task:LegalTask}
export default function TaskContextMenu({task,view,x,y,onAction,onClose}:{task:LegalTask;view:TaskView;x:number;y:number;onAction:(action:ContextAction)=>void;onClose:()=>void}){
  const fire=(type:ContextAction["type"])=>{onAction({type,task});onClose();};
  return <div className="context-backdrop" onPointerDown={onClose} onContextMenu={event=>{event.preventDefault();onClose();}}>
    <div className="context-menu" style={{left:Math.min(x,window.innerWidth-230),top:Math.min(y,window.innerHeight-360)}} onPointerDown={event=>event.stopPropagation()} role="menu">
      {view==="trash"?<button onClick={()=>fire("restore")}><RotateCcw size={16}/>恢复事项</button>:<>
        <button onClick={()=>fire("view")}><Eye size={16}/>查看详情</button>
        <button onClick={()=>fire("edit")}><Edit3 size={16}/>编辑事项</button>
        <button onClick={()=>fire("status")}><CheckCircle2 size={16}/>修改状态</button>
        <button onClick={()=>fire("urgent")}><Flame size={16}/>{task.isUrgent?"取消加急":"设置加急"}</button>
        <button onClick={()=>fire("complete")}><CheckCircle2 size={16}/>标记完成</button>
        <button onClick={()=>fire("archive")}><Archive size={16}/>归档</button>
        <span/>
        <button className="danger" onClick={()=>fire("delete")}><Trash2 size={16}/>移入回收站</button>
      </>}
    </div>
  </div>;
}
