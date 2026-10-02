import type { LegalTask, TaskInput } from "../types";
import { dateOnly, displayTicket } from "./task-utils";

export function validatePlannedDate(input:TaskInput,original?:LegalTask|null,today=dateOnly()){
  const date=input.plannedDate??today;
  if(!/^\d{4}-\d{2}-\d{2}$/.test(date)||(!original||date!==original.plannedDate)&&date<today)return "加入日期只能选择今天及未来日期";
  const reactivating=Boolean(original&&(original.archivedAt||["completed","archived"].includes(original.status))&&date!==original.plannedDate);
  if(!reactivating&&input.requestedDeadline&&dateOnly(new Date(input.requestedDeadline))<date)return "截止时间不得早于加入日期，请修改或清空截止时间";
  return "";
}

export function scheduleConfirmation(task:LegalTask,input:TaskInput,today=dateOnly()){
  if(!input.plannedDate||input.plannedDate===(task.plannedDate??task.ticketDate))return null;
  const reactivate=Boolean(task.archivedAt||["completed","archived"].includes(task.status));
  return `加入日期：${task.plannedDate??task.ticketDate} → ${input.plannedDate}\n原队列编号：${task.ticketDate}-${displayTicket(task,today)}\n原编号将作废且永久不回收，并重新取得${input.plannedDate}的正式队列序号。\n${input.plannedDate>today?"将移出当前待办，转为未来事项。":"将进入今天待办队列。"}\n${reactivate?"将重新激活事项，原截止时间自动清空；历史完成、归档和统计保留。":"不涉及终态重新激活；截止时间不会自动清空。"}\n\n确认保存新计划？`;
}
