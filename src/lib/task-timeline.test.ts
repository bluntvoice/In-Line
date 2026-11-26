import { describe, expect, it } from "vitest";
import type { TaskLog } from "../types";
import { taskTimelineEntries } from "./task-timeline";

const makeLog = (id: number, logType: string, content: string, operationId?: string, taskId = 1): TaskLog => ({
  id, taskId, logType, content,
  operationId: operationId && `timeline:v2:${Array.from(operationId, c => c.charCodeAt(0).toString(16)).join("").padStart(32, "0")}:1`,
  createdAt: "2026-11-24T08:00:00+08:00"
});

describe("task timeline", () => {
  it("groups only matching operations, preserving notes, audit and same-time legacy rows", () => {
    const logs = [
      makeLog(10, "work", "本轮已处理，移入暂缓事项", "operation-2"),
      makeLog(9, "note", "保留 pending 原文", "operation-2"),
      makeLog(8, "work", "本轮已处理，移入暂缓事项", "operation-1"),
      makeLog(7, "queue", "退出 2026-11-24 队列：11", "operation-2"),
      makeLog(6, "queue", "退出 2026-11-24 队列：10", "operation-1"),
      makeLog(5, "audit", "作废一条结构化处理活动", "operation-1"),
      makeLog(4, "status", "状态变更为：processed"),
      makeLog(3, "queue", "退出 2026-11-24 队列：09"),
      makeLog(2, "work", "本轮已完成，事项整体结束", "operation-1", 2)
    ];
    const rows = taskTimelineEntries(logs);
    expect(rows.map(row => row.log.id)).toEqual([10, 9, 8, 5, 4, 3, 2]);
    expect(rows[0].records.map(log => log.id)).toEqual([10, 7]);
    expect(rows[1].text).toBe("保留 pending 原文");
    expect(rows[2].records.map(log => log.id)).toEqual([8, 6]);
    expect(rows[4].text).toBe("状态变更为：已处理");
    expect(rows[5].text).toBe(logs[7].content);
  });

  it("uses the queue number and distinguishes requeue, reopen and restore", () => {
    const queued = makeLog(8, "queue", "重新加入 2026-11-24 队列：10（继续办理）", "requeue");
    const opened = makeLog(7, "queue", "重新开启并加入 2026-11-24 队列：09", "reopen");
    const restored = makeLog(6, "restored", "事项已恢复并加入今日队列", "restore");
    const restoredQueue = makeLog(5, "queue", "重新加入 2026-11-24 队列：08（从回收站恢复）", "restore");
    const rows = taskTimelineEntries([queued, opened, restored, restoredQueue]);
    expect(rows.map(row => row.text)).toEqual([
      "重新加入待办队列：10", "重新开启并加入待办队列：09", "已从回收站恢复，加入待办队列：08"
    ]);
    expect(rows[0].records).toEqual([queued]);
    expect(rows[2].records).toEqual([restored, restoredQueue]);
  });

  it("selects the actual business outcome for grouped status, completion and subtask creation", () => {
    const rows = taskTimelineEntries([
      makeLog(7, "queue", "退出 2026-11-24 队列：11", "waiting"),
      makeLog(6, "status", "状态变更为：waiting_materials", "waiting"),
      makeLog(5, "work", "本轮已完成，事项整体结束", "completed"),
      makeLog(4, "queue", "退出 2026-11-24 队列：10", "completed"),
      makeLog(3, "relation", "设置所属任务：甲", "child"),
      makeLog(2, "created", "创建子任务并取号：20261124-10", "child")
    ]);
    expect(rows.map(row => row.text)).toEqual([
      "待补材料，移入暂缓事项", "本轮已完成，事项整体结束", "创建子任务并取号：20261124-10；设置所属任务：甲"
    ]);
  });

  it("keeps unknown and malformed rows independent and supports projected v1 groups", () => {
    const old = "timeline:v1:0123456789abcdef0123456789abcdef:91";
    const rows = taskTimelineEntries([
      {...makeLog(5, "work", "办理"), operationId: old},
      {...makeLog(4, "future_unknown", "pending"), operationId: old},
      {...makeLog(3, "queue", "退出队列"), operationId: old},
      {...makeLog(2, "queue", "原始历史"), operationId: "timeline:v2:bad:1"},
    ]);
    expect(rows.map(row => row.log.id)).toEqual([5, 4, 2]);
    expect(rows[0].records.map(log => log.id)).toEqual([5, 3]);
    expect(rows[1].text).toBe("pending");
    expect(rows[2].records).toEqual([]);
  });
});
