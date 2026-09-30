import {describe,expect,it} from "vitest";
import {applyUIFont,DEFAULT_UI_FONT_SELECTION,filterSystemFonts} from "./ui-font";
import type {SystemFont} from "../types";

const fonts:SystemFont[]=[{family:"Microsoft YaHei",displayName:"微软雅黑",aliases:["Microsoft YaHei","微软雅黑"],cjk:true},{family:"Arial",displayName:"Arial",aliases:["Arial"],cjk:false}];
describe("UI font selection",()=>{
  it("searches full, partial, Chinese aliases and case-insensitive English locally",()=>{
    for(const query of ["Microsoft YaHei","yAhEi","雅黑","  yahei  "])expect(filterSystemFonts(fonts,query)).toEqual([fonts[0]]);
    expect(filterSystemFonts(fonts,"ARIAL")).toEqual([fonts[1]]);
    expect(filterSystemFonts(fonts,"")).toBe(fonts);
    expect(filterSystemFonts(fonts,"missing")).toEqual([]);
  });
  it("applies a quoted global variable and restores the untouched default strategy",()=>{
    const properties=new Map<string,string>(),attributes=new Map<string,string>();
    const root={style:{setProperty:(key:string,value:string)=>properties.set(key,value),removeProperty:(key:string)=>properties.delete(key)},setAttribute:(key:string,value:string)=>attributes.set(key,value),removeAttribute:(key:string)=>attributes.delete(key)} as unknown as HTMLElement;
    applyUIFont({requested:"Arial",effective:"Arial",missing:false},root);
    expect(properties.get("--ui-font-family")).toContain('"Arial",');
    expect(attributes.get("data-ui-font")).toBe("Arial");
    applyUIFont(DEFAULT_UI_FONT_SELECTION,root);
    expect(properties.size).toBe(0);expect(attributes.size).toBe(0);
    applyUIFont({requested:"Missing",effective:"",missing:true},root);
    expect(attributes.size).toBe(0);
  });
});
