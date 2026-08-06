export type WeekStart = "monday" | "sunday";
export type StatisticsPreset = "currentWeek" | "previousWeek" | "month" | "quarter" | "custom";

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
