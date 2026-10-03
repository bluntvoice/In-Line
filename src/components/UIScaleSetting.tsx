import {useState} from "react";
import {RotateCcw} from "lucide-react";
import {UI_SCALES,type UIScale} from "../lib/ui-scale";
import {useUIScale} from "./UIScaleProvider";

export default function UIScaleSetting({notify}:{notify:(message:string)=>void}){
  const {scale,unavailable,save}=useUIScale();
  const [busy,setBusy]=useState(false);
  const change=async(value:UIScale)=>{setBusy(true);try{await save(value);notify(`界面大小已设为 ${value}%`);}catch(error){notify("调整界面大小失败："+String(error));}finally{setBusy(false);}};
  return <div className="setting-row ui-scale-setting"><div><strong>界面大小</strong><span>统一调整文字、按钮与间距，所有窗口生效</span>{unavailable&&<small className="font-fallback-note">暂时无法读取设置，请重试。</small>}</div><div className="ui-scale-actions"><select aria-label="界面大小" disabled={busy} value={scale} onChange={event=>void change(Number(event.target.value) as UIScale)}>{UI_SCALES.map(value=><option key={value} value={value}>{value}%{value===100?"（默认）":""}</option>)}</select><button type="button" className="button secondary small" disabled={busy||(scale===100&&!unavailable)} onClick={()=>void change(100)}><RotateCcw size={15}/>恢复默认</button></div></div>;
}
