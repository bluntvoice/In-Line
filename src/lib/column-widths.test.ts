import { describe,expect,it } from "vitest";
import { defaultTaskColumnLayouts,fitTaskColumnWidths,normalizeTaskColumnLayouts,normalizeTaskColumnWidth,serializeTaskColumnLayouts,TASK_COLUMN_DEFINITIONS } from "./column-widths";

describe("task table column widths",()=>{
  it("keeps layouts independent for each page",()=>{
    const layouts=defaultTaskColumnLayouts();
    layouts.queue.title=360;
    layouts.archive.title=420;
    const restored=normalizeTaskColumnLayouts(serializeTaskColumnLayouts(layouts));
    expect(restored.queue.title).toBe(360);
    expect(restored.archive.title).toBe(420);
    expect(restored.trash.title).not.toBe(360);
  });

  it("merges partial saved columns with current defaults",()=>{
    const restored=normalizeTaskColumnLayouts(JSON.stringify({version:1,pages:{queue:{title:380,unknown:999}}}));
    expect(restored.queue.title).toBe(380);
    expect(restored.queue.number).toBe(defaultTaskColumnLayouts().queue.number);
    expect(Object.prototype.hasOwnProperty.call(restored.queue,"unknown")).toBe(false);
  });

  it("falls back safely for corrupt or out-of-range values",()=>{
    const defaults=defaultTaskColumnLayouts();
    expect(normalizeTaskColumnLayouts("not-json")).toEqual(defaults);
    const restored=normalizeTaskColumnLayouts({pages:{queue:{title:-10,status:"wide",number:Number.NaN}}});
    expect(restored.queue.title).toBe(defaults.queue.title);
    expect(restored.queue.status).toBe(defaults.queue.status);
    expect(restored.queue.number).toBe(defaults.queue.number);
    expect(normalizeTaskColumnWidth("title",420.4)).toBe(420);
  });

  it("fits a modestly overflowing table while preserving saved widths",()=>{
    const saved=defaultTaskColumnLayouts().queue;
    const fitted=fitTaskColumnWidths(saved,920);
    expect(TASK_COLUMN_DEFINITIONS.reduce((sum,column)=>sum+fitted[column.id],0)).toBe(920);
    expect(TASK_COLUMN_DEFINITIONS.every(column=>fitted[column.id]>=column.minWidth)).toBe(true);
    expect(saved.title).toBe(255);
    expect(fitTaskColumnWidths(saved,1200)).toBe(saved);
  });

  it("keeps horizontal scrolling possible when the viewport is narrower than all minimums",()=>{
    const fitted=fitTaskColumnWidths(defaultTaskColumnLayouts().queue,700);
    expect(TASK_COLUMN_DEFINITIONS.reduce((sum,column)=>sum+fitted[column.id],0)).toBeGreaterThan(700);
  });

  it("preserves intentional wide column layouts and their horizontal scrolling",()=>{
    const saved=defaultTaskColumnLayouts().queue;
    saved.title+=160;
    expect(fitTaskColumnWidths(saved,920)).toBe(saved);
  });

  it("preserves even modest manual changes instead of compressing saved layouts",()=>{
    const saved=defaultTaskColumnLayouts().queue;
    saved.title+=8;
    expect(fitTaskColumnWidths(saved,920)).toBe(saved);
    expect(fitTaskColumnWidths(saved,700)).toBe(saved);
    expect(saved.title).toBe(263);
  });

  it("fits default widths at fractional viewport boundaries without dropping minimums",()=>{
    const fitted=fitTaskColumnWidths(defaultTaskColumnLayouts().queue,924.75);
    expect(TASK_COLUMN_DEFINITIONS.reduce((sum,column)=>sum+fitted[column.id],0)).toBe(924);
    expect(TASK_COLUMN_DEFINITIONS.every(column=>fitted[column.id]>=column.minWidth)).toBe(true);
  });

  it("reserves badge and cell padding at the horizontal scrolling boundary",()=>{
    const saved=defaultTaskColumnLayouts().queue;
    const fitted=fitTaskColumnWidths(saved,700,98);
    expect(fitted.number).toBe(98);
    expect(saved.number).toBe(82);
    expect(TASK_COLUMN_DEFINITIONS.reduce((sum,column)=>sum+fitted[column.id],0)).toBeGreaterThan(700);
    expect(TASK_COLUMN_DEFINITIONS.reduce((sum,column)=>sum+fitTaskColumnWidths(saved,920,98)[column.id],0)).toBe(920);
  });

  it("protects wider number content without rewriting saved custom columns",()=>{
    const saved=defaultTaskColumnLayouts().queue;
    saved.number=70;saved.title=420;
    const rendered=fitTaskColumnWidths(saved,700,124.4);
    expect(rendered.number).toBe(125);
    expect(rendered.title).toBe(420);
    expect(saved.number).toBe(70);
    expect(saved.title).toBe(420);
  });
});
