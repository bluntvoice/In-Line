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

// 78px 编号底色，加上单元格左右各 10px 留白。
export const MIN_TASK_NUMBER_COLUMN_WIDTH = 98;

const isRecord = (value: unknown): value is Record<string, unknown> => Boolean(value) && typeof value === "object" && !Array.isArray(value);

export function defaultTaskColumnWidths(): TaskColumnWidths {
  return Object.fromEntries(TASK_COLUMN_DEFINITIONS.map(column => [column.id, column.defaultWidth])) as TaskColumnWidths;
}

export function fitTaskColumnWidths(widths: TaskColumnWidths, availableWidth: number, numberMinimum = 0): TaskColumnWidths {
  const minimumFor = (column: typeof TASK_COLUMN_DEFINITIONS[number]) => column.id === "number" && Number.isFinite(numberMinimum)
    ? Math.max(column.minWidth, Math.ceil(numberMinimum)) : column.minWidth;
  // 仅对显示宽度作内容保护，不能回写或压缩用户保存的其他列宽。
  const rendered = widths.number < minimumFor(TASK_COLUMN_DEFINITIONS[0])
    ? { ...widths, number: minimumFor(TASK_COLUMN_DEFINITIONS[0]) } : widths;
  const available = Math.floor(availableWidth);
  const total = TASK_COLUMN_DEFINITIONS.reduce((sum, column) => sum + rendered[column.id], 0);
  if (!Number.isFinite(available) || available <= 0 || total <= available) return rendered;
  // 仅默认布局自适应；即使手动保存的布局只超出几像素，也不能偷偷缩窄。
  if (TASK_COLUMN_DEFINITIONS.some(column => widths[column.id] !== column.defaultWidth)) return rendered;

  const minimum = TASK_COLUMN_DEFINITIONS.reduce((sum, column) => sum + minimumFor(column), 0);
  if (available <= minimum) {
    return Object.fromEntries(TASK_COLUMN_DEFINITIONS.map(column => [column.id, minimumFor(column)])) as TaskColumnWidths;
  }

  const ratio = (available - minimum) / (total - minimum);
  const fitted = Object.fromEntries(TASK_COLUMN_DEFINITIONS.map(column => {
    const min = minimumFor(column);
    const width = min + Math.floor((rendered[column.id] - min) * ratio);
    return [column.id, width];
  })) as TaskColumnWidths;
  let remainder = available - TASK_COLUMN_DEFINITIONS.reduce((sum, column) => sum + fitted[column.id], 0);
  for (const column of TASK_COLUMN_DEFINITIONS) {
    if (remainder === 0) break;
    if (fitted[column.id] < rendered[column.id]) {
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
