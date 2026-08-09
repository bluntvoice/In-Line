import { useEffect,useMemo,useState } from "react";
import { Archive,ArrowDown,ArrowUp,BarChart3,BookOpen,ClockAlert,Copy,Inbox,Info,PauseCircle,Plus,RotateCcw,Search,Settings,Trash2,X } from "lucide-react";
import { api } from "./api";
import type { BootstrapData,LegalTask,MasterData,TaskView } from "./types";
import { commonContacts,displayTicket,formatDateTime,formatDeadline,historyTimestamp,isDeferredStatus,isOverdue,sortDeferredQueue,taskDetailView,visibleQueueTasks } from "./lib/task-utils";
import StatusBadge from "./components/StatusBadge";
import TicketNumber from "./components/TicketNumber";
import TaskForm from "./components/TaskForm";
import TaskDetail from "./components/TaskDetail";
import TaskContextMenu,{type ContextAction} from "./components/TaskContextMenu";
import SettingsPanel from "./components/SettingsPanel";
import AboutPanel from "./components/AboutPanel";
import StatisticsPanel from "./components/StatisticsPanel";
import HelpPanel from "./components/HelpPanel";
import QueueDialog from "./components/QueueDialog";
import { DeadlineFilterHeader,ValueFilterHeader } from "./components/TaskTableFilter";
import { activeFilterCount,applyTaskFilters,EMPTY_TASK_FILTERS,uniqueValues,type TaskFilters } from "./lib/task-filters";
import { STATUS_LABELS } from "./lib/task-utils";

const emptyMasters:MasterData={departments:[],taskTypes:[],contacts:[]};
type MenuState={task:LegalTask;x:number;y:number}|null;
type PageView=TaskView|"deferred";
const newFilters=():TaskFilters=>({...EMPTY_TASK_FILTERS,deadlinePeriods:[]});

export default function App(){
  const [data,setData]=useState<BootstrapData|null>(null);
  const [view,setView]=useState<PageView>("queue");
  const [query,setQuery]=useState("");
  const [selected,setSelected]=useState<LegalTask|null>(null);
  const [editing,setEditing]=useState<LegalTask|null|undefined>(undefined);
  const [settings,setSettings]=useState(false);
  const [about,setAbout]=useState(false);
  const [statistics,setStatistics]=useState(false);
  const [help,setHelp]=useState(false);
  const [menu,setMenu]=useState<MenuState>(null);
  const [queueAction,setQueueAction]=useState<{task:LegalTask;reopen:boolean}|null>(null);
  const [message,setMessage]=useState("");
  const [startupError,setStartupError]=useState("");
  const [version,setVersion]=useState("");
  const [filtersByView,setFiltersByView]=useState<Record<PageView,TaskFilters>>({queue:newFilters(),deferred:newFilters(),archive:newFilters(),trash:newFilters()});
  const [selectedTrashIds,setSelectedTrashIds]=useState<number[]>([]);

  const toast=(text:string)=>{setMessage(text);window.setTimeout(()=>setMessage(""),2300);};
  const showTaskDetails=(task:LegalTask)=>{
    setSettings(false);
    setAbout(false);
    setStatistics(false);
    setHelp(false);
    setView(taskDetailView(task));
    setSelected(task);
  };
  const refresh=async()=>{
    setStartupError("");
    try{
      const next=await api.bootstrap();
      setData(next);
      setSelectedTrashIds(current=>current.filter(id=>next.trash.some(task=>task.id===id)));
      setSelected(current=>current?[...next.queue,...next.archive,...next.trash].find(value=>value.id===current.id)??null:null);
    }catch(error){
      const detail=error instanceof Error?error.message:String(error);
      setStartupError(detail||"初始化失败，请重试");
    }
  };
  useEffect(()=>{
    void refresh();
    const offData=api.onDataChanged(()=>void refresh());
    const offNew=api.onNewTask(()=>setEditing(null));
    const offTaskUi=api.onTaskUiAction(({id,action})=>{
      void api.getTask(id).then(task=>{
        if(action==="view")showTaskDetails(task);else setEditing(task);
      }).catch(error=>toast("无法打开事项："+String(error)));
    });
    void api.getVersion().then(setVersion).catch(()=>undefined);
    void api.globalShortcutAvailable().then(available=>{if(!available)toast("全局快捷键注册失败，请到软件设置中更换组合");}).catch(()=>undefined);
    return()=>{offData();offNew();offTaskUi();};
  },[]);

  const source=useMemo(()=>{
    if(!data)return[];
    if(view==="queue")return visibleQueueTasks(data.queue);
    if(view==="deferred")return sortDeferredQueue(data.queue.filter(task=>isDeferredStatus(task.status)));
    return data[view];
  },[data,view]);
  const filters=filtersByView[view];
  const updateFilters=(change:Partial<TaskFilters>)=>setFiltersByView(current=>({...current,[view]:{...current[view],...change}}));
  const filterOptions=useMemo(()=>({
    departments:uniqueValues(source.flatMap(task=>task.departments?.length?task.departments:[task.department])),
    contacts:uniqueValues(source.flatMap(task=>task.contacts?.length?task.contacts:[task.contact])),
    taskTypes:uniqueValues(source.map(task=>task.taskType)),
    statuses:[...new Set(source.map(task=>task.status))]
  }),[source]);
  const tasks=useMemo(()=>{
    const filtered=applyTaskFilters(source,filters);
    const key=query.trim().toLocaleLowerCase("zh-CN");if(!key)return filtered;
    return filtered.filter(task=>[task.permanentNumber,task.department,task.contact,task.taskType,task.title,task.details,task.internalNotes]
      .some(value=>value.toLocaleLowerCase("zh-CN").includes(key)));
  },[source,query,filters]);

  const copy=async(task:LegalTask)=>{try{await api.copyTicketImage(task);toast("已复制："+displayTicket(task));}catch(error){toast("复制失败："+String(error));}};
  const move=async(event:React.MouseEvent,task:LegalTask,direction:"up"|"down")=>{event.stopPropagation();try{await api.moveTask(task.id,direction);}catch(error){toast("调整失败："+String(error));}};
  const handleAction=async(action:ContextAction)=>{
    const {task,type}=action;
    if(type==="view"){setSelected(task);return;}
    if(type==="edit"||type==="status"){setEditing(task);return;}
    if(type==="urgent"){if(!task.isUrgent){setEditing(task);return;}await api.saveTask({...task,id:task.id,isUrgent:false,urgentRequester:"",urgentReason:""});}
    if(type==="process"){await api.processRound(task.id);toast("已记录本轮处理，事项已进入暂缓队列");return;}
    if(type==="complete"){await api.completeRound(task.id);toast("已记录本轮完成，事项整体结束");return;}
    if(type==="enqueue"){setQueueAction({task,reopen:false});return;}
    if(type==="reopen"){setQueueAction({task,reopen:true});return;}
    if(type==="archive")await api.archiveTask(task.id);
    if(type==="delete")await api.deleteTask(task.id);
    if(type==="restore")await api.restoreTask(task.id);
    if(type==="permanentDelete"){
      if(!window.confirm(`永久删除“${task.title}”？事项及其全部办理记录将不可恢复。`))return;
      await api.permanentlyDeleteTasks([task.id]);
      setSelectedTrashIds(current=>current.filter(id=>id!==task.id));
      toast("事项已永久删除");return;
    }
    toast("操作已完成");
  };
  const permanentlyDeleteSelected=async()=>{
    if(!selectedTrashIds.length)return;
    if(!window.confirm(`永久删除选中的 ${selectedTrashIds.length} 项？事项及其全部办理记录将不可恢复。`))return;
    const count=await api.permanentlyDeleteTasks(selectedTrashIds);setSelectedTrashIds([]);setSelected(null);toast(`已永久删除 ${count} 项`);
  };
  const emptyTrash=async()=>{
    if(!data?.trash.length)return;
    if(!window.confirm(`清空回收站中的 ${data.trash.length} 项？全部事项及办理记录将不可恢复。`))return;
    const count=await api.emptyTrash();setSelectedTrashIds([]);setSelected(null);toast(`回收站已清空，共永久删除 ${count} 项`);
  };
  const context=(task:LegalTask,x:number,y:number)=>setMenu({task,x,y});
  const contextKey=(event:React.KeyboardEvent,task:LegalTask)=>{
    if(event.shiftKey&&event.key==="F10"){event.preventDefault();const rect=event.currentTarget.getBoundingClientRect();context(task,rect.left+120,rect.top+32);}
    else if(event.key==="Enter"){event.preventDefault();setSelected(task);}
  };

  if(!data)return <div className="app-loading"><img src="/inline-mark.svg"/>{startupError?<section className="startup-error" role="alert"><h1>队列暂时无法载入</h1><p>{startupError}</p><div><button className="button primary" onClick={()=>void refresh()}>重新载入</button><button className="button secondary" onClick={()=>setEditing(null)}>直接新增取号</button></div><small>数据仍保存在本机，程序不会自动清空数据库。</small></section>:<p>正在整理队列…</p>}
    {editing!==undefined&&<TaskForm task={editing} masters={emptyMasters} commonContacts={[]} onClose={()=>setEditing(undefined)} onSaved={()=>{setEditing(undefined);void refresh();}}/>}
    {message&&<div className="toast">{message}</div>}
  </div>;
  const activeQueue=data.queue.filter(task=>task.hasActiveQueue);
  const urgent=activeQueue.filter(task=>task.isUrgent).length;
  const overdue=activeQueue.filter(task=>isOverdue(task)).length;
  const deferred=data.queue.filter(task=>isDeferredStatus(task.status));
  const queueCount=activeQueue.length;
  const deferredOverdue=deferred.filter(task=>isOverdue(task)).length;
  const frequentContacts=commonContacts([...data.queue,...data.archive].sort((a,b)=>a.updatedAt.localeCompare(b.updatedAt)));
  const actionView:TaskView=view==="deferred"?"queue":view;
  const openView=(next:PageView)=>{setSettings(false);setAbout(false);setStatistics(false);setHelp(false);setSelected(null);setView(next);};

  return <div className="app-shell">
    <aside className="sidebar">
      <div className="brand"><img src="/inline-mark.svg"/><div><strong>In Line</strong><span>排着呢</span></div></div>
      <button className="new-ticket" onClick={()=>setEditing(null)}><Plus size={18}/>新增取号<kbd>{data.settings.global_shortcut??"Alt+I"}</kbd></button>
      <nav>
        <button className={!settings&&!about&&!statistics&&!help&&view==="queue"?"active":""} onClick={()=>openView("queue")}><Inbox size={18}/><span>待办队列</span><b>{queueCount}</b></button>
        <button className={!settings&&!about&&!statistics&&!help&&view==="deferred"?"active":""} onClick={()=>openView("deferred")}><PauseCircle size={18}/><span>暂缓事项</span><span className="nav-counts"><b>{deferred.length}</b>{deferredOverdue>0&&<em title={`${deferredOverdue} 项已逾期`}><ClockAlert size={12}/>{deferredOverdue}</em>}</span></button>
        <button className={!settings&&!about&&!statistics&&!help&&view==="archive"?"active":""} onClick={()=>openView("archive")}><Archive size={18}/><span>历史归档</span><b>{data.archive.length}</b></button>
        <button className={!settings&&!about&&!statistics&&!help&&view==="trash"?"active":""} onClick={()=>openView("trash")}><Trash2 size={18}/><span>回收站</span><b>{data.trash.length}</b></button>
        <button className={statistics?"active":""} onClick={()=>{setStatistics(true);setHelp(false);setSettings(false);setAbout(false);setSelected(null);}}><BarChart3 size={18}/><span>统计中心</span></button>
        <button className={help?"active":""} onClick={()=>{setHelp(true);setStatistics(false);setSettings(false);setAbout(false);setSelected(null);}}><BookOpen size={18}/><span>使用说明</span></button>
      </nav>
      <div className="sidebar-summary"><div><span>加急</span><b>{urgent}</b></div><div><span>逾期</span><b>{overdue}</b></div></div>
      <button className={settings?"settings-button active":"settings-button"} onClick={()=>{setSettings(true);setAbout(false);setStatistics(false);setHelp(false);}}><Settings size={18}/>软件设置</button>
      <button className={about?"settings-button active":"settings-button"} onClick={()=>{setAbout(true);setSettings(false);setStatistics(false);setHelp(false);}}><Info size={18}/>关于</button>
      <small className="app-version">{version?`v${version}`:""}</small>
    </aside>
    <main className="workspace">
      {help?<HelpPanel/>:about?<AboutPanel version={version} onCopy={async value=>{try{await api.copyText(value);toast("GitHub 地址已复制");}catch(error){toast("复制失败："+String(error));}}}/>:settings?<SettingsPanel backups={data.backups} settings={data.settings} onChanged={()=>void refresh()} onOpenTask={id=>{void api.getTask(id).then(showTaskDetails).catch(error=>toast(String(error)));}} notify={toast}/>:statistics?<div className={selected?"queue-layout statistics-layout with-detail":"queue-layout statistics-layout"}><StatisticsPanel weekStartsOn={data.settings.week_start_day==="sunday"?"sunday":"monday"} refreshKey={[...data.queue,...data.archive].map(task=>task.updatedAt).join("|")} currentOverdueCount={data.queue.filter(task=>isOverdue(task)).length} currentOverdueByTaskType={Object.fromEntries([...new Set(data.queue.map(task=>task.taskType))].map(type=>[type,data.queue.filter(task=>task.taskType===type&&isOverdue(task)).length]))} notify={toast} onOpenTask={id=>{void api.getTask(id).then(setSelected).catch(error=>toast(String(error)));}}/>{selected&&<TaskDetail task={selected} view={selected.deletedAt?"trash":selected.archivedAt||["completed","cancelled","archived"].includes(selected.status)?"archive":"queue"} mergeCandidates={[...data.queue,...data.archive]} onClose={()=>setSelected(null)} onEdit={()=>setEditing(selected)} onChanged={()=>{setSelected(null);void refresh();}} notify={toast}/>}</div>:<>
        <header className="workspace-header"><div><p>通用事项取号与队列管理</p><h1>{view==="queue"?"待办队列":view==="deferred"?"暂缓事项":view==="archive"?"历史归档":"回收站"}</h1></div>
          <label className="search-box"><Search size={17}/><input value={query} onChange={event=>setQuery(event.target.value)} placeholder="搜索编号、对接人或事项关键词"/>{query&&<button onClick={()=>setQuery("")}><X size={15}/></button>}</label>
        </header>
        <div className={selected?"queue-layout with-detail":"queue-layout"}>
          <section className="table-panel"><div className={`table-meta ${view==="trash"?"trash-table-meta":""}`}><span>共 {tasks.length} 项{activeFilterCount(filters)>0&&<><b> · 已启用 {activeFilterCount(filters)} 项筛选</b><button type="button" onClick={()=>setFiltersByView(current=>({...current,[view]:newFilters()}))}>清除筛选</button></>}</span>{view==="trash"?<div className="trash-bulk-actions"><button type="button" className="button secondary small" disabled={!tasks.length} onClick={()=>setSelectedTrashIds(tasks.every(task=>selectedTrashIds.includes(task.id))?[]:tasks.map(task=>task.id))}>{tasks.length>0&&tasks.every(task=>selectedTrashIds.includes(task.id))?"取消全选":"全选当前列表"}</button><button type="button" className="button secondary small danger" disabled={!selectedTrashIds.length} onClick={()=>void permanentlyDeleteSelected().catch(error=>toast("批量删除失败："+String(error)))}><Trash2 size={14}/>永久删除选中项（{selectedTrashIds.length}）</button><button type="button" className="button secondary small danger" disabled={!data.trash.length} onClick={()=>void emptyTrash().catch(error=>toast("清空回收站失败："+String(error)))}>清空回收站</button></div>:<span>单击查看详情 · 复制按钮生成图片 · 右键管理事项</span>}</div>
            <div className="table-scroll"><table className="task-table"><thead><tr><th>号码</th><th>事项标题</th>
              <th><ValueFilterHeader label="部门 / 团队" values={filterOptions.departments} selected={filters.departments} onChange={departments=>updateFilters({departments})}/></th>
              <th><ValueFilterHeader label="对接人" values={filterOptions.contacts} selected={filters.contacts} onChange={contacts=>updateFilters({contacts})}/></th>
              <th><ValueFilterHeader label="事项类型" values={filterOptions.taskTypes} selected={filters.taskTypes} onChange={taskTypes=>updateFilters({taskTypes})}/></th>
              <th><ValueFilterHeader label="当前状态" values={filterOptions.statuses} selected={filters.statuses} renderLabel={status=>STATUS_LABELS[status]} onChange={statuses=>updateFilters({statuses})}/></th>
              <th>{view==="archive"?"完成时间":<DeadlineFilterHeader date={filters.deadlineDate} periods={filters.deadlinePeriods} onChange={(deadlineDate,deadlinePeriods)=>updateFilters({deadlineDate,deadlinePeriods})}/>}</th><th>操作</th></tr></thead>
              <tbody>{tasks.map((task,index)=>{const taskOverdue=isOverdue(task);const canMoveUp=view==="queue"&&task.hasActiveQueue&&index>0&&tasks[index-1].hasActiveQueue&&isOverdue(tasks[index-1])===taskOverdue;const canMoveDown=view==="queue"&&task.hasActiveQueue&&index<tasks.length-1&&tasks[index+1].hasActiveQueue&&isOverdue(tasks[index+1])===taskOverdue;return <tr key={task.id} className={taskOverdue?"overdue-row":undefined} tabIndex={0} onClick={()=>setSelected(task)} onContextMenu={event=>{event.preventDefault();context(task,event.clientX,event.clientY);}} onKeyDown={event=>contextKey(event,task)}>
                <td><span className="ticket-cell">{view==="trash"&&<input type="checkbox" checked={selectedTrashIds.includes(task.id)} aria-label={`选择 ${task.title}`} onClick={event=>event.stopPropagation()} onChange={event=>setSelectedTrashIds(current=>event.target.checked?[...current,task.id]:current.filter(id=>id!==task.id))}/>}<TicketNumber task={task}/></span></td><td><strong>{task.title}</strong>{task.isUrgent&&<span className="urgent-mark">加急</span>}{task.isImportConflict&&<span className="conflict-mark">导入冲突</span>}</td>
                <td>{task.department}</td><td>{task.contact}</td><td>{task.taskType}</td><td><StatusBadge status={task.status} overdue={taskOverdue}/></td>
                <td className={taskOverdue?"deadline overdue":"deadline"}>{view==="archive"?formatDateTime(historyTimestamp(task)):formatDeadline(task.requestedDeadline,task.requestedDeadlineLabel)}</td>
                <td><div className="row-actions">{view==="trash"?<><button onClick={event=>{event.stopPropagation();void api.restoreTask(task.id).then(()=>toast("事项已恢复并加入今日队列")).catch(error=>toast(String(error)));}} title="恢复"><RotateCcw size={17}/></button><button className="danger" onClick={event=>{event.stopPropagation();void handleAction({type:"permanentDelete",task}).catch(error=>toast(String(error)));}} title="永久删除"><Trash2 size={17}/></button></>:<><button onClick={event=>{event.stopPropagation();void copy(task);}} title="复制取号图片"><Copy size={17}/></button><button disabled={!canMoveUp} onClick={event=>void move(event,task,"up")} title="上移"><ArrowUp size={17}/></button><button disabled={!canMoveDown} onClick={event=>void move(event,task,"down")} title="下移"><ArrowDown size={17}/></button></>}</div></td>
              </tr>})}</tbody></table>
              {!tasks.length&&<div className="empty-state"><img src="/inline-mark.svg"/><h2>{query||activeFilterCount(filters)>0?"没有匹配事项":view==="deferred"?"目前没有暂缓事项":view==="trash"?"回收站为空":"目前没有排队事项"}</h2><p>{query||activeFilterCount(filters)>0?"请调整关键词或列筛选条件。":view==="deferred"?"待补充材料、待内部确认、待对方确认和已暂停事项会显示在这里。":view==="trash"?"移入回收站的事项会显示在这里，可恢复或永久删除。":"新增事项后，系统会自动生成今日号码。"}</p></div>}
            </div>
          </section>
          {selected&&<TaskDetail task={selected} view={actionView} mergeCandidates={[...data.queue,...data.archive]} onClose={()=>setSelected(null)} onEdit={()=>setEditing(selected)} onChanged={()=>{setSelected(null);void refresh();}} notify={toast}/>}
        </div>
      </>}
    </main>
    {editing!==undefined&&<TaskForm task={editing} masters={data.masters??emptyMasters} commonContacts={frequentContacts} onClose={()=>setEditing(undefined)} onSaved={()=>{setEditing(undefined);void refresh();}}/>}
    {menu&&<TaskContextMenu {...menu} view={actionView} onAction={action=>void handleAction(action).catch(error=>toast(String(error)))} onClose={()=>setMenu(null)}/>}
    {queueAction&&<QueueDialog task={queueAction.task} reopen={queueAction.reopen} onClose={()=>setQueueAction(null)} onSaved={()=>{toast(queueAction.reopen?"事项已重新开启并加入今日队列":"事项已加入今日队列");setQueueAction(null);void refresh();}}/>}
    {message&&<div className="toast">{message}</div>}
  </div>;
}
