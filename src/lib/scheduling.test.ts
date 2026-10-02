import { describe,it,expect } from "vitest";
import type { LegalTask,TaskInput } from "../types";
import { scheduleConfirmation,validatePlannedDate } from "./scheduling";
const task={plannedDate:"2026-10-05",ticketDate:"2026-10-05",dailySequence:3,status:"pending",hasActiveQueue:false,isScheduled:true} as LegalTask;
describe("加入日期校验与确认",()=>{
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
