import {createContext,useContext,useEffect,useRef,useState,type ReactNode} from "react";
import {api} from "../api";
import {resolveUIScale,type UIScale} from "../lib/ui-scale";

const ScaleContext=createContext<{scale:UIScale;unavailable:boolean;save:(scale:UIScale)=>Promise<void>}>({scale:100,unavailable:false,save:async()=>{throw new Error("界面大小尚未初始化");}});
export const useUIScale=()=>useContext(ScaleContext);
export default function UIScaleProvider({children}:{children:ReactNode}){
  const [state,setState]=useState({scale:100 as UIScale,unavailable:false});
  const request=useRef(0);
  useEffect(()=>{
    let active=true,timer:ReturnType<typeof setTimeout>|undefined;
    const refresh=async(attempt=0)=>{
      const id=++request.current;
      clearTimeout(timer);
      try{
        const scale=resolveUIScale(await api.getUIScale());
        if(active&&id===request.current)setState({scale,unavailable:false});
      }catch{
        if(!active||id!==request.current)return;
        setState(current=>({...current,unavailable:true}));
        const delay=[150,500,1500][attempt];
        if(delay!==undefined)timer=setTimeout(()=>{if(active&&id===request.current)void refresh(attempt+1);},delay);
      }
    };
    void refresh();const off=api.onDataChanged(()=>void refresh());
    const focus=()=>void refresh();window.addEventListener("focus",focus);
    return()=>{active=false;clearTimeout(timer);off();window.removeEventListener("focus",focus);};
  },[]);
  const save=async(scale:UIScale)=>{await api.setSetting("ui_scale",String(scale));++request.current;setState({scale,unavailable:false});};
  return <ScaleContext.Provider value={{...state,save}}>{children}</ScaleContext.Provider>;
}
