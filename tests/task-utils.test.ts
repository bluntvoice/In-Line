import { describe,expect,it } from "vitest";
import { alphaPrefix,commonContacts,dayDifference,deadlineShortcut,displayTicket,formatDeadline,fromDateTimeLocalValue,historyTimestamp,isDeferredStatus,localizeStatusText,queueAheadMessage,sortQueue,toDateTimeLocalValue,visibleQueueTasks } from "../src/lib/task-utils";
import { activeFilterCount,applyTaskFilters,deadlinePeriod,EMPTY_TASK_FILTERS,type TaskFilters } from "../src/lib/task-filters";
import { fitTextLines,ticketRenderKey } from "../src/lib/ticket-image";
import { statisticsPresetRange } from "../src/lib/statistics-range";
import type { LegalTask } from "../src/types";

const task=(id:number,order:number):LegalTask=>({id,customSortOrder:order,permanentNumber:`20260717-${String(id).padStart(2,"0")}`,dailySequence:id,ticketDate:"2026-07-17",department:"产品组",departments:["产品组"],contact:"小林",contacts:["小林"],taskType:"任务处理",title:"测试事项",details:"测试",status:"pending",priority:"normal",workload:"standard",isUrgent:false,urgentRequester:"",urgentReason:"",requestedDeadline:null,requestedDeadlineLabel:null,internalNotes:"",createdAt:"2026-07-17T00:00:00Z",updatedAt:"2026-07-17T00:00:00Z",startedAt:null,completedAt:null,archivedAt:null,deletedAt:null,processingRounds:0,hasActiveQueue:true});

describe("取号和人工顺位",()=>{
  it("当天显示两位号码",()=>expect(displayTicket({ticketDate:"2026-07-17",dailySequence:1},"2026-07-17")).toBe("01"));
  it("跨天增加字母前缀",()=>{expect(displayTicket({ticketDate:"2026-07-16",dailySequence:3},"2026-07-17")).toBe("A03");expect(alphaPrefix(27)).toBe("AA");});
  it("归档后固定为归档当天的排队编号",()=>{
    const archived={ticketDate:"2026-07-17",dailySequence:3,archivedAt:"2026-07-19T10:00:00"};
    expect(displayTicket(archived,"2026-08-04")).toBe("B03");
    expect(displayTicket(archived,"2027-08-04")).toBe("B03");
  });
  it("旧版归档数据缺少归档时间时按完成时间固定编号",()=>{
    const archived={ticketDate:"2026-07-17",dailySequence:4,status:"completed" as const,archivedAt:null,completedAt:"2026-07-20T10:00:00",updatedAt:"2026-08-04T10:00:00"};
    expect(historyTimestamp(archived)).toBe("2026-07-20T10:00:00");
    expect(displayTicket(archived,"2027-08-04")).toBe("C04");
  });
  it("系统时间倒退不产生负天数",()=>expect(dayDifference("2026-07-17","2026-07-16")).toBe(0));
  it("加急不覆盖人工顺位",()=>{const first={...task(1,2),isUrgent:true,priority:"critical" as const};const second=task(2,1);expect(sortQueue([first,second]).map(value=>value.id)).toEqual([2,1]);});
  it("已逾期事项优先显示，同组内仍保持人工顺位",()=>{
    const now=new Date("2026-07-18T08:00:00.000Z");
    const regular=task(1,1);
    const overdue={...task(2,2),status:"waiting_materials" as const,requestedDeadline:"2026-07-17T08:00:00.000Z"};
    const overdueLater={...task(3,3),status:"paused" as const,requestedDeadline:"2026-07-17T09:00:00.000Z"};
    expect(sortQueue([regular,overdueLater,overdue],now).map(value=>value.id)).toEqual([2,3,1]);
  });
  it("暂缓事项包含待补材料、待内部确认、待对方确认和已暂停",()=>{
    expect(["waiting_materials","waiting_confirmation","waiting_counterparty_confirmation","paused","processed"].every(status=>isDeferredStatus(status as LegalTask["status"]))).toBe(true);
    expect(isDeferredStatus("processing")).toBe(false);
  });
  it("待办队列默认隐藏暂缓事项，并允许通过设置重新显示",()=>{
    const regular=task(1,1);
    const deferred={...task(2,2),status:"waiting_counterparty_confirmation" as const};
    expect(visibleQueueTasks([regular,deferred],false).map(value=>value.id)).toEqual([1]);
    expect(visibleQueueTasks([regular,deferred],true).map(value=>value.id)).toEqual([1,2]);
  });
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
    const contacts=["小林","小周","小林","小陈","小周","小吴"].map(contact=>({contact,contacts:[contact]}));
    expect(commonContacts(contacts)).toEqual(["小周","小林","小吴"]);
  });
});

describe("队列表头筛选",()=>{
  it("按部门、对接人、类型和状态组合筛选",()=>{
    const tasks=[
      {...task(1,1),department:"产品组",contact:"小林",taskType:"合同审查",status:"pending" as const},
      {...task(2,2),department:"行政组",contact:"小周",contacts:["小周"],taskType:"采购申请",status:"completed" as const}
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

describe("分享图标题排版",()=>{
  const measure=(value:string)=>value.length;
  it("会继续填满第二行，而不是第二行首字后立即省略",()=>{
    expect(fitTextLines("一二三四五六七八九十",5,2,measure)).toEqual({lines:["一二三四五","六七八九十"],truncated:false});
  });
  it("只有确实放不下时才在末尾添加省略号",()=>{
    expect(fitTextLines("一二三四五六七八九十一",5,2,measure)).toEqual({lines:["一二三四五","六七八九…"],truncated:true});
  });
});

describe("时间线状态中文化",()=>{
  it("将历史记录中的内部状态码替换为中文",()=>{
    expect(localizeStatusText("记录本次处理：completed")).toBe("记录本次处理：已完成");
    expect(localizeStatusText("状态变更为：cancelled")).toBe("状态变更为：已取消");
  });
});

describe("分享图缓存",()=>{
  it("相同内容复用缓存，队列位置或可见字段变化时自动失效",()=>{
    const original=task(1,1);
    expect(ticketRenderKey(original,2)).toBe(ticketRenderKey({...original},2));
    expect(ticketRenderKey(original,2)).not.toBe(ticketRenderKey(original,1));
    expect(ticketRenderKey(original,2)).not.toBe(ticketRenderKey({...original,title:"更新后的事项"},2));
  });
});

describe("统计周期",()=>{
  const wednesday=new Date(2026,7,5,12,0,0);
  it("本周默认从周一起算并截止今天",()=>{
    expect(statisticsPresetRange("currentWeek","monday",wednesday)).toEqual({start:"2026-08-03",end:"2026-08-05"});
  });
  it("允许改为周日起算",()=>{
    expect(statisticsPresetRange("currentWeek","sunday",wednesday)).toEqual({start:"2026-08-02",end:"2026-08-05"});
  });
  it("上一周跟随系统设置并取完整七天",()=>{
    expect(statisticsPresetRange("previousWeek","monday",wednesday)).toEqual({start:"2026-07-27",end:"2026-08-02"});
    expect(statisticsPresetRange("previousWeek","sunday",wednesday)).toEqual({start:"2026-07-26",end:"2026-08-01"});
  });
});
