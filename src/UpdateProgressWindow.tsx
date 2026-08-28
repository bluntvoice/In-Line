import { useEffect,useState } from "react";
import { Download,RefreshCw,ShieldCheck,X } from "lucide-react";
import { api } from "./api";
import {updateProgressPresentation} from "./lib/update-ui";
import type { UpdateProgress } from "./types";

const EMPTY:UpdateProgress={phase:"idle",version:null,downloadedBytes:0,totalBytes:null,percent:null,message:null};

export default function UpdateProgressWindow(){
  const [progress,setProgress]=useState<UpdateProgress>(EMPTY);
  const [retrying,setRetrying]=useState(false);
  useEffect(()=>{
    void api.getUpdateProgress().then(setProgress).catch(()=>undefined);
    return api.onUpdateProgress(setProgress);
  },[]);
  const retry=async()=>{
    setRetrying(true);
    try{await api.checkForUpdate();}
    catch(error){setProgress(current=>({...current,phase:"failed",message:error instanceof Error?error.message:String(error)}));}
    finally{setRetrying(false);}
  };
  const presentation=updateProgressPresentation(progress);
  return <main className="update-progress-card">
    <header><div className="update-progress-icon">{progress.phase==="verifying"?<ShieldCheck size={19}/>:<Download size={19}/>}</div><div><strong>{presentation.title}</strong><span>{progress.version?`v${progress.version}`:"正在准备…"}</span></div><button type="button" title="隐藏" onClick={()=>void api.hideUpdateProgress()}><X size={16}/></button></header>
    <p>{presentation.detail}</p>
    {presentation.showProgress&&<div className={`update-progress-track${presentation.indeterminate?" indeterminate":""}`}><span style={presentation.indeterminate?undefined:{width:`${progress.percent}%`}}/></div>}
    {progress.phase==="failed"&&<footer><button type="button" disabled={retrying} onClick={()=>void retry()}><RefreshCw size={14} className={retrying?"spinning":""}/>{retrying?"正在重试…":"重新下载"}</button><button type="button" onClick={()=>void api.hideUpdateProgress()}>关闭</button></footer>}
    {progress.phase==="downloading"&&<small>关闭此窗口不会中断下载，可在“关于”页面恢复显示。</small>}
  </main>;
}
