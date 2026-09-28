import { describe,expect,it } from "vitest";
import { defaultTaskColumnLayouts,normalizeTaskColumnLayouts,normalizeTaskColumnWidth,serializeTaskColumnLayouts } from "./column-widths";

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
});
