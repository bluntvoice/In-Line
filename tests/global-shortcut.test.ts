import {describe,expect,it} from "vitest";
import {shortcutFromKeyboardEvent,shortcutUsageHint} from "../src/lib/global-shortcut";

const event=(code:string,values:Partial<{ctrlKey:boolean;altKey:boolean;shiftKey:boolean;metaKey:boolean}>={})=>({code,ctrlKey:false,altKey:false,shiftKey:false,metaKey:false,...values});

describe("global shortcut capture",()=>{
  it("captures letters, digits and function keys with stable modifier ordering",()=>{
    expect(shortcutFromKeyboardEvent(event("KeyK",{ctrlKey:true,altKey:true}))).toEqual({shortcut:"Ctrl+Alt+K"});
    expect(shortcutFromKeyboardEvent(event("Digit8",{altKey:true,shiftKey:true}))).toEqual({shortcut:"Alt+Shift+8"});
    expect(shortcutFromKeyboardEvent(event("F8",{altKey:true}))).toEqual({shortcut:"Alt+F8"});
  });
  it("rejects unsafe or incomplete combinations",()=>{
    expect(shortcutFromKeyboardEvent(event("KeyI"))).toEqual({error:"请至少同时按住 Ctrl 或 Alt"});
    expect(shortcutFromKeyboardEvent(event("KeyI",{metaKey:true}))).toEqual({error:"Windows 键组合由系统保留，请改用 Ctrl 或 Alt"});
    expect(shortcutFromKeyboardEvent(event("F4",{altKey:true}))).toEqual({error:"Alt+F4 是 Windows 关闭窗口快捷键，不能使用"});
  });
  it("warns when a valid global shortcut may overlap application shortcuts",()=>{
    expect(shortcutUsageHint("Ctrl+I")).toContain("其他软件");
    expect(shortcutUsageHint("Ctrl+Alt+I")).toContain("可以使用");
  });
});
