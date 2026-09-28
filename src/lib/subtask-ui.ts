import type { CreateSubtaskInput, LegalTask } from "../types";

const inheritedValues = (values: string[], fallback: string) => values?.length ? values : [fallback].filter(Boolean);

export function createSubtaskDraft(parent: Pick<LegalTask,"id"|"taskType"|"departments"|"department"|"contacts"|"contact">): CreateSubtaskInput {
  return {
    parentTaskId: parent.id,
    title: "",
    details: "",
    taskType: parent.taskType,
    departments: inheritedValues(parent.departments, parent.department),
    contacts: inheritedValues(parent.contacts, parent.contact),
    priority: "normal",
    workload: "standard",
    isUrgent: false,
    urgentRequester: "",
    urgentReason: "",
    requestedDeadline: null,
    requestedDeadlineLabel: null,
    internalNotes: "",
    enqueueToday: true
  };
}

export function reorderById<T extends { id: number }>(items: T[], draggedId: number, targetId: number): T[] {
  if (draggedId === targetId) return items;
  const from = items.findIndex(item => item.id === draggedId);
  const to = items.findIndex(item => item.id === targetId);
  if (from < 0 || to < 0) return items;
  const next = [...items];
  const [dragged] = next.splice(from, 1);
  next.splice(to, 0, dragged);
  return next;
}

export function moveById<T extends { id: number }>(items: T[], id: number, direction: "up" | "down"): T[] {
  const from = items.findIndex(item => item.id === id);
  const to = direction === "up" ? from - 1 : from + 1;
  if (from < 0 || to < 0 || to >= items.length) return items;
  const next = [...items];
  [next[from], next[to]] = [next[to], next[from]];
  return next;
}
