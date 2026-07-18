import { describe,expect,it } from "vitest";
import { alphaPrefix,commonContacts,dayDifference,deadlineShortcut,displayTicket,formatDeadline,fromDateTimeLocalValue,queueAheadMessage,sortQueue,toDateTimeLocalValue } from "../src/lib/task-utils";
import { activeFilterCount,applyTaskFilters,deadlinePeriod,EMPTY_TASK_FILTERS,type TaskFilters } from "../src/lib/task-filters";
import type { LegalTask } from "../src/types";

const task=(id:number,order:number):LegalTask=>({id,customSortOrder:order,permanentNumber:`20260717-${String(id).padStart(2,"0")}`,dailySequence:id,ticketDate:"2026-07-17",department:"产品组",contact:"小林",taskType:"任务处理",title:"测试事项",details:"测试",status:"pending",priority:"normal",workload:"standard",isUrgent:false,urgentRequester:"",urgentReason:"",requestedDeadline:null,requestedDeadlineLabel:null,internalNotes:"",createdAt:"2026-07-17T00:00:00Z",updatedAt:"2026-07-17T00:00:00Z",startedAt:null,completedAt:null,archivedAt:null,deletedAt:null});

describe("取号和人工顺位",()=>{
  it("当天显示两位号码",()=>expect(displayTicket({ticketDate:"2026-07-17",dailySequence:1},"2026-07-17")).toBe("01"));
  it("跨天增加字母前缀",()=>{expect(displayTicket({ticketDate:"2026-07-16",dailySequence:3},"2026-07-17")).toBe("A03");expect(alphaPrefix(27)).toBe("AA");});
  it("系统时间倒退不产生负天数",()=>expect(dayDifference("2026-07-17","2026-07-16")).toBe(0));
  it("加急不覆盖人工顺位",()=>{const first={...task(1,2),isUrgent:true,priority:"critical" as const};const second=task(2,1);expect(sortQueue([first,second]).map(value=>value.id)).toEqual([2,1]);});
});

describe("本地截止时间",()=>{
  it("保存为 UTC 后再次编辑仍显示用户选择的本地时间",()=>{
    const selected="2026-07-17T15:30";
    expect(toDateTimeLocalValue(fromDateTimeLocalValue(selected))).toBe(selected);
  });
  it("未设置时间时保持空值",()=>{
    expect(fromDateTimeLocalValue("")).toBeNull();
    expect(toDateTimeLocalValue(null)).toBe("");
  });
  it("模糊时间保留用户可读标签",()=>{
    const morning=deadlineShortcut("morning",new Date(2026,6,17,10,0));
    expect(morning.label).toBe("今天上午");
    expect(formatDeadline(morning.value,morning.label)).toBe("今天上午");
  });
  it("当天时间已过时自动顺延到明天",()=>{
    expect(deadlineShortcut("morning",new Date(2026,6,17,12,0)).label).toBe("明天上午");
  });
});

describe("常用对接人",()=>{
  it("按使用频次选出三个联系人，同频时优先最近出现的人",()=>{
    const contacts=["小林","小周","小林","小陈","小周","小吴"].map(contact=>({contact}));
    expect(commonContacts(contacts)).toEqual(["小周","小林","小吴"]);
  });
});

describe("队列表头筛选",()=>{
  it("按部门、对接人、类型和状态组合筛选",()=>{
    const tasks=[
      {...task(1,1),department:"产品组",contact:"小林",taskType:"合同审查",status:"pending" as const},
      {...task(2,2),department:"行政组",contact:"小周",taskType:"采购申请",status:"completed" as const}
    ];
    const filters:TaskFilters={...EMPTY_TASK_FILTERS,departments:["产品组"],contacts:["小林"],taskTypes:["合同审查"],statuses:["pending"],deadlinePeriods:[]};
    expect(applyTaskFilters(tasks,filters).map(value=>value.id)).toEqual([1]);
    expect(activeFilterCount(filters)).toBe(4);
  });
  it("截止时间支持日期与上午中午下午晚上时段",()=>{
    const morning=new Date(2026,6,18,9,30).toISOString();
    const evening=new Date(2026,6,18,20,0).toISOString();
    const tasks=[{...task(1,1),requestedDeadline:morning},{...task(2,2),requestedDeadline:evening}];
    const filters:TaskFilters={...EMPTY_TASK_FILTERS,deadlineDate:"2026-07-18",deadlinePeriods:["morning"]};
    expect(deadlinePeriod(morning)).toBe("morning");
    expect(deadlinePeriod(evening)).toBe("evening");
    expect(applyTaskFilters(tasks,filters).map(value=>value.id)).toEqual([1]);
  });
});

describe("分享图排队提示",()=>{
  it("右下角只显示前方事项数而不是队列总数",()=>{
    expect(queueAheadMessage(0)).toBe("前面还有0个事项待处理，请耐心等待");
    expect(queueAheadMessage(2)).toBe("前面还有2个事项待处理，请耐心等待");
  });
});
