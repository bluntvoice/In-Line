import { describe,it,expect } from "vitest";
import type { LegalTask,TaskInput } from "../types";
import { filterDeferredTasks,scheduleLabel,scheduleConfirmation,validatePlannedDate } from "./scheduling";
import { displayTicket,sortDeferredQueue,taskDetailView,visibleQueueTasks } from "./task-utils";
const task={plannedDate:"2026-10-05",ticketDate:"2026-10-05",dailySequence:3,status:"pending",hasActiveQueue:false,isScheduled:true} as LegalTask;
describe("加入日期校验与确认",()=>{
  it("未来事项优先按日期与正式序号排列，普通暂缓保持最近进入顺序",()=>{
    const values=[{...task,id:1,plannedDate:"2026-10-08"},{...task,id:2,dailySequence:4},{...task,id:3,dailySequence:2},{...task,id:4,isScheduled:false,status:"paused",updatedAt:"2026-10-02T08:00:00Z"},{...task,id:5,isScheduled:false,status:"paused",updatedAt:"2026-10-02T09:00:00Z"}] as LegalTask[];
    expect(sortDeferredQueue(values).map(task=>task.id)).toEqual([3,2,1,5,4]);
    expect(filterDeferredTasks(values,"scheduled")).toHaveLength(3);
    expect(filterDeferredTasks(values,"deferred")).toHaveLength(2);
    expect(filterDeferredTasks(values,"all")).toHaveLength(5);
    expect(taskDetailView(task)).toBe("deferred");expect(visibleQueueTasks([task])).toEqual([]);
    expect(displayTicket(task,"2026-10-02")).toBe("03");expect(scheduleLabel(task)).toContain("2026-10-05");
  });
  it("拒绝过去日期以及早于计划日期的截止时间",()=>{
    expect(validatePlannedDate({plannedDate:"2026-10-01"} as TaskInput,null,"2026-10-02")).toContain("今天");
    expect(validatePlannedDate({plannedDate:"2026-10-05",requestedDeadline:"2026-10-04T12:00:00+08:00"} as TaskInput,null,"2026-10-02")).toContain("截止时间");
    expect(validatePlannedDate({plannedDate:"2026-10-05",requestedDeadline:"2026-10-05T12:00:00+08:00"} as TaskInput,null,"2026-10-02")).toBe("");
  });
  it("日期不变不重复取号，修改必须解释旧号不回收和新状态",()=>{
    expect(scheduleConfirmation(task,{plannedDate:"2026-10-05"} as TaskInput,"2026-10-02")).toBeNull();
    const text=scheduleConfirmation(task,{plannedDate:"2026-10-08"} as TaskInput,"2026-10-02");
    expect(text).toContain("永久不回收");expect(text).toContain("2026-10-05");expect(text).toContain("2026-10-08");expect(text).toContain("移出当前待办");
  });
});
