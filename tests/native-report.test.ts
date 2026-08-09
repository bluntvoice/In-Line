import {describe,expect,it} from "vitest";
import {buildNativeReport,nativeReportTitle} from "../src/lib/native-report";
import type {StatisticsResult} from "../src/types";

const data:StatisticsResult={
  range:{start:"2026-08-03T00:00:00+08:00",end:"2026-08-10T00:00:00+08:00"},
  summary:{handledTasks:32,processed:9,completed:18,waitingMaterials:2,waitingConfirmation:2,waitingCounterpartyConfirmation:1,rateMode:"processing",rateNumerator:32,rateDenominator:35,completionRate:32/35},
  byTaskType:[{taskType:"合同审核",handledTasks:14,completed:9,pendingFollowUp:5},{taskType:"咨询",handledTasks:8,completed:5,pendingFollowUp:3},{taskType:"函件",handledTasks:6,completed:3,pendingFollowUp:3},{taskType:"其他",handledTasks:4,completed:1,pendingFollowUp:3}],
  byDepartment:[],trend:[],trendGranularity:"day"
};

describe("native report",()=>{
  it("creates an overall weekly report with type and overdue summaries",()=>{
    const report=buildNativeReport({data,preset:"currentWeek",overdueCount:3});
    expect(report).toContain("本周共处理 32 项，其中完成 18 项、已处理待跟进 9 项、暂缓 5 项");
    expect(report).toContain("合同审核 14 项（完成 9 项、待跟进 5 项）");
    expect(report).toContain("截至当前仍有 3 项逾期");
    expect(report).toContain("六、下一步建议");
  });

  it("can narrow the preview to one task type",()=>{
    const details=[
      {taskId:1,permanentNumber:"A",title:"A",department:"一部",contact:"甲",resultStatus:"completed" as const,firstHandledAt:"",lastHandledAt:"",handlingCount:1},
      {taskId:2,permanentNumber:"B",title:"B",department:"一部",contact:"乙",resultStatus:"processed" as const,firstHandledAt:"",lastHandledAt:"",handlingCount:1},
      {taskId:3,permanentNumber:"C",title:"C",department:"一部",contact:"丙",resultStatus:"waiting_materials" as const,firstHandledAt:"",lastHandledAt:"",handlingCount:1}
    ];
    const report=buildNativeReport({data,preset:"month",overdueCount:1,taskType:"合同审核",details,displayRange:{start:"2026-07-01",end:"2026-07-31"}});
    expect(nativeReportTitle("month","合同审核")).toBe("月度工作报告 · 合同审核");
    expect(report).toContain("上一个月共处理 3 项“合同审核”类事项，其中完成 1 项、已处理待跟进 1 项、暂缓 1 项");
    expect(report).toContain("统计期间为 2026-07-01 至 2026-07-31");
    expect(report).toContain("本期共形成 3 次办理记录");
    expect(report).toContain("三、代表事项");
  });
});
