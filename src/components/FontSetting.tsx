import {useEffect,useMemo,useRef,useState} from "react";
import {Check,ChevronDown,Download,RotateCcw,Search,X} from "lucide-react";
import {api} from "../api";
import type {RecommendedFontProgress,SystemFont} from "../types";
import {RECOMMENDED_UI_FONT_FAMILY,RECOMMENDED_UI_FONT_NAME,filterSystemFonts} from "../lib/ui-font";
import {useUIFont} from "./UIFontProvider";

let fontsRequest:Promise<SystemFont[]>|null=null;
const loadFonts=()=>fontsRequest??(fontsRequest=api.listSystemFonts().catch(error=>{fontsRequest=null;throw error;}));
const initialProgress:RecommendedFontProgress={phase:"idle",downloadedBytes:0,totalBytes:25443648,percent:0,message:null};

export default function FontSetting({notify}:{notify:(text:string)=>void}){
  const {selection,unavailable}=useUIFont();
  const [open,setOpen]=useState(false),[query,setQuery]=useState("");
  const [fonts,setFonts]=useState<SystemFont[]>([]),[loading,setLoading]=useState(false),[busy,setBusy]=useState(false),[error,setError]=useState("");
  const [progress,setProgress]=useState(initialProgress),[statusError,setStatusError]=useState("");
  const downloading=progress.phase==="downloading"||progress.phase==="verifying",ready=progress.phase==="ready";
  const recommendedSelected=selection.effective===RECOMMENDED_UI_FONT_FAMILY;
  const current=fonts.find(font=>font.family===selection.effective);
  const results=useMemo(()=>filterSystemFonts(fonts,query),[fonts,query]);
  const trigger=useRef<HTMLButtonElement>(null),search=useRef<HTMLInputElement>(null);
  useEffect(()=>{
    let active=true,revision=0;
    const off=api.onRecommendedFontProgress(value=>{revision++;if(active){setProgress(value);setStatusError("");}});
    const refresh=()=>{const id=revision;void api.getRecommendedFontStatus().then(value=>{if(active&&id===revision){setProgress(value);setStatusError("");}}).catch(value=>{if(active)setStatusError(String(value));});};
    refresh();window.addEventListener("focus",refresh);
    return()=>{active=false;off();window.removeEventListener("focus",refresh);};
  },[]);
  useEffect(()=>{
    if(!open)return;
    let active=true;setLoading(true);setError("");
    void loadFonts().then(values=>{if(active)setFonts(values);}).catch(value=>{if(active)setError(String(value));}).finally(()=>{if(active)setLoading(false);});
    search.current?.focus();return()=>{active=false;};
  },[open]);
  const close=()=>{setOpen(false);trigger.current?.focus();};
  useEffect(()=>{
    if(!open)return;
    const escape=(event:KeyboardEvent)=>{if(event.key==="Escape"&&!busy)close();};
    window.addEventListener("keydown",escape);return()=>window.removeEventListener("keydown",escape);
  },[open,busy]);
  const choose=async(family:string)=>{
    setBusy(true);setError("");
    try{await api.setSetting("ui_font_family",family);close();notify(family?"软件字体已更新，所有窗口即时生效":"已恢复系统默认字体");}
    catch(value){setError(String(value));notify("设置字体失败："+String(value));}finally{setBusy(false);}
  };
  const download=async()=>{
    setBusy(true);setStatusError("");
    try{await api.downloadRecommendedFont();notify("推荐字体在后台下载，完成后自动启用");}catch(value){setStatusError(String(value));}finally{setBusy(false);}
  };
  return <>
    <div className="setting-row font-setting-row"><div><strong>软件字体</strong><span>可选本机字体，所有窗口即时生效</span>{selection.missing&&<small className="font-fallback-note">原字体“{selection.requested===RECOMMENDED_UI_FONT_FAMILY?RECOMMENDED_UI_FONT_NAME:selection.requested}”不可用，当前使用系统默认字体。</small>}{unavailable&&<small className="font-fallback-note">字体加载失败，当前使用系统默认字体；可重新下载或选择其他字体。</small>}</div><div className="font-setting-actions"><button ref={trigger} type="button" className="button secondary font-setting-trigger" disabled={busy} aria-haspopup="dialog" aria-expanded={open} onClick={()=>{setQuery("");setOpen(true);}}>{recommendedSelected?RECOMMENDED_UI_FONT_NAME:current?.displayName||selection.effective||"系统默认字体"}<ChevronDown size={15}/></button><button type="button" className="button secondary small" disabled={busy||(!selection.requested&&!unavailable)} onClick={()=>void choose("")}><RotateCcw size={15}/>恢复默认</button></div></div>
    <div className="setting-row recommended-font-row"><div><strong>推荐字体 · {RECOMMENDED_UI_FONT_NAME}</strong><span>三个字重约 25.44 MB；下载后离线使用，仅供 In-Line 使用</span>{downloading&&<div className="recommended-font-progress" role="status"><progress aria-label="推荐字体下载进度" value={progress.downloadedBytes} max={progress.totalBytes}/><small>{progress.phase==="verifying"?"正在校验字体…":"后台下载中"} · {(progress.downloadedBytes/1e6).toFixed(2)} / {(progress.totalBytes/1e6).toFixed(2)} MB · {progress.percent}%</small></div>}{ready&&<small className="recommended-font-note">已下载{recommendedSelected?"并启用":"，可直接启用"}；软件更新后无需重复下载</small>}{(progress.message||statusError)&&<small className="font-fallback-note" role="alert">{statusError||progress.message}</small>}</div><button type="button" className="button secondary" disabled={busy||downloading||(ready&&recommendedSelected)} onClick={()=>void (ready?choose(RECOMMENDED_UI_FONT_FAMILY):download())}><Download size={15}/>{downloading?"下载处理中…":ready?(recommendedSelected?"已启用":"使用推荐字体"):progress.phase==="error"?"重新下载并启用":"下载并启用"}</button></div>
    {open&&<div className="modal-layer nested-modal font-picker-layer" onMouseDown={event=>{if(event.target===event.currentTarget&&!busy)close();}}><section className="font-picker" role="dialog" aria-modal="true" aria-labelledby="font-picker-title"><header><div><h2 id="font-picker-title">软件字体</h2><small>系统字体 · CJK 优先 · 中英文名称均可搜索</small></div><button type="button" className="icon-button" disabled={busy} aria-label="关闭字体选择" onClick={close}><X size={18}/></button></header><label className="font-search"><Search size={16}/><input ref={search} value={query} placeholder="搜索字体名称，例如 YaHei、雅黑" aria-label="搜索字体名称" onChange={event=>setQuery(event.target.value)}/></label><div className="font-picker-list" role="listbox" aria-label="系统字体" aria-busy={loading||busy}><button type="button" role="option" aria-selected={!selection.effective} disabled={busy} onClick={()=>void choose("")}><span><strong>系统默认字体</strong><small>Microsoft YaHei UI / Segoe UI</small></span>{!selection.effective&&<Check size={17}/>}</button>{ready&&<button type="button" role="option" aria-selected={recommendedSelected} disabled={busy} onClick={()=>void choose(RECOMMENDED_UI_FONT_FAMILY)}><span><strong>{RECOMMENDED_UI_FONT_NAME}</strong><small>推荐字体 · 已下载 · 仅供 In-Line 使用</small></span>{recommendedSelected&&<Check size={17}/>}</button>}{loading?<p role="status">正在读取系统字体…</p>:error?<p role="alert">{error}</p>:results.map(font=><button type="button" role="option" aria-selected={selection.effective===font.family} key={font.family} disabled={busy} onClick={()=>void choose(font.family)}><span><strong>{font.displayName}</strong><small>{font.family}{font.cjk?" · CJK":""}</small></span>{selection.effective===font.family&&<Check size={17}/>}</button>)}{!loading&&!error&&!results.length&&<p>没有匹配的字体，请尝试其他名称。</p>}</div><footer><span>{loading?"枚举在后台执行":`${results.length} / ${fonts.length} 个系统字体`}</span><button type="button" className="button secondary small" disabled={busy} onClick={close}>取消</button></footer></section></div>}
  </>;
}
