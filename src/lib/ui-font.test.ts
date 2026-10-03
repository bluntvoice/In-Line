import {describe,expect,it} from "vitest";
import {applyUIFont,RECOMMENDED_UI_FONT_CSS_FAMILY,RECOMMENDED_UI_FONT_FAMILY,DEFAULT_UI_FONT_SELECTION,DEFAULT_UI_FONT_STACK,filterSystemFonts} from "./ui-font";
import type {SystemFont} from "../types";

const fonts:SystemFont[]=[{family:"Microsoft YaHei",displayName:"微软雅黑",aliases:["Microsoft YaHei","微软雅黑"],cjk:true},{family:"Arial",displayName:"Arial",aliases:["Arial"],cjk:false}];
describe("UI font selection",()=>{
  it("searches full, partial, Chinese aliases and case-insensitive English locally",()=>{
    for(const query of ["Microsoft YaHei","yAhEi","雅黑","  yahei  "])expect(filterSystemFonts(fonts,query)).toEqual([fonts[0]]);
    expect(filterSystemFonts(fonts,"ARIAL")).toEqual([fonts[1]]);
    expect(filterSystemFonts(fonts,"")).toBe(fonts);
    expect(filterSystemFonts(fonts,"missing")).toEqual([]);
  });
  it("uses the system default for empty or missing selections and a private family for downloaded fonts",()=>{
    const properties=new Map<string,string>(),attributes=new Map<string,string>();
    const root={style:{setProperty:(key:string,value:string)=>properties.set(key,value),removeProperty:(key:string)=>properties.delete(key)},setAttribute:(key:string,value:string)=>attributes.set(key,value),removeAttribute:(key:string)=>attributes.delete(key)} as unknown as HTMLElement;
    applyUIFont({requested:"Arial",effective:"Arial",missing:false},root);
    expect(properties.get("--ui-font-family")).toContain('"Arial",');
    expect(attributes.get("data-ui-font")).toBe("Arial");
    applyUIFont(DEFAULT_UI_FONT_SELECTION,root);
    expect(properties.get("--ui-font-family")).toBe(DEFAULT_UI_FONT_STACK);
    expect(attributes.get("data-ui-font")).toBe("default");
    applyUIFont({requested:"Missing",effective:"",missing:true},root);
    expect(attributes.get("data-ui-font")).toBe("default");
    applyUIFont({requested:RECOMMENDED_UI_FONT_FAMILY,effective:RECOMMENDED_UI_FONT_FAMILY,missing:false},root);
    expect(properties.get("--ui-font-family")).toBe(`"${RECOMMENDED_UI_FONT_CSS_FAMILY}",${DEFAULT_UI_FONT_STACK}`);
    expect(attributes.get("data-ui-font")).toBe(RECOMMENDED_UI_FONT_CSS_FAMILY);
    applyUIFont({requested:"Sarasa UI SC",effective:"Sarasa UI SC",missing:false},root);
    expect(attributes.get("data-ui-font")).toBe("Sarasa UI SC");
  });
});
