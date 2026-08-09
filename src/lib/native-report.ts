import type { StatisticsDetail,StatisticsResult } from "../types";
import type { StatisticsPreset } from "./statistics-range";

export interface NativeReportInput {
  data: StatisticsResult;
  preset: StatisticsPreset;
  overdueCount: number;
  taskType?: string;
  details?: StatisticsDetail[];
  displayRange?: {start:string;end:string};
}

const periodNames:Record<StatisticsPreset,{title:string;subject:string}>={
  currentWeek:{title:"本周工作报告",subject:"本周"},
  previousWeek:{title:"上一周工作报告",subject:"上一周"},
  month:{title:"月度工作报告",subject:"上一个月"},
  quarter:{title:"季度工作报告",subject:"上一季度"},
  custom:{title:"自定义周期工作报告",subject:"本统计周期"}
};

export function nativeReportTitle(preset:StatisticsPreset,taskType=""){
  const title=periodNames[preset].title;
  return taskType?`${title} · ${taskType}`:title;
}

export function buildNativeReport({data,preset,overdueCount,taskType="",details=[],displayRange}:NativeReportInput){
  const period=periodNames[preset];
  const selected=taskType?details:null;
  const handled=selected?.length??data.summary.handledTasks;
  const completed=selected?.filter(item=>item.resultStatus==="completed").length??data.summary.completed;
  const processed=selected?.filter(item=>item.resultStatus==="processed").length??data.summary.processed;
  const deferred=selected?.filter(item=>item.resultStatus.startsWith("waiting_")).length??(
    data.summary.waitingMaterials+data.summary.waitingConfirmation+data.summary.waitingCounterpartyConfirmation
  );
  const scope=taskType?`“${taskType}”类事项`:"";
  const start=displayRange?.start??data.range.start.slice(0,10);
  const end=displayRange?.end??data.range.end.slice(0,10);
  const rateLabel=selected?"事项完成占比":data.summary.rateMode==="processing"?"有效处理率":"事项办结率";
  const rate=selected?(handled?Math.round(completed/handled*100):0):Math.round(data.summary.completionRate*100);
  const paragraphs=[
    `一、总体情况\n统计期间为 ${start} 至 ${end}。${period.subject}共处理 ${handled} 项${scope}，其中完成 ${completed} 项、已处理待跟进 ${processed} 项、暂缓 ${deferred} 项；${rateLabel}为 ${rate}%。`
  ];

  if(taskType){
    const rounds=details.reduce((sum,item)=>sum+item.handlingCount,0);
    const departmentCounts=new Map<string,number>();
    details.forEach(item=>{const department=item.department.trim()||"未填写部门";departmentCounts.set(department,(departmentCounts.get(department)??0)+1);});
    const departmentSummary=[...departmentCounts].sort((a,b)=>b[1]-a[1]||a[0].localeCompare(b[0],"zh-CN")).slice(0,5).map(([name,count])=>`${name} ${count} 项`).join("、");
    paragraphs.push(`二、办理情况\n本期共形成 ${rounds} 次办理记录，平均每项 ${handled?(rounds/handled).toFixed(1):"0.0"} 次。${departmentSummary?`相关部门 / 团队主要为${departmentSummary}。`:"暂无部门 / 团队分布数据。"}`);
    const representative=[...details].sort((a,b)=>b.lastHandledAt.localeCompare(a.lastHandledAt)).slice(0,5).map((item,index)=>`${index+1}. ${item.title}（${item.resultStatus==="completed"?"已完成":"待继续跟进"}，办理 ${item.handlingCount} 次）`).join("\n");
    paragraphs.push(`三、代表事项\n${representative||"本期暂无可列示的事项明细。"}`);
  }else if(data.byTaskType.length){
    const top=data.byTaskType.slice(0,5).map(item=>`${item.taskType} ${item.handledTasks} 项（完成 ${item.completed} 项、待跟进 ${item.pendingFollowUp} 项）`).join("；");
    paragraphs.push(`二、事项类型分布\n主要事项类型为：${top}${data.byTaskType.length>5?`；另有 ${data.byTaskType.length-5} 类事项未逐项列示`:""}。`);
    if(data.byDepartment.length){
      const departments=data.byDepartment.slice(0,5).map(item=>`${item.department} ${item.handledTasks} 项（完成 ${item.completed} 项、待跟进 ${item.pendingFollowUp} 项）`).join("；");
      paragraphs.push(`三、部门 / 团队分布\n${departments}${data.byDepartment.length>5?`；另有 ${data.byDepartment.length-5} 个部门 / 团队未逐项列示`:""}。同一事项涉及多个部门时会分别计入。`);
    }else paragraphs.push("三、部门 / 团队分布\n本期暂无可统计的部门 / 团队数据。");
  }else{
    paragraphs.push("二、事项类型分布\n本期暂无可统计的事项类型。");
    paragraphs.push("三、部门 / 团队分布\n本期暂无可统计的部门 / 团队数据。");
  }

  if(data.trend.length){
    const peak=data.trend.reduce((best,item)=>item.handledTasks>best.handledTasks?item:best,data.trend[0]);
    const middle=Math.ceil(data.trend.length/2),first=data.trend.slice(0,middle).reduce((sum,item)=>sum+item.handledTasks,0),second=data.trend.slice(middle).reduce((sum,item)=>sum+item.handledTasks,0);
    const direction=second>first?"后半段办理量较前半段增加":second<first?"后半段办理量较前半段减少":"前后半段办理量基本持平";
    paragraphs.push(`四、工作节奏\n${taskType?"同期全部事项的":"本期"}工作量高点出现在 ${peak.periodStart}，处理 ${peak.handledTasks} 项；${direction}。趋势按${data.trendGranularity==="day"?"日":"自然周"}去重统计${taskType?"，作为所选类型报告的整体工作背景":""}。`);
  }else paragraphs.push("四、工作节奏\n本期暂无可用于分析工作节奏的趋势数据。");

  paragraphs.push(overdueCount>0?`五、风险与待跟进\n截至当前仍有 ${overdueCount} 项逾期，建议优先核对截止时间、责任人和下一步动作；同时持续跟进 ${processed+deferred} 项尚未完成事项。`:`五、风险与待跟进\n截至当前没有逾期事项；仍建议定期复核 ${processed+deferred} 项待跟进或暂缓事项，及时补充最新办理结果。`);
  const leadingType=taskType||data.byTaskType.slice().sort((a,b)=>b.pendingFollowUp-a.pendingFollowUp)[0]?.taskType;
  paragraphs.push(`六、下一步建议\n${overdueCount>0?"优先处理逾期事项，并逐项确认新的完成节点；":"保持当前事项的截止时间和状态更新；"}${leadingType?`重点关注“${leadingType}”类事项的后续推进；`:"持续关注各类事项的后续推进；"}每轮办理后及时记录处理结果，确保后续统计和报告准确。`);
  return paragraphs.join("\n\n");
}
