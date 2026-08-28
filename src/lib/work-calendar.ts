import type { WorkCalendarResult,WorkCalendarTask } from "../types";

export type CalendarView="week"|"fortnight"|"month";
export type CalendarSort="firstEnqueued"|"latestHandled"|"rounds"|"taskType"|"permanentNumber";
export type WeekStart="monday"|"sunday";

export const startOfDay=(value:Date)=>new Date(value.getFullYear(),value.getMonth(),value.getDate());
export const addDays=(value:Date,days:number)=>new Date(value.getFullYear(),value.getMonth(),value.getDate()+days);
export const dateKey=(value:Date)=>`${value.getFullYear()}-${String(value.getMonth()+1).padStart(2,"0")}-${String(value.getDate()).padStart(2,"0")}`;
export const weekStart=(value:Date,mode:WeekStart)=>{
  const day=startOfDay(value);const weekday=day.getDay();const offset=mode==="sunday"?weekday:(weekday+6)%7;
  return addDays(day,-offset);
};
export const monthStart=(value:Date)=>new Date(value.getFullYear(),value.getMonth(),1);
export const calendarRange=(anchor:Date,view:CalendarView,mode:WeekStart)=>{
  const start=view==="month"?monthStart(anchor):weekStart(anchor,mode);
  const end=view==="month"?new Date(start.getFullYear(),start.getMonth()+1,1):addDays(start,view==="fortnight"?14:7);
  return{start,end,startIso:start.toISOString(),endIso:end.toISOString()};
};
export const shiftCalendar=(anchor:Date,view:CalendarView,direction:-1|1)=>view==="month"?new Date(anchor.getFullYear(),anchor.getMonth()+direction,1):addDays(anchor,(view==="fortnight"?14:7)*direction);

const overlaps=(start:number,end:number,day:Date)=>start<addDays(day,1).getTime()&&end>day.getTime();
export const visiblePeriodDays=(result:WorkCalendarResult,start:Date,length=7)=>{
  const generated=new Date(result.range.generatedAt).getTime();
  return Array.from({length},(_,index)=>addDays(start,index)).filter(day=>{
    const weekday=day.getDay();if(weekday!==0&&weekday!==6)return true;
    const eventOnDay=result.events.some(event=>dateKey(new Date(event.handledAt))===dateKey(day));
    const intervalOnDay=result.tasks.some(task=>task.intervals.some(interval=>overlaps(new Date(interval.enqueuedAt).getTime(),interval.closedAt?new Date(interval.closedAt).getTime():generated,day)));
    return eventOnDay||intervalOnDay;
  });
};

export const monthGrid=(anchor:Date,mode:WeekStart)=>{
  const first=monthStart(anchor);const gridStart=weekStart(first,mode);const last=new Date(first.getFullYear(),first.getMonth()+1,0);
  const minimumEnd=addDays(weekStart(last,mode),7);const length=Math.max(35,Math.round((minimumEnd.getTime()-gridStart.getTime())/86_400_000));
  return Array.from({length},(_,index)=>addDays(gridStart,index));
};

export const sortCalendarTasks=(tasks:WorkCalendarTask[],events:WorkCalendarResult["events"],sort:CalendarSort)=>[...tasks].sort((left,right)=>{
  const leftEvents=events.filter(event=>event.taskId===left.taskId);const rightEvents=events.filter(event=>event.taskId===right.taskId);
  if(sort==="rounds")return rightEvents.length-leftEvents.length||left.permanentNumber.localeCompare(right.permanentNumber,"zh-CN",{numeric:true});
  if(sort==="latestHandled")return Math.max(0,...rightEvents.map(event=>new Date(event.handledAt).getTime()))-Math.max(0,...leftEvents.map(event=>new Date(event.handledAt).getTime()));
  if(sort==="taskType")return left.taskType.localeCompare(right.taskType,"zh-CN")||left.permanentNumber.localeCompare(right.permanentNumber,"zh-CN",{numeric:true});
  if(sort==="permanentNumber")return left.permanentNumber.localeCompare(right.permanentNumber,"zh-CN",{numeric:true});
  return Math.min(...left.intervals.map(interval=>new Date(interval.enqueuedAt).getTime()))-Math.min(...right.intervals.map(interval=>new Date(interval.enqueuedAt).getTime()));
});

export const clipInterval=(startValue:string,endValue:string|null,rangeStart:Date,rangeEnd:Date,generatedAt:string)=>{
  const rangeStartMs=rangeStart.getTime(),rangeEndMs=rangeEnd.getTime(),duration=rangeEndMs-rangeStartMs;
  const rawStart=new Date(startValue).getTime(),rawEnd=new Date(endValue??generatedAt).getTime();
  const start=Math.max(rawStart,rangeStartMs),end=Math.min(rawEnd,rangeEndMs);
  return{left:Math.max(0,(start-rangeStartMs)/duration*100),width:Math.max(.35,(Math.max(start,end)-start)/duration*100),clippedStart:rawStart<rangeStartMs,clippedEnd:rawEnd>rangeEndMs};
};

export const formatCalendarDuration=(startValue:string,endValue:string)=>{
  const minutes=Math.max(0,Math.round((new Date(endValue).getTime()-new Date(startValue).getTime())/60_000));
  const days=Math.floor(minutes/1440),hours=Math.floor(minutes%1440/60),rest=minutes%60;
  return`${days?`${days}天`:""}${String(hours).padStart(2,"0")}小时${String(rest).padStart(2,"0")}分`;
};
