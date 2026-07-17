import { useEffect,useMemo,useState } from "react";
import { Archive,ArrowDown,ArrowUp,Copy,Inbox,Plus,Search,Settings,Trash2,X } from "lucide-react";
import { api } from "./api";
import type { BootstrapData,LegalTask,MasterData,TaskView } from "./types";
import { displayTicket,formatDateTime,isOverdue } from "./lib/task-utils";
import StatusBadge from "./components/StatusBadge";
import TicketNumber from "./components/TicketNumber";
import TaskForm from "./components/TaskForm";
import TaskDetail from "./components/TaskDetail";
import TaskContextMenu,{type ContextAction} from "./components/TaskContextMenu";
import SettingsPanel from "./components/SettingsPanel";

const emptyMasters:MasterData={departments:[],taskTypes:[]};
type MenuState={task:LegalTask;x:number;y:number}|null;

export default function App(){
  const [data,setData]=useState<BootstrapData|null>(null);
  const [view,setView]=useState<TaskView>("queue");
  const [query,setQuery]=useState("");
  const [selected,setSelected]=useState<LegalTask|null>(null);
  const [editing,setEditing]=useState<LegalTask|null|undefined>(undefined);
  const [settings,setSettings]=useState(false);
  const [menu,setMenu]=useState<MenuState>(null);
  const [message,setMessage]=useState("");

  const toast=(text:string)=>{setMessage(text);window.setTimeout(()=>setMessage(""),2300);};
  const refresh=async()=>{const next=await api.bootstrap();setData(next);setSelected(current=>current?[...next.queue,...next.archive,...next.trash].find(value=>value.id===current.id)??null:null);};
  useEffect(()=>{
    void refresh();
    const offData=api.onDataChanged(()=>void refresh());
    const offNew=api.onNewTask(()=>setEditing(null));
    let offShortcut=()=>{};
    void api.registerNewTaskShortcut(()=>{void api.showMain();setEditing(null);}).then(value=>{offShortcut=value;}).catch(()=>toast("全局快捷键注册失败，可继续使用新增按钮"));
    return()=>{offData();offNew();offShortcut();};
  },[]);

  const source=data?.[view]??[];
  const tasks=useMemo(()=>{
    const key=query.trim().toLocaleLowerCase("zh-CN");if(!key)return source;
    return source.filter(task=>[task.permanentNumber,task.department,task.contact,task.taskType,task.title,task.details,task.internalNotes]
      .some(value=>value.toLocaleLowerCase("zh-CN").includes(key)));
  },[source,query]);

  const copy=async(task:LegalTask)=>{try{await api.copyTicketImage(task);toast("已复制："+displayTicket(task));}catch(error){toast("复制失败："+String(error));}};
  const move=async(event:React.MouseEvent,task:LegalTask,direction:"up"|"down")=>{event.stopPropagation();try{await api.moveTask(task.id,direction);}catch(error){toast("调整失败："+String(error));}};
  const handleAction=async(action:ContextAction)=>{
    const {task,type}=action;
    if(type==="view"){setSelected(task);return;}
    if(type==="edit"||type==="status"){setEditing(task);return;}
    if(type==="urgent"){if(!task.isUrgent){setEditing(task);return;}await api.saveTask({...task,id:task.id,isUrgent:false,urgentRequester:"",urgentReason:""});}
    if(type==="complete")await api.setTaskStatus(task.id,"completed");
    if(type==="archive")await api.archiveTask(task.id);
    if(type==="delete")await api.deleteTask(task.id);
    if(type==="restore")await api.restoreTask(task.id);
    toast("操作已完成");
  };
  const context=(task:LegalTask,x:number,y:number)=>setMenu({task,x,y});
  const contextKey=(event:React.KeyboardEvent,task:LegalTask)=>{
    if(event.shiftKey&&event.key==="F10"){event.preventDefault();const rect=event.currentTarget.getBoundingClientRect();context(task,rect.left+120,rect.top+32);}
  };

  if(!data)return <div className="app-loading"><img src="/inline-mark.svg"/><p>正在整理队列…</p></div>;
  const urgent=data.queue.filter(task=>task.isUrgent).length;
  const overdue=data.queue.filter(task=>isOverdue(task)).length;

  return <div className="app-shell">
    <aside className="sidebar">
      <div className="brand"><img src="/inline-mark.svg"/><div><strong>In Line</strong><span>排着呢</span></div></div>
      <button className="new-ticket" onClick={()=>setEditing(null)}><Plus size={18}/>新增取号<kbd>Ctrl Alt N</kbd></button>
      <nav>
        <button className={!settings&&view==="queue"?"active":""} onClick={()=>{setSettings(false);setView("queue");}}><Inbox size={18}/><span>待办队列</span><b>{data.queue.length}</b></button>
        <button className={!settings&&view==="archive"?"active":""} onClick={()=>{setSettings(false);setView("archive");}}><Archive size={18}/><span>历史归档</span><b>{data.archive.length}</b></button>
        <button className={!settings&&view==="trash"?"active":""} onClick={()=>{setSettings(false);setView("trash");}}><Trash2 size={18}/><span>回收站</span><b>{data.trash.length}</b></button>
      </nav>
      <div className="sidebar-summary"><div><span>加急</span><b>{urgent}</b></div><div><span>逾期</span><b>{overdue}</b></div></div>
      <button className={settings?"settings-button active":"settings-button"} onClick={()=>setSettings(true)}><Settings size={18}/>系统设置</button>
    </aside>
    <main className="workspace">
      {settings?<SettingsPanel backups={data.backups} onChanged={()=>void refresh()} notify={toast}/>:<>
        <header className="workspace-header"><div><p>通用事项取号与队列管理</p><h1>{view==="queue"?"待办队列":view==="archive"?"历史归档":"回收站"}</h1></div>
          <label className="search-box"><Search size={17}/><input value={query} onChange={event=>setQuery(event.target.value)} placeholder="搜索编号、对接人或事项关键词"/>{query&&<button onClick={()=>setQuery("")}><X size={15}/></button>}</label>
        </header>
        <div className={selected?"queue-layout with-detail":"queue-layout"}>
          <section className="table-panel"><div className="table-meta"><span>共 {tasks.length} 项</span><span>单击复制取号图片 · 右键管理事项</span></div>
            <div className="table-scroll"><table className="task-table"><thead><tr><th>号码</th><th>事项标题</th><th>部门 / 团队</th><th>对接人</th><th>类型</th><th>状态</th><th>截止时间</th><th>操作</th></tr></thead>
              <tbody>{tasks.map((task,index)=><tr key={task.id} tabIndex={0} onClick={()=>void copy(task)} onDoubleClick={event=>{event.preventDefault();setSelected(task);}} onContextMenu={event=>{event.preventDefault();context(task,event.clientX,event.clientY);}} onKeyDown={event=>contextKey(event,task)}>
                <td><TicketNumber task={task}/></td><td><strong>{task.title}</strong>{task.isUrgent&&<span className="urgent-mark">加急</span>}</td>
                <td>{task.department}</td><td>{task.contact}</td><td>{task.taskType}</td><td><StatusBadge status={task.status} overdue={isOverdue(task)}/></td>
                <td className={isOverdue(task)?"deadline overdue":"deadline"}>{formatDateTime(task.requestedDeadline)}</td>
                <td><div className="row-actions"><button onClick={event=>{event.stopPropagation();void copy(task);}} title="复制取号图片"><Copy size={17}/></button>
                  <button disabled={view!=="queue"||index===0} onClick={event=>void move(event,task,"up")} title="上移"><ArrowUp size={17}/></button>
                  <button disabled={view!=="queue"||index===tasks.length-1} onClick={event=>void move(event,task,"down")} title="下移"><ArrowDown size={17}/></button></div></td>
              </tr>)}</tbody></table>
              {!tasks.length&&<div className="empty-state"><img src="/inline-mark.svg"/><h2>{query?"没有匹配事项":"目前没有排队事项"}</h2><p>{query?"请尝试其他关键词。":"新增事项后，系统会自动生成今日号码。"}</p></div>}
            </div>
          </section>
          {selected&&<TaskDetail task={selected} view={view} onClose={()=>setSelected(null)} onEdit={()=>setEditing(selected)} onChanged={()=>{setSelected(null);void refresh();}}/>}
        </div>
      </>}
    </main>
    {editing!==undefined&&<TaskForm task={editing} masters={data.masters??emptyMasters} recentContacts={data.queue.map(task=>task.contact)} onClose={()=>setEditing(undefined)} onSaved={()=>{setEditing(undefined);void refresh();}}/>}
    {menu&&<TaskContextMenu {...menu} view={view} onAction={action=>void handleAction(action).catch(error=>toast(String(error)))} onClose={()=>setMenu(null)}/>}
    {message&&<div className="toast">{message}</div>}
  </div>;
}
