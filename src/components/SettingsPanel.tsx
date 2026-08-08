import { useEffect,useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { AlertTriangle,ChevronRight,Copy,DatabaseBackup,FileInput,FolderOpen,MonitorUp,Plug,RefreshCw,RotateCcw,Trash2,X } from "lucide-react";
import { api } from "../api";
import type { BackupConflictItem,BackupInfo } from "../types";

type McpDialog={title:string;summary:string;scenario:string;usage:string;content:string}|null;

export default function SettingsPanel({backups,settings,onChanged,onOpenTask,notify}:{backups:BackupInfo[];settings:Record<string,string>;onChanged:()=>void;onOpenTask:(id:number)=>void;notify:(text:string)=>void}){
  const [launch,setLaunch]=useState(false);
  const [weekStart,setWeekStart]=useState<"monday"|"sunday">(settings.week_start_day==="sunday"?"sunday":"monday");
  const [rateMode,setRateMode]=useState<"closure"|"processing">(settings.statistics_rate_mode==="closure"?"closure":"processing");
  const [visibleBackups,setVisibleBackups]=useState(backups);
  const [busy,setBusy]=useState("");
  const [mcpDialog,setMcpDialog]=useState<McpDialog>(null);
  const [importConflicts,setImportConflicts]=useState<BackupConflictItem[]>([]);

  useEffect(()=>{void api.launchAtLogin().then(setLaunch);},[]);
  useEffect(()=>setWeekStart(settings.week_start_day==="sunday"?"sunday":"monday"),[settings]);
  useEffect(()=>setRateMode(settings.statistics_rate_mode==="closure"?"closure":"processing"),[settings]);
  useEffect(()=>setVisibleBackups(backups),[backups]);
  useEffect(()=>{
    let active=true;
    const sync=()=>void api.listBackups().then(values=>{if(active)setVisibleBackups(values);}).catch(()=>undefined);
    sync();
    const timer=window.setInterval(sync,2500);
    return()=>{active=false;window.clearInterval(timer);};
  },[]);
  useEffect(()=>{
    if(!mcpDialog&&!importConflicts.length)return;
    const close=(event:KeyboardEvent)=>{if(event.key==="Escape"){setMcpDialog(null);setImportConflicts([]);}};
    window.addEventListener("keydown",close);
    return()=>window.removeEventListener("keydown",close);
  },[mcpDialog,importConflicts.length]);

  const refreshBackups=async(showMessage=false)=>{
    const values=await api.listBackups();
    setVisibleBackups(values);
    if(showMessage)notify("备份列表已刷新");
  };
  const manualRefresh=async()=>{
    setBusy("refresh");
    try{await refreshBackups(true);}
    catch(error){notify("刷新失败："+String(error));}
    finally{setBusy("");}
  };
  const backup=async()=>{
    setBusy("backup");
    try{
      const value=await api.createBackup();
      notify("备份完成："+value.name);
      await refreshBackups();
    }catch(error){notify("备份失败："+String(error));}
    finally{setBusy("");}
  };
  const importBackup=async()=>{
    try{
      const selected=await open({multiple:false,directory:false,filters:[{name:"In Line 数据库备份",extensions:["db"]}]});
      const path=Array.isArray(selected)?selected[0]:selected;
      if(!path)return;
      setBusy("import");
      const value=await api.importBackup(path);
      notify("备份已导入："+value.name);
      await refreshBackups();
    }catch(error){notify("导入失败："+String(error));}
    finally{setBusy("");}
  };
  const restore=async(value:BackupInfo)=>{
    if(!window.confirm("将所选备份与当前数据合并：相同事项合并记录，不同内容保留为“冲突”事项；备份中的软件设置会覆盖当前设置。系统会先自动备份当前数据，是否继续？"))return;
    setBusy(value.path);
    try{
      const result=await api.restoreBackup(value.path);
      notify(`合并完成：新增 ${result.addedTasks} 项，合并 ${result.mergedTasks} 项，冲突保留 ${result.conflictTasks} 项`);
      if(result.conflicts.length)setImportConflicts(result.conflicts);
      await refreshBackups();
      onChanged();
    }catch(error){notify("恢复失败："+String(error));}
    finally{setBusy("");}
  };
  const remove=async(value:BackupInfo)=>{
    if(!window.confirm(`确定删除备份“${value.name}”吗？删除后无法恢复。`))return;
    setBusy(value.path);
    try{await api.deleteBackup(value.path);notify("备份已删除");await refreshBackups();}
    catch(error){notify("删除失败："+String(error));}
    finally{setBusy("");}
  };
  const saveWeekStart=async(value:"monday"|"sunday")=>{try{await api.setSetting("week_start_day",value);setWeekStart(value);}catch(error){notify("设置保存失败："+String(error));}};
  const saveRateMode=async(value:"closure"|"processing")=>{try{await api.setSetting("statistics_rate_mode",value);setRateMode(value);}catch(error){notify("设置保存失败："+String(error));}};
  const showMcpContent=async(dialog:Omit<NonNullable<McpDialog>,"content">,load:()=>Promise<string>)=>{
    setBusy("mcp");
    try{setMcpDialog({...dialog,content:await load()});}
    catch(error){notify("读取指令失败："+String(error));}
    finally{setBusy("");}
  };
  const copyMcpContent=async()=>{
    if(!mcpDialog)return;
    try{await api.copyText(mcpDialog.content);notify(mcpDialog.title+"已复制");}
    catch(error){notify("复制失败："+String(error));}
  };

  return <section className="settings-page"><h1>软件设置</h1><p>所有事项与备份均保存在本机，不上传数据。</p>
    <div className="setting-row"><div><strong>桌面悬浮窗</strong><span>关闭主界面后默认显示，也可在此手动显示或隐藏</span></div><button className="button secondary" onClick={()=>void api.toggleFloating()}><MonitorUp size={16}/>显示 / 隐藏</button></div>
    <div className="setting-row"><div><strong>每周起始日</strong><span>用于统计中心“本周”和“上一周”的日期范围</span></div><div className="week-start-options" role="group" aria-label="每周起始日"><button type="button" className={weekStart==="monday"?"active":""} onClick={()=>void saveWeekStart("monday")}>周一</button><button type="button" className={weekStart==="sunday"?"active":""} onClick={()=>void saveWeekStart("sunday")}>周日</button></div></div>
    <div className="setting-row"><div><strong>统计比例口径</strong><span>{rateMode==="processing"?"有效处理率：有效办理事项 ÷ 周期内应处理事项":"事项办结率：已完成事项 ÷ 周期内实际处理事项"}</span></div><div className="week-start-options rate-mode-options" role="group" aria-label="统计比例口径"><button type="button" className={rateMode==="processing"?"active":""} onClick={()=>void saveRateMode("processing")}>有效处理率</button><button type="button" className={rateMode==="closure"?"active":""} onClick={()=>void saveRateMode("closure")}>事项办结率</button></div></div>
    <div className="setting-row"><div><strong>开机自动启动</strong><span>登录 Windows 后启动 In Line</span></div><label className="switch"><input type="checkbox" checked={launch} onChange={async event=>{const value=event.target.checked;await api.setLaunchAtLogin(value);setLaunch(value);}}/><span/></label></div>
    <div className="setting-row"><div><strong>AI MCP 接入</strong><span>复制通用接入信息，可直接交给 Codex 等 AI 客户端完成配置</span></div><div className="mcp-actions"><button className="button secondary" disabled={busy==="mcp"} onClick={()=>void showMcpContent({title:"通用 MCP 接入",summary:"一份适用于 stdio MCP 客户端的接入指令，包含本机程序路径、工具清单和只读权限范围。",scenario:"首次在 Codex 等 AI 客户端接入 In Line，安装路径改变后重新配置，或排查 MCP 启动问题时使用。",usage:"复制后交给目标客户端，按其中的启动命令完成接入。"},api.mcpConnectionGuide)}><Plug size={16}/>通用接入</button></div></div>
    <div className="setting-row"><div><strong>数据备份</strong><span>事项、办理记录和软件设置会统一写入本地数据库备份</span></div><button className="button secondary" disabled={busy!==""} onClick={()=>void backup()}><DatabaseBackup size={16}/>{busy==="backup"?"备份中…":"立即备份"}</button></div>
    <div className="backup-list"><div className="backup-list-header"><h2>可恢复备份</h2><div className="backup-toolbar"><button className="button secondary" disabled={busy!==""} onClick={()=>void importBackup()}><FileInput size={16}/>{busy==="import"?"导入中…":"导入备份"}</button><button className="button secondary" disabled={busy!==""} onClick={()=>void manualRefresh()}><RefreshCw className={busy==="refresh"?"spin":""} size={16}/>{busy==="refresh"?"刷新中…":"刷新"}</button><button className="button secondary" onClick={()=>void api.openBackupDirectory().catch(error=>notify("打开备份目录失败："+String(error)))}><FolderOpen size={16}/>备份目录</button></div></div>{visibleBackups.slice(0,12).map(value=><article key={value.path}><div><strong>{value.name}</strong><span>{new Date(value.modifiedAt).toLocaleString("zh-CN")} · {(value.size/1024).toFixed(0)} KB</span></div><div className="backup-actions"><button className="icon-button" disabled={busy!==""} aria-busy={busy===value.path} onClick={()=>void restore(value)} title="合并此备份" aria-label={`合并备份 ${value.name}`}><RotateCcw className={busy===value.path?"spin":""} size={16}/></button><button className="icon-button danger" disabled={busy!==""} onClick={()=>void remove(value)} title="删除此备份" aria-label={`删除备份 ${value.name}`}><Trash2 size={16}/></button></div></article>)}{!visibleBackups.length&&<p className="muted">暂无备份，可立即备份或导入 .db 文件。</p>}</div>
    {mcpDialog&&<div className="modal-layer nested-modal" role="presentation" onMouseDown={event=>{if(event.target===event.currentTarget)setMcpDialog(null);}}><section className="mcp-content-dialog" role="dialog" aria-modal="true" aria-labelledby="mcp-dialog-title"><header><div><span>AI MCP 接入</span><h2 id="mcp-dialog-title">{mcpDialog.title}</h2></div><button className="icon-button" onClick={()=>setMcpDialog(null)} aria-label="关闭"><X size={18}/></button></header><div className="mcp-content-body"><dl className="mcp-content-help"><div><dt>简要解释</dt><dd>{mcpDialog.summary}</dd></div><div><dt>适用场景</dt><dd>{mcpDialog.scenario}</dd></div><div><dt>怎么使用</dt><dd>{mcpDialog.usage}</dd></div></dl><textarea readOnly value={mcpDialog.content} aria-label={mcpDialog.title}/></div><footer><button className="button secondary" onClick={()=>setMcpDialog(null)}>关闭</button><button className="button primary" onClick={()=>void copyMcpContent()}><Copy size={16}/>复制内容</button></footer></section></div>}
    {importConflicts.length>0&&<div className="modal-layer nested-modal" role="presentation"><section className="import-conflict-dialog" role="dialog" aria-modal="true" aria-labelledby="import-conflict-title"><header><div><span>备份合并完成</span><h2 id="import-conflict-title"><AlertTriangle size={19}/>发现 {importConflicts.length} 项导入冲突</h2></div><button className="icon-button" onClick={()=>setImportConflicts([])} aria-label="关闭"><X size={18}/></button></header><p>这些备份事项与当前数据同名但内容不同，已安全保留并添加“导入冲突”标识。请逐项查看后合并，或在确认无需处理时解除标识。</p><div className="import-conflict-list">{importConflicts.map(item=><button type="button" key={item.taskId} onClick={()=>{setImportConflicts([]);onOpenTask(item.taskId);}}><span><strong>{item.importedTitle}</strong><small>{item.permanentNumber} · 原始标题：{item.sourceTitle}</small></span><ChevronRight size={17}/></button>)}</div><footer><button className="button primary" onClick={()=>setImportConflicts([])}>知道了，稍后处理</button></footer></section></div>}
  </section>;
}
