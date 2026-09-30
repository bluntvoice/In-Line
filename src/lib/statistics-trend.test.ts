import {describe,expect,it} from "vitest";
import {statisticsDisplayTrend,statisticsTrendRange} from "./statistics-range";

describe("trend drill-down ranges",()=>{
  it("preserves result counts when filling zero weekdays",()=>{
    const point={periodStart:"2026-09-21",handledTasks:3,processed:1,completed:1};
    const display=statisticsDisplayTrend([point],"day",{start:"2026-09-21",end:"2026-09-22"},"custom");
    expect(display[0]).toEqual(point);
    expect(display[1].handledTasks).toBe(0);
  });
  it("clips first and last weekly buckets to the selected range",()=>{
    const range={start:"2026-09-23",end:"2026-10-01"};
    expect(statisticsTrendRange("2026-09-21","week",range)).toEqual({start:"2026-09-23",end:"2026-09-27"});
    expect(statisticsTrendRange("2026-09-28","week",range)).toEqual({start:"2026-09-28",end:"2026-10-01"});
    expect(statisticsTrendRange("2026-09-24","day",range)).toEqual({start:"2026-09-24",end:"2026-09-24"});
  });
});
