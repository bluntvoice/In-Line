import { useEffect,useMemo,useState } from "react";
import { CalendarDays,ChevronLeft,ChevronRight,Clock3,RotateCcw,X } from "lucide-react";
import { api } from "../api";
import type { WorkCalendarEvent,WorkCalendarResult,WorkResult } from "../types";
import { addDays,calendarRange,dateKey,formatCalendarDuration,monthGrid,shiftCalendar,sortCalendarTasks,startOfDay,visiblePeriodDays,type CalendarSort,type CalendarView,type WeekStart } from "../lib/work-calendar";
import { STATUS_LABELS } from "../lib/task-utils";

const WEEKDAYS=["周日","周一","周二","周三","周四","周五","周六"];
const resultLabel=(status:WorkResult)=>status==="completed"?"已完成":status==="processed"?"已处理":`已处理 · ${STATUS_LABELS[status]}`;
const dateTime=(value:string)=>new Intl.DateTimeFormat("zh-CN",{month:"2-digit",day:"2-digit",hour:"2-digit",minute:"2-digit",hour12:false}).format(new Date(value));
const timeOnly=(value:string)=>new Intl.DateTimeFormat("zh-CN",{hour:"2-digit",minute:"2-digit",hour12:false}).format(new Date(value));
const dayTitle=(value:Date)=>`${value.getFullYear()}年${value.getMonth()+1}月${value.getDate()}日`;

const position=(value:number,days:Date[])=>{
  if(!days.length)return 0;
  for(let index=0;index<days.length;index++){
    const start=days[index].getTime(),end=addDays(days[index],1).getTime();
    if(value<start)return index/days.length*100;
    if(value<end)return(index+(value-start)/(end-start))/days.length*100;
  }
  return 100;
};

export default function WorkCalendarPanel({weekStartsOn,refreshKey,onOpenTask,notify}:{weekStartsOn:WeekStart;refreshKey:string|number;onOpenTask:(id:number)=>void;notify:(message:string)=>void}){
  const [view,setView]=useState<CalendarView>("week");
  const [anchor,setAnchor]=useState(()=>new Date());
  const [sort,setSort]=useState<CalendarSort>("firstEnqueued");
  const [data,setData]=useState<WorkCalendarResult|null>(null);
  const [loading,setLoading]=useState(true);
  const [error,setError]=useState("");
  const [selectedDay,setSelectedDay]=useState<Date|null>(null);
  const range=useMemo(()=>calendarRange(anchor,view,weekStartsOn),[anchor,view,weekStartsOn]);

  useEffect(()=>{
    let alive=true;setLoading(true);setError("");
    void api.getWorkCalendar(range.startIso,range.endIso).then(result=>{if(alive)setData(result);}).catch(reason=>{if(alive){setError(String(reason));notify("工作日历载入失败："+String(reason));}}).finally(()=>{if(alive)setLoading(false);});
    return()=>{alive=false;};
  },[range.startIso,range.endIso,refreshKey]);

  const days=useMemo(()=>data&&view!=="month"?visiblePeriodDays(data,range.start,view==="fortnight"?14:7):[],[data,view,range.start]);
  const tasks=useMemo(()=>data?sortCalendarTasks(data.tasks.filter(task=>task.intervals.length),data.events,sort):[],[data,sort]);
  const cells=useMemo(()=>monthGrid(anchor,weekStartsOn),[anchor,weekStartsOn]);
  const eventsByDay=useMemo(()=>{
    const map=new Map<string,WorkCalendarEvent[]>();
    for(const event of data?.events??[]){const key=dateKey(new Date(event.handledAt));map.set(key,[...(map.get(key)??[]),event]);}
    return map;
  },[data]);
  const selectedEvents=selectedDay?(eventsByDay.get(dateKey(selectedDay))??[]):[];
  const rangeLabel=view!=="month"?`${range.start.getFullYear()}-${String(range.start.getMonth()+1).padStart(2,"0")}-${String(range.start.getDate()).padStart(2,"0")} 至 ${dateKey(addDays(range.end,-1))}`:`${range.start.getFullYear()}年${range.start.getMonth()+1}月`;
  const periodLabel=view==="week"?"本周":view==="fortnight"?"双周":"本月";
  const navUnit=view==="week"?"周":view==="fortnight"?"双周":"月";
  const dayMinWidth=view==="fortnight"?110:150;

  return <section className="work-calendar-panel">
    <header className="work-calendar-header">
      <div><p>按实际入队与办理记录还原工作轨迹</p><h1>工作日历</h1></div>
      <div className="calendar-controls">
        <div className="calendar-nav"><button title={`上一${navUnit}`} onClick={()=>setAnchor(current=>shiftCalendar(current,view,-1))}><ChevronLeft size={17}/></button><button onClick={()=>setAnchor(new Date())}><RotateCcw size={15}/>今天</button><button title={`下一${navUnit}`} onClick={()=>setAnchor(current=>shiftCalendar(current,view,1))}><ChevronRight size={17}/></button></div>
        <div className="calendar-view-tabs"><button className={view==="week"?"active":""} onClick={()=>{setView("week");setSelectedDay(null);}}>周</button><button className={view==="fortnight"?"active":""} onClick={()=>{setView("fortnight");setSelectedDay(null);}}>双周</button><button className={view==="month"?"active":""} onClick={()=>setView("month")}>月</button></div>
        <strong>{rangeLabel}</strong>
      </div>
    </header>
    {data&&<div className="calendar-summary"><span>{periodLabel}</span><b>{data.summary.handledTasks}</b> 个事项<i/>处理 <b>{data.summary.handlingRounds}</b> 轮<i/>完成 <b>{data.summary.completedTasks}</b> 项</div>}
    {loading?<div className="calendar-state"><CalendarDays size={32}/><span>正在整理实际工作轨迹…</span></div>:error?<div className="calendar-state error"><span>{error}</span><button className="button secondary" onClick={()=>setAnchor(current=>new Date(current))}>重新载入</button></div>:data&&view!=="month"?<>
      <div className="calendar-toolbar"><label>排序<select value={sort} onChange={event=>setSort(event.target.value as CalendarSort)}><option value="firstEnqueued">{periodLabel}首次进入队列时间</option><option value="latestHandled">最近处理时间</option><option value="rounds">{periodLabel}处理轮次（高到低）</option><option value="taskType">事项类型</option><option value="permanentNumber">永久编号</option></select></label><span><i className="legend-line"/>队列区间<i className="legend-dot handled"/>已处理<i className="legend-dot completed"/>已完成</span></div>
      <div className="calendar-week-scroll">
        <div className="calendar-week-grid" style={{minWidth:`${300+days.length*dayMinWidth}px`}}>
          <div className="calendar-week-corner">事项</div>
          <div className="calendar-day-header" style={{gridTemplateColumns:`repeat(${days.length},minmax(${dayMinWidth}px,1fr))`}}>{days.map(day=><div key={dateKey(day)}><strong>{WEEKDAYS[day.getDay()]}</strong><time>{String(day.getMonth()+1).padStart(2,"0")}-{String(day.getDate()).padStart(2,"0")}</time></div>)}</div>
          {tasks.map(task=><div className="calendar-week-row" key={task.taskId}>
            <button className="calendar-task-cell" onClick={()=>onOpenTask(task.taskId)} title={`${task.title}\n${task.taskType}`}><strong>{task.title}</strong><span>{task.taskType}</span></button>
            <div className="calendar-timeline-cell">{days.map(day=><i className="calendar-day-line" key={dateKey(day)} style={{left:`${days.indexOf(day)/days.length*100}%`,width:`${100/days.length}%`}}/>)}
              {task.intervals.map(interval=>{const rawStart=new Date(interval.enqueuedAt).getTime(),rawEnd=new Date(interval.closedAt??data.range.generatedAt).getTime();const left=position(rawStart,days),right=position(rawEnd,days);const title=`第 ${interval.roundIndex} 轮\n入队：${dateTime(interval.enqueuedAt)}\n${interval.closedAt?`本轮结束：${dateTime(interval.closedAt)}`:"当前仍在队列中"}${interval.resultStatus?`\n结果：${resultLabel(interval.resultStatus)}\n队列停留：${formatCalendarDuration(interval.enqueuedAt,interval.closedAt??data.range.generatedAt)}`:""}`;return <button key={interval.queueEntryId} className={`calendar-interval ${interval.currentActive?"active":""}`} style={{left:`${left}%`,width:`${Math.max(.6,right-left)}%`,minWidth:`min(44px, ${Math.max(0,100-left)}%)`}} title={title} onClick={()=>onOpenTask(task.taskId)}><span>第{interval.roundIndex}轮</span>{interval.resultStatus&&<i className={interval.resultStatus==="completed"?"completed":"handled"}/>}</button>;})}
            </div>
          </div>)}
          {!tasks.length&&<div className="calendar-empty-week"><Clock3 size={28}/><strong>{periodLabel}没有真实入队区间</strong><span>这里只展示实际“入队 → 本轮结束”的工作轨迹。</span></div>}
        </div>
      </div>
    </>:data?<div className="calendar-month-wrap">
      <div className="calendar-month"><div className="calendar-month-weekdays">{Array.from({length:7},(_,index)=>(weekStartsOn==="sunday"?index:(index+1)%7)).map(day=><strong key={day}>{WEEKDAYS[day]}</strong>)}</div><div className="calendar-month-grid">{cells.map(day=>{const outside=day.getMonth()!==anchor.getMonth();const dayEvents=eventsByDay.get(dateKey(day))??[];const handled=dayEvents.filter(event=>event.resultStatus!=="completed").length,completed=dayEvents.length-handled;const unique=[...new Map(dayEvents.map(event=>[event.taskId,event])).values()].slice(0,3);const intensity=Math.min(4,dayEvents.length);return <button key={dateKey(day)} disabled={outside} className={`calendar-month-day heat-${intensity} ${outside?"outside":""} ${dateKey(day)===dateKey(new Date())?"today":""} ${selectedDay&&dateKey(day)===dateKey(selectedDay)?"selected":""}`} onClick={()=>setSelectedDay(day)}><time>{day.getDate()}</time>{dayEvents.length>0&&<div className="calendar-day-counts"><span>处理 {handled}</span><b>完成 {completed}</b></div>}<div className="calendar-day-tasks">{unique.map(event=><span key={event.taskId}>{event.title}</span>)}{dayEvents.length>unique.length&&<em>+{dayEvents.length-unique.length}</em>}</div></button>})}</div></div>
      {selectedDay&&<aside className="calendar-day-drawer"><header><div><span>办理明细</span><h2>{dayTitle(selectedDay)}</h2></div><button className="icon-button" onClick={()=>setSelectedDay(null)}><X size={17}/></button></header><p>已处理 {selectedEvents.filter(event=>event.resultStatus!=="completed").length} · 已完成 {selectedEvents.filter(event=>event.resultStatus==="completed").length}</p><div>{selectedEvents.length?selectedEvents.map(event=><button key={event.eventId} onClick={()=>onOpenTask(event.taskId)}><strong>{event.title}</strong><span><time>{timeOnly(event.handledAt)}</time><b className={event.resultStatus==="completed"?"completed":"handled"}>{resultLabel(event.resultStatus)}</b>{event.roundIndex&&<em>第{event.roundIndex}轮</em>}</span></button>):<div className="calendar-drawer-empty">当日没有有效办理记录</div>}</div></aside>}
    </div>:null}
  </section>;
}
