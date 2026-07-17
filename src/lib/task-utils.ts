import type { LegalTask,Priority,TaskStatus,Workload } from "../types";

export const STATUS_LABELS:Record<TaskStatus,string>={pending:"待处理",processing:"处理中",waiting_materials:"待补材料",waiting_confirmation:"待内部确认",paused:"已暂停",completed:"已完成",cancelled:"已取消",archived:"已归档"};
export const PRIORITY_LABELS:Record<Priority,string>={normal:"普通",elevated:"较急",urgent:"紧急",critical:"重大紧急"};
export const WORKLOAD_LABELS:Record<Workload,string>={simple:"简单",standard:"一般",complex:"复杂",major:"重大"};

export function dateOnly(value=new Date()){
  return `${value.getFullYear()}-${String(value.getMonth()+1).padStart(2,"0")}-${String(value.getDate()).padStart(2,"0")}`;
}
export function dayDifference(from:string,to=dateOnly()){
  const start=new Date(`${from}T00:00:00`);const end=new Date(`${to}T00:00:00`);
  return Math.max(0,Math.round((end.getTime()-start.getTime())/86_400_000));
}
export function alphaPrefix(days:number){
  if(days<=0)return"";let value=days,output="";
  while(value>0){value-=1;output=String.fromCharCode(65+(value%26))+output;value=Math.floor(value/26);}
  return output;
}
export function displayTicket(task:Pick<LegalTask,"ticketDate"|"dailySequence">,today?:string){
  return `${alphaPrefix(dayDifference(task.ticketDate,today))}${String(task.dailySequence).padStart(2,"0")}`;
}
export function isOverdue(task:Pick<LegalTask,"requestedDeadline"|"status">,now=new Date()){
  return Boolean(task.requestedDeadline&&!["completed","cancelled","archived"].includes(task.status)&&new Date(task.requestedDeadline).getTime()<now.getTime());
}
export function sortQueue(tasks:LegalTask[]){return [...tasks].sort((a,b)=>a.customSortOrder-b.customSortOrder||a.id-b.id);}
export function commonContacts(tasks:Pick<LegalTask,"contact">[],limit=3){
  const counts=new Map<string,{count:number;lastIndex:number}>();
  tasks.forEach((task,index)=>{
    const name=task.contact.trim();if(!name)return;
    const current=counts.get(name);counts.set(name,{count:(current?.count??0)+1,lastIndex:index});
  });
  return [...counts.entries()].sort((a,b)=>b[1].count-a[1].count||b[1].lastIndex-a[1].lastIndex||a[0].localeCompare(b[0],"zh-CN"))
    .slice(0,limit).map(([name])=>name);
}
export function formatDateTime(value:string|null){
  if(!value)return"未设置";
  return new Intl.DateTimeFormat("zh-CN",{month:"2-digit",day:"2-digit",hour:"2-digit",minute:"2-digit",hour12:false}).format(new Date(value));
}
export function toDateTimeLocalValue(value:string|null){
  if(!value)return"";
  const date=new Date(value);
  if(Number.isNaN(date.getTime()))return"";
  const pad=(part:number)=>String(part).padStart(2,"0");
  return `${date.getFullYear()}-${pad(date.getMonth()+1)}-${pad(date.getDate())}T${pad(date.getHours())}:${pad(date.getMinutes())}`;
}
export function fromDateTimeLocalValue(value:string){
  if(!value)return null;
  const date=new Date(value);
  return Number.isNaN(date.getTime())?null:date.toISOString();
}
