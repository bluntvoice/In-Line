export type WeekStart = "monday" | "sunday";
export const STATISTICS_PRESETS = {
  currentWeek: { label: "本周", comparison: "上周同期" },
  previousWeek: { label: "上周", comparison: "上上周" },
  currentMonth: { label: "本月", comparison: "上月同期" },
  month: { label: "上月", comparison: "再上一个月" },
  currentQuarter: { label: "本季度", comparison: "上季度同期" },
  quarter: { label: "上季度", comparison: "再上一季度" },
  custom: { label: "自定义", comparison: "前一等长周期" }
} as const;

export type StatisticsPreset = keyof typeof STATISTICS_PRESETS;
export type StatisticsSelection = {
  preset: StatisticsPreset;
  customRange?: { start: string; end: string };
};
export type StatisticsTrendPoint = { periodStart: string; handledTasks: number; processed?: number; completed?: number };

export function statisticsTrendRange(periodStart:string,granularity:"day"|"week",range:{start:string;end:string}){
  const end=parseDateInput(periodStart);
  end.setDate(end.getDate()+(granularity==="week"?6:0));
  return {start:periodStart<range.start?range.start:periodStart,end:dateInput(end)>range.end?range.end:dateInput(end)};
}

const WEEKDAY_LABELS = ["周日", "周一", "周二", "周三", "周四", "周五", "周六"] as const;

const dateInput = (date: Date) => `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;

function weekStartDate(now: Date, weekStartsOn: WeekStart) {
  const start = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const offset = weekStartsOn === "sunday" ? start.getDay() : (start.getDay() + 6) % 7;
  start.setDate(start.getDate() - offset);
  return start;
}

export function statisticsWeekdayLabel(value: string) {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value);
  if (!match) return "";
  const date = new Date(Number(match[1]), Number(match[2]) - 1, Number(match[3]));
  return WEEKDAY_LABELS[date.getDay()];
}

export function statisticsPresetRange(
  preset: Exclude<StatisticsPreset, "custom">,
  weekStartsOn: WeekStart,
  now = new Date()
) {
  if (preset === "currentWeek") {
    return { start: dateInput(weekStartDate(now, weekStartsOn)), end: dateInput(now) };
  }

  if (preset === "currentMonth" || preset === "currentQuarter") {
    const month = preset === "currentMonth" ? now.getMonth() : Math.floor(now.getMonth() / 3) * 3;
    return { start: dateInput(new Date(now.getFullYear(), month, 1)), end: dateInput(now) };
  }

  let start: Date;
  let end: Date;
  if (preset === "previousWeek") {
    end = weekStartDate(now, weekStartsOn);
    start = new Date(end);
    start.setDate(start.getDate() - 7);
  } else if (preset === "month") {
    end = new Date(now.getFullYear(), now.getMonth(), 1);
    start = new Date(now.getFullYear(), now.getMonth() - 1, 1);
  } else {
    const quarterStart = Math.floor(now.getMonth() / 3) * 3;
    end = new Date(now.getFullYear(), quarterStart, 1);
    start = new Date(now.getFullYear(), quarterStart - 3, 1);
  }
  const inclusiveEnd = new Date(end);
  inclusiveEnd.setDate(inclusiveEnd.getDate() - 1);
  return { start: dateInput(start), end: dateInput(inclusiveEnd) };
}

function parseDateInput(value: string) {
  const [year, month, day] = value.split("-").map(Number);
  return new Date(year, month - 1, day);
}

export function statisticsDisplayTrend(
  trend: StatisticsTrendPoint[],
  granularity: "day" | "week",
  range: { start: string; end: string },
  preset: StatisticsPreset
) {
  if (granularity !== "day") return trend;
  const start = parseDateInput(range.start);
  const rangeEnd = parseDateInput(range.end);
  if (Number.isNaN(start.getTime()) || Number.isNaN(rangeEnd.getTime()) || start > rangeEnd) return trend;

  const end = new Date(rangeEnd);
  if (preset === "currentWeek") {
    const firstMonday = new Date(start);
    firstMonday.setDate(firstMonday.getDate() + ((8 - firstMonday.getDay()) % 7));
    const friday = new Date(firstMonday);
    friday.setDate(friday.getDate() + 4);
    if (friday > end) end.setTime(friday.getTime());
  }

  const values = new Map(trend.map(point => [point.periodStart, point]));
  const display: StatisticsTrendPoint[] = [];
  for (const date = new Date(start); date <= end; date.setDate(date.getDate() + 1)) {
    const periodStart = dateInput(date);
    const point = values.get(periodStart) ?? { periodStart, handledTasks: 0 };
    const weekday = date.getDay();
    if ((weekday >= 1 && weekday <= 5) || point.handledTasks > 0) display.push(point);
  }
  return display;
}

export function statisticsComparisonRange(
  preset: StatisticsPreset,
  range: { start: string; end: string }
) {
  if (!range.start || !range.end || range.start > range.end) return range;
  const currentStart = parseDateInput(range.start);
  const currentEnd = parseDateInput(range.end);
  let comparisonStart: Date;

  if (preset === "currentWeek") {
    comparisonStart = new Date(currentStart);
    comparisonStart.setDate(comparisonStart.getDate() - 7);
    const comparisonEnd = new Date(currentEnd);
    comparisonEnd.setDate(comparisonEnd.getDate() - 7);
    return { start: dateInput(comparisonStart), end: dateInput(comparisonEnd) };
  }

  const comparisonEnd = new Date(currentStart);
  comparisonEnd.setDate(comparisonEnd.getDate() - 1);
  if (preset === "month" || preset === "currentMonth") {
    comparisonStart = new Date(currentStart.getFullYear(), currentStart.getMonth() - 1, 1);
  } else if (preset === "quarter" || preset === "currentQuarter") {
    comparisonStart = new Date(currentStart.getFullYear(), currentStart.getMonth() - 3, 1);
  } else {
    const inclusiveDays = Math.round((currentEnd.getTime() - currentStart.getTime()) / 86_400_000) + 1;
    comparisonStart = new Date(currentStart);
    comparisonStart.setDate(comparisonStart.getDate() - inclusiveDays);
  }

  if (preset === "currentMonth" || preset === "currentQuarter") {
    const dayOffset = Math.round((currentEnd.getTime() - currentStart.getTime()) / 86_400_000);
    const sameDay = new Date(comparisonStart);
    sameDay.setDate(sameDay.getDate() + dayOffset);
    if (sameDay < comparisonEnd) comparisonEnd.setTime(sameDay.getTime());
  }
  return { start: dateInput(comparisonStart), end: dateInput(comparisonEnd) };
}
