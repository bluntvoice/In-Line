import { useEffect,useState } from "react";
import { ArrowDown,ArrowUp,Copy,ExternalLink,Grip,Minimize2,Plus,X } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { LogicalSize } from "@tauri-apps/api/dpi";
import { api } from "./api";
import type { LegalTask } from "./types";
import { isOverdue } from "./lib/task-utils";
import StatusBadge from "./components/StatusBadge";
import TaskContextMenu,{type ContextAction} from "./components/TaskContextMenu";

import TicketNumber from "./components/TicketNumber";

export default function FloatingWindow(){
  const [tasks,setTasks]=useState<LegalTask[]>([]);const [mini,setMini]=useState(false);const [message,setMessage]=useState("");const [menu,setMenu]=useState<{task:LegalTask;x:number;y:number}|null>(null);
  const refresh=async()=>setTasks(await api.listTasks("queue"));
  useEffect(()=>{void refresh();return api.onDataChanged(()=>void refresh());},[]);
  const toast=(text:string)=>{setMessage(text);window.setTimeout(()=>setMessage(""),1600);};
  const copy=async(task:LegalTask)=>{try{await api.copyTicketImage(task);toast("已复制："+task.permanentNumber);}catch(error){toast("复制失败："+String(error));}};
  const move=async(event:React.MouseEvent,task:LegalTask,direction:"up"|"down")=>{event.stopPropagation();await api.moveTask(task.id,direction);};
  const action=async(value:ContextAction)=>{
    if(["view","edit","status","urgent"].includes(value.type)){await api.showMain();return;}
    if(value.type==="complete")await api.setTaskStatus(value.task.id,"completed");
    if(value.type==="archive")await api.archiveTask(value.task.id);
    if(value.type==="delete")await api.deleteTask(value.task.id);
    if(value.type==="restore")await api.restoreTask(value.task.id);
    toast("操作已完成");
  };
  const resize=async(value:boolean)=>{
    const window=getCurrentWindow();if(value){await window.setSize(new LogicalSize(390,48));}else{await window.setSize(new LogicalSize(420,540));}
    setMini(value);
  };
  if(mini){
    const task=tasks[0];
    return <div className="floating-mini" data-tauri-drag-region><Grip size={14}/>{task?<button className="mini-main" onClick={()=>void copy(task)}><TicketNumber task={task}/><strong>{task.title}</strong>{task.isUrgent?<span className="mini-urgent">加急</span>:<StatusBadge status={task.status} overdue={isOverdue(task)}/>}</button>:<span className="mini-empty">目前没有排队事项</span>}
      <button className="float-icon" onClick={()=>void resize(false)} title="展开"><ExternalLink size={15}/></button><button className="float-icon" onClick={()=>void api.toggleFloating()} title="隐藏"><X size={15}/></button>{message&&<span className="float-toast">{message}</span>}</div>;
  }
  return <div className="floating-expanded">
    <header className="floating-header" data-tauri-drag-region><Grip size={16}/><img src="/inline-mark.svg"/><strong>In Line</strong><span>{tasks.length} 项</span>
      <button className="float-icon" onClick={()=>void api.showMain()} title="打开主界面"><ExternalLink size={15}/></button><button className="float-icon" onClick={()=>void resize(true)} title="迷你模式"><Minimize2 size={15}/></button><button className="float-icon" onClick={()=>void api.toggleFloating()} title="隐藏"><X size={15}/></button></header>
    <div className="floating-list">{tasks.slice(0,12).map((task,index)=><article key={task.id} className={task.isUrgent?"floating-card urgent": "floating-card"} onClick={()=>void copy(task)} onContextMenu={event=>{event.preventDefault();setMenu({task,x:event.clientX,y:event.clientY});}}>
      <TicketNumber task={task}/><div className="floating-copy"><strong>{task.title}</strong><span>{task.department} · {task.contact}</span></div>
      <div className="float-row-actions"><button onClick={event=>{event.stopPropagation();void copy(task);}} title="复制"><Copy size={15}/></button><button disabled={index===0} onClick={event=>void move(event,task,"up")} title="上移"><ArrowUp size={15}/></button><button disabled={index===tasks.length-1} onClick={event=>void move(event,task,"down")} title="下移"><ArrowDown size={15}/></button></div>
    </article>)}{!tasks.length&&<div className="floating-empty"><img src="/inline-mark.svg"/><p>目前没有排队事项</p></div>}</div>
    <footer className="floating-footer"><button onClick={()=>void api.requestNewTask()}><Plus size={15}/>新增取号</button><span>单击复制 · 右键管理</span></footer>{menu&&<TaskContextMenu {...menu} view="queue" onAction={value=>void action(value).catch(error=>toast(String(error)))} onClose={()=>setMenu(null)}/>} {message&&<div className="float-toast expanded">{message}</div>}
  </div>;
}
