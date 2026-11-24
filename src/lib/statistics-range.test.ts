import { describe, expect, it } from "vitest";
import { statisticsComparisonRange, statisticsPresetRange } from "./statistics-range";

describe("statistics calendar ranges", () => {
  it.each([
    ["currentMonth", new Date(2024, 1, 1, 0, 30), "2024-02-01", "2024-02-01"],
    ["currentMonth", new Date(2024, 1, 29, 23, 30), "2024-02-01", "2024-02-29"],
    ["month", new Date(2024, 2, 1), "2024-02-01", "2024-02-29"],
    ["month", new Date(2025, 0, 1), "2024-12-01", "2024-12-31"],
    ["currentQuarter", new Date(2025, 0, 1), "2025-01-01", "2025-01-01"],
    ["currentQuarter", new Date(2024, 8, 30), "2024-07-01", "2024-09-30"],
    ["quarter", new Date(2025, 0, 1), "2024-10-01", "2024-12-31"]
  ] as const)("%s for local date %s", (preset, now, start, end) => {
    expect(statisticsPresetRange(preset, "monday", now)).toEqual({ start, end });
  });

  it.each([
    ["currentMonth", "2024-03-01", "2024-03-05", "2024-02-01", "2024-02-05"],
    ["currentMonth", "2024-03-01", "2024-03-31", "2024-02-01", "2024-02-29"],
    ["currentQuarter", "2024-04-01", "2024-04-01", "2024-01-01", "2024-01-01"],
    ["currentQuarter", "2025-01-01", "2025-02-01", "2024-10-01", "2024-11-01"],
    ["currentQuarter", "2025-07-01", "2025-09-30", "2025-04-01", "2025-06-30"],
    ["previousWeek", "2024-03-10", "2024-03-16", "2024-03-03", "2024-03-09"],
    ["custom", "2024-12-30", "2025-01-02", "2024-12-26", "2024-12-29"]
  ] as const)("%s compares %s to %s", (preset, start, end, previousStart, previousEnd) => {
    expect(statisticsComparisonRange(preset, { start, end })).toEqual({ start: previousStart, end: previousEnd });
  });
});
