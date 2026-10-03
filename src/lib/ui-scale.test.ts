import {describe,expect,it} from "vitest";
import {resolveUIScale,UI_SCALES} from "./ui-scale";
describe("UI scale restoration",()=>{
  it("keeps the six supported values and safely falls back for old or invalid settings",()=>{
    for(const value of UI_SCALES)expect(resolveUIScale(String(value))).toBe(value);
    for(const value of [undefined,null,"",90,160,"NaN",123,"1.5"])expect(resolveUIScale(value)).toBe(100);
  });
});
