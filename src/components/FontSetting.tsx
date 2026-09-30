import {useEffect,useMemo,useRef,useState} from "react";
import {Check,ChevronDown,RotateCcw,Search,X} from "lucide-react";
import {api} from "../api";
import type {SystemFont} from "../types";
import {filterSystemFonts} from "../lib/ui-font";
import {useUIFont} from "./UIFontProvider";

let fontsRequest:Promise<SystemFont[]>|null=null;
const loadFonts=()=>fontsRequest??(fontsRequest=api.listSystemFonts().catch(error=>{fontsRequest=null;throw error;}));

export default function FontSetting({notify}:{notify:(text:string)=>void}){
  const {selection,unavailable}=useUIFont();
  const [open,setOpen]=useState(false),[query,setQuery]=useState("");
  const [fonts,setFonts]=useState<SystemFont[]>([]),[loading,setLoading]=useState(false),[busy,setBusy]=useState(false),[error,setError]=useState("");
  const current=fonts.find(font=>font.family===selection.effective);
  const results=useMemo(()=>filterSystemFonts(fonts,query),[fonts,query]);
  const trigger=useRef<HTMLButtonElement>(null);
  const search=useRef<HTMLInputElement>(null);
  useEffect(()=>{
    if(!open)return;
    let active=true;
    setLoading(true);setError("");
    void loadFonts().then(values=>{if(active)setFonts(values);}).catch(value=>{if(active)setError(String(value));}).finally(()=>{if(active)setLoading(false);});
    search.current?.focus();
    return()=>{active=false;};
  },[open]);
  const close=()=>{setOpen(false);trigger.current?.focus();};
  useEffect(()=>{
    if(!open)return;
    const escape=(event:KeyboardEvent)=>{if(event.key==="Escape"&&!busy)close();};
    window.addEventListener("keydown",escape);return()=>window.removeEventListener("keydown",escape);
  },[open,busy]);
  const choose=async(family:string)=>{
    setBusy(true);setError("");
    try{await api.setSetting("ui_font_family",family);close();notify(family?"软件字体已更新，所有窗口即时生效":"已恢复默认字体");}
    catch(value){setError(String(value));notify("设置字体失败："+String(value));}
    finally{setBusy(false);}
  };
  return <>
    <div className="setting-row font-setting-row"><div><strong>软件字体</strong><span>读取本机已安装字体；选择后全局即时生效</span>{selection.missing&&<small className="font-fallback-note">原字体“{selection.requested}”不可用，当前使用默认字体。</small>}{unavailable&&<small className="font-fallback-note">暂时无法读取字体设置，当前安全使用默认字体。</small>}</div><div className="font-setting-actions"><button ref={trigger} type="button" className="button secondary font-setting-trigger" disabled={busy} aria-haspopup="dialog" aria-expanded={open} onClick={()=>{setQuery("");setOpen(true);}}>{current?.displayName||selection.effective||"默认字体"}<ChevronDown size={15}/></button><button type="button" className="button secondary small" disabled={busy||(!selection.requested&&!unavailable)} onClick={()=>void choose("")}><RotateCcw size={15}/>恢复默认</button></div></div>
    {open&&<div className="modal-layer nested-modal font-picker-layer" onMouseDown={event=>{if(event.target===event.currentTarget&&!busy)close();}}><section className="font-picker" role="dialog" aria-modal="true" aria-labelledby="font-picker-title"><header><div><h2 id="font-picker-title">软件字体</h2><small>本机可用字体 · CJK 优先 · 中英文名称均可搜索</small></div><button type="button" className="icon-button" disabled={busy} aria-label="关闭字体选择" onClick={close}><X size={18}/></button></header><label className="font-search"><Search size={16}/><input ref={search} value={query} placeholder="搜索字体名称，例如 YaHei、雅黑" aria-label="搜索字体名称" onChange={event=>setQuery(event.target.value)}/></label><div className="font-picker-list" role="listbox" aria-label="系统字体" aria-busy={loading||busy}><button type="button" role="option" aria-selected={!selection.effective} disabled={busy} onClick={()=>void choose("")}><span><strong>默认字体</strong><small>恢复 In Line 原有字体策略</small></span>{!selection.effective&&<Check size={17}/>}</button>{loading?<p role="status">正在读取系统字体…</p>:error?<p role="alert">{error}</p>:results.map(font=><button type="button" role="option" aria-selected={selection.effective===font.family} key={font.family} disabled={busy} onClick={()=>void choose(font.family)}><span><strong>{font.displayName}</strong><small>{font.family}{font.cjk?" · CJK":""}</small></span>{selection.effective===font.family&&<Check size={17}/>}</button>)}{!loading&&!error&&!results.length&&<p>没有匹配的字体，请尝试其他名称。</p>}</div><footer><span>{loading?"枚举在后台执行":`${results.length} / ${fonts.length} 个字体系列`}</span><button type="button" className="button secondary small" disabled={busy} onClick={close}>取消</button></footer></section></div>}
  </>;
}
