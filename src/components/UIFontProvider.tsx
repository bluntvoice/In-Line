import {createContext,useContext,useEffect,useState,type ReactNode} from "react";
import {api} from "../api";
import type {UiFontSelection} from "../types";
import {applyUIFont,DEFAULT_UI_FONT_SELECTION,RECOMMENDED_UI_FONT_FAMILY} from "../lib/ui-font";
import {loadRecommendedFont} from "../lib/recommended-font";

const FontContext=createContext<{selection:UiFontSelection;unavailable:boolean}>({selection:DEFAULT_UI_FONT_SELECTION,unavailable:false});
export const useUIFont=()=>useContext(FontContext);

export default function UIFontProvider({children}:{children:ReactNode}){
  const [state,setState]=useState({selection:DEFAULT_UI_FONT_SELECTION,unavailable:false});
  useEffect(()=>{
    let active=true,request=0;
    const refresh=async()=>{
      const id=++request;
      try{
        const selection=await api.getUIFontSelection();
        if(selection.effective===RECOMMENDED_UI_FONT_FAMILY)await loadRecommendedFont();
        if(!active||id!==request)return;
        applyUIFont(selection);setState({selection,unavailable:false});
      }catch{
        if(!active||id!==request)return;
        applyUIFont(DEFAULT_UI_FONT_SELECTION);setState({selection:DEFAULT_UI_FONT_SELECTION,unavailable:true});
      }
    };
    void refresh();
    // Same native broadcast already used for settings and backup restoration, in every route.
    const dispose=api.onDataChanged(()=>void refresh());
    return()=>{active=false;dispose();};
  },[]);
  return <FontContext.Provider value={state}>{children}</FontContext.Provider>;
}
