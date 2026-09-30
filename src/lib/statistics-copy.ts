import type {StatisticsDetail} from "../types";

const collator=new Intl.Collator("zh-CN",{numeric:true,sensitivity:"base"});
const singleLine=(value:string)=>value.replace(/[\r\n]+/g," ");

export function buildStatisticsDetailsCopy(details:StatisticsDetail[]){
  const items=details.filter(item=>item.hasProcessedOrCompleted).map(item=>({
    taskId:item.taskId,taskType:singleLine(item.taskType),department:singleLine(item.department),title:singleLine(item.title)
  })).sort((a,b)=>collator.compare(a.taskType,b.taskType)||collator.compare(a.department,b.department)||collator.compare(a.title,b.title)||a.taskId-b.taskId);
  return{count:items.length,text:items.map(item=>`${item.taskType}-${item.department}-${item.title}\n`).join("")};
}
