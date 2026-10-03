import {createContext,useContext,useEffect,useRef,useState,type ReactNode} from "react";
import {api} from "../api";
import {resolveUIScale,type UIScale} from "../lib/ui-scale";

const ScaleContext=createContext<{scale:UIScale;unavailable:boolean;save:(scale:UIScale)=>Promise<void>}>({scale:100,unavailable:false,save:async()=>{throw new Error("界面大小尚未初始化");}});
export const useUIScale=()=>useContext(ScaleContext);
export default function UIScaleProvider({children}:{children:ReactNode}){
  const [state,setState]=useState({scale:100 as UIScale,unavailable:false});
  const request=useRef(0);
  useEffect(()=>{
    let active=true;
    const refresh=async()=>{
      const id=++request.current;
      try{
        const scale=resolveUIScale(await api.getUIScale());
        if(active&&id===request.current)setState({scale,unavailable:false});
      }catch{if(active&&id===request.current)setState(current=>({...current,unavailable:true}));}
    };
    void refresh();const off=api.onDataChanged(()=>void refresh());return()=>{active=false;off();};
  },[]);
  const save=async(scale:UIScale)=>{await api.setSetting("ui_scale",String(scale));++request.current;setState({scale,unavailable:false});};
  return <ScaleContext.Provider value={{...state,save}}>{children}</ScaleContext.Provider>;
}
