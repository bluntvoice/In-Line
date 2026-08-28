import { useEffect,useRef,useState } from "react";
import { CheckCircle2,Copy,ExternalLink,RefreshCw,UserRound } from "lucide-react";
import { api } from "../api";
import {UPDATE_BUTTONS} from "../lib/update-ui";

const REPOSITORY = "https://github.com/bluntvoice/In-Line";

interface Props {
  version: string;
  onCopy: (value: string) => Promise<void>;
}

export default function AboutPanel({ version,onCopy }: Props) {
  const [checking,setChecking]=useState(false);
  const [buttonText,setButtonText]=useState<string>(UPDATE_BUTTONS.idle.text);
  const [message,setMessage]=useState("");
  const resetTimer=useRef<number|null>(null);
  useEffect(()=>()=>{
    if(resetTimer.current!==null)window.clearTimeout(resetTimer.current);
  },[]);
  const resetButton=()=>{
    if(resetTimer.current!==null)window.clearTimeout(resetTimer.current);
    resetTimer.current=window.setTimeout(()=>setButtonText(UPDATE_BUTTONS.idle.text),3000);
  };
  const checkUpdate=async()=>{
    if(checking)return;
    setChecking(true);setButtonText(UPDATE_BUTTONS.checking.text);setMessage("");
    try{
      const result=await api.checkForUpdate();
      if(result.status==="up_to_date"){
        setButtonText(UPDATE_BUTTONS.latest.text);resetButton();
      }else{
        setButtonText(UPDATE_BUTTONS.idle.text);setMessage(result.remoteVersion?`发现新版本 v${result.remoteVersion}，正在下载`:"正在下载更新");
      }
    }catch(error){
      setButtonText(UPDATE_BUTTONS.idle.text);
      const text=error instanceof Error?error.message:String(error);
      setMessage(text||"检查更新失败，请稍后重试");
    }finally{setChecking(false);}
  };
  const displayVersion=version?`v${version}`:"读取中…";
  return <section className="about-page">
    <header><span>关于 In Line</span><h1>排着呢</h1><p>一个纯本地、轻量的通用事项取号与队列工具。</p></header>
    <div className="about-card"><img src="/inline-mark.svg" alt="In Line 图标"/><div><strong>In Line</strong><span>当前版本 {displayVersion}</span></div></div>
    <dl className="about-details">
      <div><dt><UserRound size={17}/>项目作者</dt><dd className="about-author"><span>六朝声</span><small>（微信公众号：凡声）</small></dd></div>
      <div><dt><ExternalLink size={17}/>GitHub 开源地址</dt><dd><code>{REPOSITORY}</code><button type="button" onClick={()=>void onCopy(REPOSITORY)}><Copy size={15}/>复制地址</button></dd></div>
      <div><dt><CheckCircle2 size={17}/>软件版本</dt><dd><span className="about-version-value"><span>{displayVersion}</span>{message&&<small>{message}</small>}</span><button type="button" disabled={checking} onClick={()=>void checkUpdate()}><RefreshCw size={15} className={checking?"spinning":""}/>{buttonText}</button></dd></div>
    </dl>
    <p className="about-license">本项目依据 GNU General Public License v3.0 开源。</p>
  </section>;
}
