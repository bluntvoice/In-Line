export type WeekStart = "monday" | "sunday";
export const STATISTICS_PRESETS = [
  { value: "currentWeek", label: "本周", comparison: "上周同期" },
  { value: "previousWeek", label: "上周", comparison: "上上周" },
  { value: "currentMonth", label: "本月", comparison: "上月同期" },
  { value: "month", label: "上月", comparison: "再上一个月" },
  { value: "currentQuarter", label: "本季度", comparison: "上季度同期" },
  { value: "quarter", label: "上季度", comparison: "再上一季度" },
  { value: "custom", label: "自定义", comparison: "前一等长周期" }
] as const;
export type StatisticsPreset = typeof STATISTICS_PRESETS[number]["value"];
type DateRange = { start: string; end: string };
export interface StatisticsPeriodSelection { preset: StatisticsPreset; customStart: string; customEnd: string }
export type StatisticsTrendPoint = { periodStart: string; handledTasks: number; processed?: number; completed?: number };

export function statisticsTrendRange(periodStart:string,granularity:"day"|"week",range:{start:string;end:string}){
  const end=parseDateInput(periodStart);
  end.setDate(end.getDate()+(granularity==="week"?6:0));
  return {start:periodStart<range.start?range.start:periodStart,end:dateInput(end)>range.end?range.end:dateInput(end)};
}

const WEEKDAY_LABELS = ["周日", "周一", "周二", "周三", "周四", "周五", "周六"] as const;

const dateInput = (date: Date) => `${String(date.getFullYear()).padStart(4, "0")}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;

function weekStartDate(now: Date, weekStartsOn: WeekStart) {
  const start = calendarDate(now.getFullYear(), now.getMonth(), now.getDate());
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

export function statisticsTrendLabel(periodStart:string,granularity:"day"|"week",range:{start:string;end:string}){
  if(granularity==="day")return {primary:statisticsWeekdayLabel(periodStart),secondary:periodStart.slice(5),title:`${periodStart}（${statisticsWeekdayLabel(periodStart)}）`};
  const bucket=statisticsTrendRange(periodStart,granularity,range);
  return {primary:bucket.start.slice(5),secondary:`至 ${bucket.end.slice(5)}`,title:`${bucket.start} 至 ${bucket.end}`};
}

export function statisticsPresetRange(
  preset: Exclude<StatisticsPreset, "custom">,
  weekStartsOn: WeekStart,
  now = new Date()
): DateRange {
  if (preset === "currentWeek" || preset === "previousWeek") {
    const start = weekStartDate(now, weekStartsOn);
    if (preset === "currentWeek") return { start: dateInput(start), end: dateInput(now) };
    return { start: dateInput(addDays(start, -7)), end: dateInput(addDays(start, -1)) };
  }

  const quarterly = preset === "currentQuarter" || preset === "quarter";
  const month = quarterly ? Math.floor(now.getMonth() / 3) * 3 : now.getMonth();
  const year = now.getFullYear();
  const start = calendarDate(year, month, 1);
  if (preset === "currentMonth" || preset === "currentQuarter") {
    return { start: dateInput(start), end: dateInput(now) };
  }
  return {
    start: dateInput(calendarDate(year, month - (quarterly ? 3 : 1), 1)),
    end: dateInput(addDays(start, -1))
  };
}

function calendarDate(year: number, month: number, day: number) {
  const date = new Date(2000, 0, 1, 12);
  date.setFullYear(year, month, day);
  date.setHours(0, 0, 0, 0);
  return date;
}

function parseDateInput(value: string) {
  const [year, month, day] = value.split("-").map(Number);
  return calendarDate(year, month - 1, day);
}

function addDays(date: Date, days: number) {
  return calendarDate(date.getFullYear(), date.getMonth(), date.getDate() + days);
}

function dayIndex(date: Date) {
  const utc = new Date(0);
  utc.setUTCFullYear(date.getFullYear(), date.getMonth(), date.getDate());
  return utc.getTime() / 86_400_000;
}

function validDateInput(value: string) {
  return /^\d{4}-\d{2}-\d{2}$/.test(value) && !value.startsWith("0000") &&
    dateInput(parseDateInput(value)) === value;
}

export function statisticsRangeError(range: DateRange) {
  if (!validDateInput(range.start) || !validDateInput(range.end)) return "请输入有效的开始日期和结束日期";
  if (range.start > range.end) return "开始日期不能晚于结束日期";
  if (range.end === "9999-12-31") return "结束日期超出可查询的日期范围";
  return "";
}

export function statisticsQueryBounds(range: DateRange) {
  return {
    start: parseDateInput(range.start).toISOString(),
    end: addDays(parseDateInput(range.end), 1).toISOString()
  };
}

export function statisticsLocalDay(now = new Date()) {
  return dateInput(now);
}

export function nextStatisticsLocalDay(now = new Date()) {
  return addDays(now, 1);
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

export function statisticsComparisonRange(preset: StatisticsPreset, range: DateRange): DateRange {
  const start = parseDateInput(range.start);
  const end = parseDateInput(range.end);
  if (preset === "currentWeek" || preset === "previousWeek") {
    return { start: dateInput(addDays(start, -7)), end: dateInput(addDays(end, -7)) };
  }

  const days = dayIndex(end) - dayIndex(start) + 1;
  if (preset === "custom") {
    return { start: dateInput(addDays(start, -days)), end: dateInput(addDays(start, -1)) };
  }

  const months = preset === "currentQuarter" || preset === "quarter" ? 3 : 1;
  const comparisonStart = calendarDate(start.getFullYear(), start.getMonth() - months, 1);
  const comparisonEnd = addDays(start, -1);
  if (preset === "month" || preset === "quarter") {
    return { start: dateInput(comparisonStart), end: dateInput(comparisonEnd) };
  }
  const sameDaysEnd = addDays(comparisonStart, days - 1);
  return { start: dateInput(comparisonStart), end: dateInput(sameDaysEnd < comparisonEnd ? sameDaysEnd : comparisonEnd) };
}
