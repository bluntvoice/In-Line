import {describe,expect,it} from "vitest";
import {pieVerticalLayout,setPngDpi,STATISTICS_EXPORT_DPI,trendBarLayout} from "../src/lib/statistics-export";

const minimalPng=new Uint8Array([
  137,80,78,71,13,10,26,10,
  0,0,0,13,73,72,68,82,0,0,0,1,0,0,0,1,8,6,0,0,0,31,21,196,137,
  0,0,0,0,73,69,78,68,174,66,96,130
]);

describe("statistics chart export",()=>{
  it("adds 300 DPI physical resolution metadata",()=>{
    const output=setPngDpi(minimalPng);
    expect(STATISTICS_EXPORT_DPI).toBe(300);
    expect(new TextDecoder().decode(output.slice(37,41))).toBe("pHYs");
    expect(Array.from(output.slice(41,45))).toEqual([0,0,46,35]);
    expect(Array.from(output.slice(45,49))).toEqual([0,0,46,35]);
    expect(output[49]).toBe(1);
  });
  it("keeps trend bars narrow and centered in their own slots",()=>{
    const first=trendBarLayout(440,5,0),last=trendBarLayout(440,5,4);
    expect(first.barWidth).toBe(22);
    expect(first.barX+(first.barWidth/2)).toBe(first.labelX);
    expect(last.labelX).toBe(396);
  });
  it("centers the pie and its legend vertically inside an equal-height card",()=>{
    const layout=pieVerticalLayout(500,760,8);
    expect(layout.centerY).toBe((500+88+500+760-28)/2);
    expect(layout.legendTop+8*layout.legendRowHeight/2).toBe(layout.centerY);
  });
});
