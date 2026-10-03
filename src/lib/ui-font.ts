import type {SystemFont,UiFontSelection} from "../types";

export const RECOMMENDED_UI_FONT_FAMILY="in-line:sarasa-ui-sc";
export const RECOMMENDED_UI_FONT_CSS_FAMILY="In Line Sarasa UI SC";
export const RECOMMENDED_UI_FONT_NAME="更纱黑体 UI SC";
export const DEFAULT_UI_FONT_STACK='"Microsoft YaHei UI","Microsoft YaHei","Segoe UI",sans-serif';
export const DEFAULT_UI_FONT_SELECTION:UiFontSelection={requested:"",effective:"",missing:false};

export function filterSystemFonts(fonts:SystemFont[],query:string){
  const needle=query.trim().toLocaleLowerCase();
  return needle?fonts.filter(font=>[font.family,font.displayName,...font.aliases].some(name=>name.toLocaleLowerCase().includes(needle))):fonts;
}

export function applyUIFont(selection:UiFontSelection,root:HTMLElement=document.documentElement){
  if(selection.effective){
    // Quoting via a string literal prevents a family name from becoming CSS syntax.
    const family=selection.effective===RECOMMENDED_UI_FONT_FAMILY?RECOMMENDED_UI_FONT_CSS_FAMILY:selection.effective;
    root.style.setProperty("--ui-font-family",`${JSON.stringify(family)},${DEFAULT_UI_FONT_STACK}`);
    root.setAttribute("data-ui-font",family);
  }else{
    root.style.setProperty("--ui-font-family",DEFAULT_UI_FONT_STACK);
    root.setAttribute("data-ui-font","default");
  }
}
