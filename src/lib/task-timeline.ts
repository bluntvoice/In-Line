import type { TaskLog, TaskStatus } from "../types";
import { isDeferredStatus, localizeStatusText } from "./task-utils";

type TimelineEntry = { log: TaskLog; records: TaskLog[]; text: string };

const automaticTypes = new Set([
  "created", "updated", "queue", "status", "urgent", "work", "archived", "deleted", "merged",
  "relation", "restored", "import_conflict", "scheduled_created", "scheduled_reactivated",
  "schedule_changed", "scheduled_activated", "scheduled_late", "scheduled_early",
  "scheduled_stopped", "queue_number_voided", "backup_number_remapped"
]);
const operationPattern = /^timeline:v[12]:[a-f0-9]{32}:[1-9][0-9]*$/;

export function timelineLogText(log: TaskLog) {
  return automaticTypes.has(log.logType) ? localizeStatusText(log.content) : log.content;
}

function operationText(records: TaskLog[]) {
  const find = (kind: string) => records.find(log => log.logType === kind)?.content;
  const queue = records.find(log => log.logType === "queue")?.content ?? "";
  const arrival = /^(重新加入|重新开启并加入) \d{4}-\d{2}-\d{2} 队列：(\d+)(?:（|$)/.exec(queue);

  if (find("restored") && arrival) return `已从回收站恢复，加入待办队列：${arrival[2]}`;
  const merged = find("merged");
  if (merged) return merged.replace(/，相关办理记录与历史记录已并入$/, "");
  const created = find("created");
  if (created) return [created, find("relation")].filter(Boolean).join("；");
  const work = find("work");
  if (work) return localizeStatusText(work);
  if (arrival) return `${arrival[1]}待办队列：${arrival[2]}`;
  const status = find("status")?.match(/^状态变更为：([a-z_]+)$/)?.[1];
  if (status) {
    const label = localizeStatusText(status);
    const moved = queue.startsWith("退出 ") && isDeferredStatus(status as TaskStatus);
    return moved ? `${label}，移入暂缓事项` : `状态改为${label}`;
  }
  const primary = records.find(log => ["archived", "deleted", "updated"].includes(log.logType));
  return timelineLogText(primary ?? records[0]);
}

export function taskTimelineEntries(logs: TaskLog[]): TimelineEntry[] {
  const entries: TimelineEntry[] = [];
  const operations = new Map<string, TaskLog[]>();
  for (const log of logs) {
    if (!log.operationId || !operationPattern.test(log.operationId) || !automaticTypes.has(log.logType)) {
      entries.push({ log, records: [], text: timelineLogText(log) });
      continue;
    }

    const key = `${log.taskId}:${log.operationId}`;
    const existing = operations.get(key);
    if (existing) {
      existing.push(log);
    } else {
      const records = [log];
      operations.set(key, records);
      entries.push({ log, records, text: "" });
    }
  }

  return entries.map(entry => {
    if (!entry.records.length) return entry;
    const text = operationText(entry.records);
    const expanded = entry.records.length > 1 || text !== timelineLogText(entry.log);
    return { ...entry, text, records: expanded ? entry.records : [] };
  });
}
