import {useEffect,useRef,useState} from "react";
import {api} from "../api";

export default function FloatingWindowSetting({notify}:{notify:(message:string)=>void}){
  const [visible,setVisible]=useState<boolean|null>(null);
  const [busy,setBusy]=useState(false);
  const [unavailable,setUnavailable]=useState(false);
  const revision=useRef(0),saving=useRef(false);

  useEffect(()=>{
    let active=true;
    const refresh=()=>{
      if(saving.current)return;
      const current=++revision.current;
      void api.getFloatingVisible().then(value=>{
        if(active&&current===revision.current){setVisible(value);setUnavailable(false);}
      }).catch(()=>{if(active&&current===revision.current)setUnavailable(true);});
    };
    const dispose=api.onFloatingVisibilityChanged(value=>{
      if(active){revision.current++;setVisible(value);setUnavailable(false);}
    });
    refresh();
    window.addEventListener("focus",refresh);
    return()=>{active=false;revision.current++;dispose();window.removeEventListener("focus",refresh);};
  },[]);

  const change=async(value:boolean)=>{
    if(saving.current)return;
    saving.current=true;setBusy(true);
    const current=++revision.current;
    try{
      const actual=await api.setFloatingVisible(value);
      if(current===revision.current){setVisible(actual);setUnavailable(false);}
    }catch(error){notify("悬浮窗设置失败："+String(error));}
    finally{saving.current=false;setBusy(false);}
  };

  return <div className="setting-row floating-window-setting"><div><strong>桌面悬浮窗</strong><span>关闭主界面后默认显示，也可在此手动显示或隐藏</span>{unavailable&&<small className="font-fallback-note" role="status">暂时无法读取悬浮窗状态，重新打开设置或切回窗口可重试。</small>}</div><div className="floating-switch-control"><span aria-live="polite">{busy?"切换中…":unavailable?"状态不可用":visible===null?"读取中…":visible?"已显示":"已隐藏"}</span><label className="switch"><input type="checkbox" role="switch" aria-label="显示桌面悬浮窗" checked={visible===true} disabled={busy||visible===null||unavailable} onChange={event=>void change(event.target.checked)}/><span aria-hidden="true"/></label></div></div>;
}
