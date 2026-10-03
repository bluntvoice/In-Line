import type {SystemFont,UiFontSelection} from "../types";

export const BUNDLED_UI_FONT_FAMILY="Sarasa UI SC";
export const BUNDLED_UI_FONT_CSS_FAMILY="In Line Sarasa UI SC";
export const BUNDLED_UI_FONT_NAME="更纱黑体 UI SC";
export const DEFAULT_UI_FONT_STACK='"In Line Sarasa UI SC","Microsoft YaHei UI","Microsoft YaHei","Segoe UI",sans-serif';
export const DEFAULT_UI_FONT_SELECTION:UiFontSelection={requested:"",effective:"",missing:false};

export function filterSystemFonts(fonts:SystemFont[],query:string){
  const needle=query.trim().toLocaleLowerCase();
  return needle?fonts.filter(font=>[font.family,font.displayName,...font.aliases].some(name=>name.toLocaleLowerCase().includes(needle))):fonts;
}

export function applyUIFont(selection:UiFontSelection,root:HTMLElement=document.documentElement){
  if(selection.effective&&selection.effective!==BUNDLED_UI_FONT_FAMILY){
    // Quoting via a string literal prevents a family name from becoming CSS syntax.
    root.style.setProperty("--ui-font-family",`${JSON.stringify(selection.effective)},${DEFAULT_UI_FONT_STACK}`);
    root.setAttribute("data-ui-font",selection.effective);
  }else{
    root.style.setProperty("--ui-font-family",DEFAULT_UI_FONT_STACK);
    root.setAttribute("data-ui-font",BUNDLED_UI_FONT_CSS_FAMILY);
  }
}
