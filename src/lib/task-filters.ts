import type { LegalTask, TaskStatus } from "../types";
import { dateOnly } from "./task-utils";

export type DeadlinePeriod = "morning" | "noon" | "afternoon" | "evening";
export type ValueSelection<T extends string = string> = T[] | null;
export type TaskStructureFilter = "all" | "top-level" | "subtask";

export interface TaskFilters {
  departments: ValueSelection;
  contacts: ValueSelection;
  taskTypes: ValueSelection;
  statuses: ValueSelection<TaskStatus>;
  structure: TaskStructureFilter;
  deadlineDate: string;
  deadlinePeriods: DeadlinePeriod[];
}

export const EMPTY_TASK_FILTERS: TaskFilters = {
  departments: null,
  contacts: null,
  taskTypes: null,
  statuses: null,
  structure: "all",
  deadlineDate: "",
  deadlinePeriods: []
};

export const DEADLINE_PERIOD_LABELS: Record<DeadlinePeriod, string> = {
  morning: "上午",
  noon: "中午",
  afternoon: "下午",
  evening: "晚上"
};

export function deadlinePeriod(value: string): DeadlinePeriod | null {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return null;
  const hour = date.getHours();
  if (hour >= 5 && hour < 11) return "morning";
  if (hour >= 11 && hour < 14) return "noon";
  if (hour >= 14 && hour < 18) return "afternoon";
  return "evening";
}

function matchesSelection<T extends string>(value: T, selected: ValueSelection<T>) {
  return selected === null || selected.includes(value);
}

export function applyTaskFilters(tasks: LegalTask[], filters: TaskFilters) {
  return tasks.filter((task) => {
    if (filters.departments !== null && !(task.departments?.length ? task.departments : [task.department]).some(department => filters.departments?.includes(department))) return false;
    if (filters.contacts !== null && !(task.contacts?.length ? task.contacts : [task.contact]).some(contact => filters.contacts?.includes(contact))) return false;
    if (!matchesSelection(task.taskType, filters.taskTypes)) return false;
    if (!matchesSelection(task.status, filters.statuses)) return false;
    if (filters.structure === "top-level" && task.parentTaskId !== null) return false;
    if (filters.structure === "subtask" && task.parentTaskId === null) return false;

    const hasDeadlineFilter = Boolean(filters.deadlineDate || filters.deadlinePeriods.length);
    if (!hasDeadlineFilter) return true;
    if (!task.requestedDeadline) return false;
    const deadline = new Date(task.requestedDeadline);
    if (Number.isNaN(deadline.getTime())) return false;
    if (filters.deadlineDate && dateOnly(deadline) !== filters.deadlineDate) return false;
    const period = deadlinePeriod(task.requestedDeadline);
    return !filters.deadlinePeriods.length || Boolean(period && filters.deadlinePeriods.includes(period));
  });
}

export function activeFilterCount(filters: TaskFilters) {
  return [
    filters.departments !== null,
    filters.contacts !== null,
    filters.taskTypes !== null,
    filters.statuses !== null,
    filters.structure !== "all",
    Boolean(filters.deadlineDate || filters.deadlinePeriods.length)
  ].filter(Boolean).length;
}

export function uniqueValues(values: string[]) {
  return [...new Set(values.map((value) => value.trim()).filter(Boolean))].sort((a, b) => a.localeCompare(b, "zh-CN"));
}

export function matchesTaskSearch(task: LegalTask, query: string, parentTitle?: string) {
  const key = query.trim().toLocaleLowerCase("zh-CN");
  if (!key) return true;
  return [task.permanentNumber, task.department, task.contact, task.taskType, task.title, task.details, task.internalNotes, task.plannedDate??task.ticketDate,task.isScheduled?"未来事项":"", parentTitle ?? ""]
    .some(value => value.toLocaleLowerCase("zh-CN").includes(key));
}
