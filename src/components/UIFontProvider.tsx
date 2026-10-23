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
    let active=true,request=0,timer:ReturnType<typeof setTimeout>|undefined;
    const refresh=async(attempt=0)=>{
      const id=++request;
      clearTimeout(timer);
      try{
        const selection=await api.getUIFontSelection();
        if(selection.effective===RECOMMENDED_UI_FONT_FAMILY)await loadRecommendedFont();
        if(!active||id!==request)return;
        applyUIFont(selection);setState({selection,unavailable:false});
      }catch{
        if(!active||id!==request)return;
        // A temporary startup/read failure must not erase an already loaded
        // font or write a default selection back over the saved preference.
        setState(current=>({...current,unavailable:true}));
        const delay=[150,500,1500][attempt];
        if(delay!==undefined)timer=setTimeout(()=>{if(active&&id===request)void refresh(attempt+1);},delay);
      }
    };
    void refresh();
    // Same native broadcast already used for settings and backup restoration, in every route.
    const dispose=api.onDataChanged(()=>void refresh());
    const focus=()=>void refresh();window.addEventListener("focus",focus);
    return()=>{active=false;clearTimeout(timer);dispose();window.removeEventListener("focus",focus);};
  },[]);
  return <FontContext.Provider value={state}>{children}</FontContext.Provider>;
}
