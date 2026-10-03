import {convertFileSrc} from "@tauri-apps/api/core";
import {RECOMMENDED_UI_FONT_CSS_FAMILY} from "./ui-font";

let fontRequest:Promise<void>|null=null;
export function loadRecommendedFont(){
  return fontRequest??(fontRequest=(async()=>{
    const definitions=[{file:"Regular",weight:400},{file:"SemiBold",weight:600},{file:"Bold",weight:700}];
    const faces=await Promise.all(definitions.map(async({file,weight})=>{
      const url=convertFileSrc(`SarasaUiSC-${file}.woff2`,"recommended-font");
      return new FontFace(RECOMMENDED_UI_FONT_CSS_FAMILY,`url(${JSON.stringify(url)}) format("woff2")`,{weight:String(weight),style:"normal",display:"swap"}).load();
    }));
    for(const face of faces)document.fonts.add(face);
  })().catch(error=>{fontRequest=null;throw error;}));
}
