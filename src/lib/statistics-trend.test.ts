import {describe,expect,it} from "vitest";
import {statisticsDisplayTrend,statisticsTrendLabel,statisticsTrendRange} from "./statistics-range";

describe("trend drill-down ranges",()=>{
  it("labels quarterly weekly totals with their whole date range",()=>{
    const range={start:"2026-07-01",end:"2026-09-30"};
    expect(statisticsTrendLabel("2026-07-13","week",range)).toEqual({primary:"07-13",secondary:"至 07-19",title:"2026-07-13 至 2026-07-19"});
    expect(statisticsTrendLabel("2026-07-20","week",range)).toEqual({primary:"07-20",secondary:"至 07-26",title:"2026-07-20 至 2026-07-26"});
  });
  it("labels partial quarter weeks with the same clipped dates as drill-down",()=>{
    const range={start:"2026-07-01",end:"2026-09-30"};
    expect(statisticsTrendLabel("2026-06-29","week",range)).toEqual({primary:"07-01",secondary:"至 07-05",title:"2026-07-01 至 2026-07-05"});
    expect(statisticsTrendLabel("2026-09-28","week",range)).toEqual({primary:"09-28",secondary:"至 09-30",title:"2026-09-28 至 2026-09-30"});
  });
  it("preserves daily weekdays and identifies years in cross-year week tooltips",()=>{
    expect(statisticsTrendLabel("2026-09-22","day",{start:"2026-09-21",end:"2026-09-25"})).toEqual({primary:"周二",secondary:"09-22",title:"2026-09-22（周二）"});
    expect(statisticsTrendLabel("2026-12-28","week",{start:"2026-12-01",end:"2027-01-31"})).toEqual({primary:"12-28",secondary:"至 01-03",title:"2026-12-28 至 2027-01-03"});
  });
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
