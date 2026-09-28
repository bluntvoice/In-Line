export const TASK_TABLE_PAGES = ["queue", "deferred", "archive", "trash"] as const;
export type TaskTablePage = typeof TASK_TABLE_PAGES[number];

export const TASK_COLUMN_DEFINITIONS = [
  { id: "number", label: "号码", defaultWidth: 82, minWidth: 70, maxWidth: 180 },
  { id: "title", label: "事项标题", defaultWidth: 255, minWidth: 190, maxWidth: 680 },
  { id: "department", label: "部门 / 团队", defaultWidth: 143, minWidth: 100, maxWidth: 420 },
  { id: "contact", label: "对接人", defaultWidth: 112, minWidth: 90, maxWidth: 360 },
  { id: "taskType", label: "事项类型", defaultWidth: 122, minWidth: 96, maxWidth: 360 },
  { id: "status", label: "当前状态", defaultWidth: 105, minWidth: 92, maxWidth: 220 },
  { id: "deadline", label: "截止 / 完成时间", defaultWidth: 130, minWidth: 112, maxWidth: 320 },
  { id: "actions", label: "操作", defaultWidth: 116, minWidth: 104, maxWidth: 240 }
] as const;

export type TaskColumnId = typeof TASK_COLUMN_DEFINITIONS[number]["id"];
export type TaskColumnWidths = Record<TaskColumnId, number>;
export type TaskColumnLayouts = Record<TaskTablePage, TaskColumnWidths>;

const isRecord = (value: unknown): value is Record<string, unknown> => Boolean(value) && typeof value === "object" && !Array.isArray(value);

export function defaultTaskColumnWidths(): TaskColumnWidths {
  return Object.fromEntries(TASK_COLUMN_DEFINITIONS.map(column => [column.id, column.defaultWidth])) as TaskColumnWidths;
}

export function fitTaskColumnWidths(widths: TaskColumnWidths, availableWidth: number): TaskColumnWidths {
  const available = Math.floor(availableWidth);
  const total = TASK_COLUMN_DEFINITIONS.reduce((sum, column) => sum + widths[column.id], 0);
  if (!Number.isFinite(available) || available <= 0 || total <= available) return widths;
  const defaultTotal = TASK_COLUMN_DEFINITIONS.reduce((sum, column) => sum + column.defaultWidth, 0);
  // 只吸收默认布局附近的小幅溢出；显著加宽的自定义布局仍允许横向滚动。
  if (total > defaultTotal + 96) return widths;

  const minimum = TASK_COLUMN_DEFINITIONS.reduce((sum, column) => sum + column.minWidth, 0);
  if (available <= minimum) {
    return Object.fromEntries(TASK_COLUMN_DEFINITIONS.map(column => [column.id, column.minWidth])) as TaskColumnWidths;
  }

  const ratio = (available - minimum) / (total - minimum);
  const fitted = Object.fromEntries(TASK_COLUMN_DEFINITIONS.map(column => {
    const width = column.minWidth + Math.floor((widths[column.id] - column.minWidth) * ratio);
    return [column.id, width];
  })) as TaskColumnWidths;
  let remainder = available - TASK_COLUMN_DEFINITIONS.reduce((sum, column) => sum + fitted[column.id], 0);
  for (const column of TASK_COLUMN_DEFINITIONS) {
    if (remainder === 0) break;
    if (fitted[column.id] < widths[column.id]) {
      fitted[column.id] += 1;
      remainder -= 1;
    }
  }
  return fitted;
}

export function defaultTaskColumnLayouts(): TaskColumnLayouts {
  return Object.fromEntries(TASK_TABLE_PAGES.map(page => [page, defaultTaskColumnWidths()])) as TaskColumnLayouts;
}

export function normalizeTaskColumnWidth(columnId: TaskColumnId, value: unknown) {
  const definition = TASK_COLUMN_DEFINITIONS.find(column => column.id === columnId)!;
  return typeof value === "number" && Number.isFinite(value) && value >= definition.minWidth && value <= definition.maxWidth
    ? Math.round(value)
    : definition.defaultWidth;
}

export function normalizeTaskColumnLayouts(raw: string | unknown): TaskColumnLayouts {
  let parsed: unknown = raw;
  if (typeof raw === "string") {
    try { parsed = JSON.parse(raw); } catch { return defaultTaskColumnLayouts(); }
  }
  if (!isRecord(parsed)) return defaultTaskColumnLayouts();
  const pages = isRecord(parsed.pages) ? parsed.pages : parsed;
  return Object.fromEntries(TASK_TABLE_PAGES.map(page => {
    const stored = isRecord(pages[page]) ? pages[page] : {};
    const widths = Object.fromEntries(TASK_COLUMN_DEFINITIONS.map(column => [column.id, normalizeTaskColumnWidth(column.id, stored[column.id])])) as TaskColumnWidths;
    return [page, widths];
  })) as TaskColumnLayouts;
}

export function serializeTaskColumnLayouts(layouts: TaskColumnLayouts) {
  return JSON.stringify({ version: 1, pages: layouts });
}
