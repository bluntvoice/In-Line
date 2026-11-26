import { describe, expect, it } from "vitest";
import { execFileSync } from "node:child_process";
import { STATISTICS_PRESETS, statisticsComparisonRange, statisticsPresetRange, statisticsQueryBounds, statisticsRangeError } from "./statistics-range";

describe("statistics calendar ranges", () => {
  it("provides all seven selections in display order", () => {
    expect(STATISTICS_PRESETS.map(option => option.label)).toEqual(["本周", "上周", "本月", "上月", "本季度", "上季度", "自定义"]);
  });

  it.each([
    ["currentMonth", new Date(2024, 1, 29, 23, 30), "2024-02-01", "2024-02-29"],
    ["month", new Date(2024, 2, 1), "2024-02-01", "2024-02-29"],
    ["month", new Date(2025, 0, 1), "2024-12-01", "2024-12-31"],
    ["currentQuarter", new Date(2025, 0, 1), "2025-01-01", "2025-01-01"],
    ["currentQuarter", new Date(2025, 3, 1), "2025-04-01", "2025-04-01"],
    ["currentQuarter", new Date(2025, 6, 1), "2025-07-01", "2025-07-01"],
    ["currentQuarter", new Date(2025, 9, 1), "2025-10-01", "2025-10-01"],
    ["currentQuarter", new Date(2024, 8, 30), "2024-07-01", "2024-09-30"],
    ["quarter", new Date(2025, 0, 1), "2024-10-01", "2024-12-31"]
  ] as const)("%s for local date %s", (preset, now, start, end) => {
    expect(statisticsPresetRange(preset, "monday", now)).toEqual({ start, end });
  });

  it.each([
    ["currentWeek", "monday", 4, "2024-12-30", "2025-01-04", "2024-12-23", "2024-12-28"],
    ["currentWeek", "monday", 5, "2024-12-30", "2025-01-05", "2024-12-23", "2024-12-29"],
    ["currentWeek", "sunday", 5, "2025-01-05", "2025-01-05", "2024-12-29", "2024-12-29"],
    ["previousWeek", "monday", 6, "2024-12-30", "2025-01-05", "2024-12-23", "2024-12-29"],
    ["previousWeek", "sunday", 6, "2024-12-29", "2025-01-04", "2024-12-22", "2024-12-28"]
  ] as const)("calculates %s with a %s start on Jan %s", (preset, weekStart, day, start, end, previousStart, previousEnd) => {
    const range = statisticsPresetRange(preset, weekStart, new Date(2025, 0, day));
    expect(range).toEqual({ start, end });
    expect(statisticsComparisonRange(preset, range)).toEqual({ start: previousStart, end: previousEnd });
  });

  it.each([
    ["currentMonth", "2024-03-01", "2024-03-05", "2024-02-01", "2024-02-05"],
    ["currentMonth", "2024-03-01", "2024-03-31", "2024-02-01", "2024-02-29"],
    ["currentMonth", "2025-03-01", "2025-03-31", "2025-02-01", "2025-02-28"],
    ["month", "2025-02-01", "2025-02-28", "2025-01-01", "2025-01-31"],
    ["currentQuarter", "2024-04-01", "2024-04-01", "2024-01-01", "2024-01-01"],
    ["currentQuarter", "2025-01-01", "2025-02-01", "2024-10-01", "2024-11-01"],
    ["currentQuarter", "2025-07-01", "2025-09-30", "2025-04-01", "2025-06-30"],
    ["quarter", "2024-10-01", "2024-12-31", "2024-07-01", "2024-09-30"],
    ["custom", "2024-12-30", "2025-01-02", "2024-12-26", "2024-12-29"],
    ["custom", "2024-03-10", "2024-03-10", "2024-03-09", "2024-03-09"]
  ] as const)("%s compares %s to %s", (preset, start, end, previousStart, previousEnd) => {
    expect(statisticsComparisonRange(preset, { start, end })).toEqual({ start: previousStart, end: previousEnd });
  });

  it.each([
    ["", "2024-02-29"], ["2024-02-29", ""], ["2025-02-29", "2025-03-01"],
    ["2024-04-31", "2024-05-01"], ["2024-01-01", "2024-13-01"], ["2024-01-02", "2024-01-01"]
  ])("rejects invalid or inverted dates %s … %s", (start, end) => {
    expect(statisticsRangeError({ start, end })).not.toBe("");
  });

  it("sends local midnights at the two half-open edges", () => {
    const range = { start: "2024-02-29", end: "2024-02-29" };
    expect(statisticsRangeError(range)).toBe("");
    expect(statisticsQueryBounds(range)).toEqual({
      start: new Date(2024, 1, 29).toISOString(),
      end: new Date(2024, 2, 1).toISOString()
    });
  });

  it("keeps equal calendar-day comparisons across 23-hour and 25-hour days", () => {
    const module = new URL("./statistics-range.ts", import.meta.url).href;
    const result = execFileSync(process.execPath, ["--input-type=module", "-e", `
      import { statisticsComparisonRange, statisticsQueryBounds } from ${JSON.stringify(module)};
      const ranges = [
        { start: "2025-03-09", end: "2025-03-09" },
        { start: "2025-11-02", end: "2025-11-02" }
      ];
      console.log(JSON.stringify(ranges.map(range => ({
        current: statisticsQueryBounds(range),
        comparison: statisticsComparisonRange("custom", range)
      }))));
    `], { env: { ...process.env, TZ: "America/New_York" }, encoding: "utf8" });
    expect(JSON.parse(result)).toEqual([
      {
        current: { start: "2025-03-09T05:00:00.000Z", end: "2025-03-10T04:00:00.000Z" },
        comparison: { start: "2025-03-08", end: "2025-03-08" }
      },
      {
        current: { start: "2025-11-02T04:00:00.000Z", end: "2025-11-03T05:00:00.000Z" },
        comparison: { start: "2025-11-01", end: "2025-11-01" }
      }
    ]);
  });
});
