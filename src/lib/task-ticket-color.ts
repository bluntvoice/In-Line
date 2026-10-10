import type { LegalTask } from "../types";
import { isOverdue } from "./task-utils";
import { normalizeHexColor } from "./ticket-colors";
export { ticketTextColor } from "./ticket-colors";

export function taskTicketColor(task: Pick<LegalTask,"requestedDeadline"|"status"|"priority"|"ticketColor">, now = new Date()): string | null {
  if (isOverdue(task, now) || task.priority === "critical") return "#C43D4B";
  return task.ticketColor ? normalizeHexColor(task.ticketColor) : null;
}
