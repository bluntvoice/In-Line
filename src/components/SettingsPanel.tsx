import { useEffect,useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { AlertTriangle,CheckCircle2,ChevronRight,Copy,DatabaseBackup,FileInput,FolderOpen,Keyboard,MonitorUp,Plug,RefreshCw,RotateCcw,Trash2,X } from "lucide-react";
import { api } from "../api";
import type { BackupConflictItem,BackupInfo,BackupMergeResult } from "../types";
import { backupFailureGuidance,prioritizeBackups } from "../lib/backup-ux";
import { shortcutFromKeyboardEvent,shortcutUsageHint } from "../lib/global-shortcut";

type McpDialog={title:string;summary:string;scenario:string;usage:string;content:string}|null;
type BackupDestination="queue"|"deferred"|"archive";
type BackupNotice={tone:"success"|"error";title:string;message:string;recommendation:string;result?:BackupMergeResult}|null;

export default function SettingsPanel({backups,settings,isDatabaseEmpty,onChanged,onOpenTask,onNavigateBackupResult,notify}:{backups:BackupInfo[];settings:Record<string,string>;isDatabaseEmpty:boolean;onChanged:()=>void;onOpenTask:(id:number)=>void;onNavigateBackupResult:(view:BackupDestination)=>void;notify:(text:string)=>void}){
  const [launch,setLaunch]=useState(false);
  const [weekStart,setWeekStart]=useState<"monday"|"sunday">(settings.week_start_day==="sunday"?"sunday":"monday");
  const [rateMode,setRateMode]=useState<"closure"|"processing">(settings.statistics_rate_mode==="closure"?"closure":"processing");
  const [shortcut,setShortcut]=useState(settings.global_shortcut??"Alt+I");
  const [shortcutRecording,setShortcutRecording]=useState(false);
  const [shortcutFeedback,setShortcutFeedback]=useState<{tone:"idle"|"checking"|"success"|"error";message:string}>({tone:"idle",message:"点击输入框，然后直接按下新的组合键"});
  const [visibleBackups,setVisibleBackups]=useState(backups);
  const [busy,setBusy]=useState("");
  const [mcpDialog,setMcpDialog]=useState<McpDialog>(null);
  const [importConflicts,setImportConflicts]=useState<BackupConflictItem[]>([]);
  const [highlightedBackup,setHighlightedBackup]=useState<string|null>(null);
  const [backupNotice,setBackupNotice]=useState<BackupNotice>(null);
  const [retryBackup,setRetryBackup]=useState<BackupInfo|null>(null);

  useEffect(()=>{void api.launchAtLogin().then(setLaunch);},[]);
  useEffect(()=>setWeekStart(settings.week_start_day==="sunday"?"sunday":"monday"),[settings]);
  useEffect(()=>setRateMode(settings.statistics_rate_mode==="closure"?"closure":"processing"),[settings]);
  useEffect(()=>setShortcut(settings.global_shortcut??"Alt+I"),[settings]);
  useEffect(()=>setVisibleBackups(prioritizeBackups(backups,highlightedBackup)),[backups,highlightedBackup]);
  useEffect(()=>{
    let active=true;
    const sync=()=>void api.listBackups().then(values=>{if(active)setVisibleBackups(prioritizeBackups(values,highlightedBackup));}).catch(()=>undefined);
    sync();
    const timer=window.setInterval(sync,2500);
    return()=>{active=false;window.clearInterval(timer);};
  },[highlightedBackup]);
  useEffect(()=>{
    if(!mcpDialog&&!importConflicts.length)return;
    const close=(event:KeyboardEvent)=>{if(event.key==="Escape"){setMcpDialog(null);setImportConflicts([]);}};
    window.addEventListener("keydown",close);
    return()=>window.removeEventListener("keydown",close);
  },[mcpDialog,importConflicts.length]);

  const refreshBackups=async(showMessage=false,pinnedPath=highlightedBackup)=>{
    const values=await api.listBackups();
    setVisibleBackups(prioritizeBackups(values,pinnedPath));
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
  const showRestoreSuccess=(result:BackupMergeResult)=>{
    setRetryBackup(null);
    setBackupNotice({
      tone:"success",
      title:"备份数据已成功导入",
      message:`新增 ${result.addedTasks} 项，合并 ${result.mergedTasks} 项，冲突保留 ${result.conflictTasks} 项。`,
      recommendation:"数据会按原状态分别显示在待办队列、暂缓事项和历史归档中，可直接跳转查看。",
      result
    });
  };
  const showBackupFailure=(error:unknown,stage:"import"|"restore",value:BackupInfo|null=null)=>{
    const guidance=backupFailureGuidance(error,stage);
    setRetryBackup(stage==="restore"?value:null);
    setBackupNotice({
      tone:"error",
      title:stage==="restore"?(value?.name.includes("-import")?"备份已导入，但数据恢复失败":"备份数据恢复失败"):"备份导入失败",
      message:`原因：${guidance.reason}`,
      recommendation:`建议：${guidance.recommendation}`
    });
  };
  const mergeBackup=async(value:BackupInfo)=>{
    setBusy(value.path);
    try{
      const result=await api.restoreBackup(value.path);
      showRestoreSuccess(result);
      if(result.conflicts.length)setImportConflicts(result.conflicts);
      const pinnedPath=value.name.includes("-import")?value.path:highlightedBackup;
      await refreshBackups(false,pinnedPath).catch(()=>undefined);
      onChanged();
    }catch(error){
      showBackupFailure(error,"restore",value);
      await refreshBackups(false,highlightedBackup).catch(()=>undefined);
    }finally{setBusy("");}
  };
  const importAndRestore=async()=>{
    try{
      const selected=await open({multiple:false,directory:false,filters:[{name:"In Line 数据库备份",extensions:["db"]}]});
      const path=Array.isArray(selected)?selected[0]:selected;
      if(!path)return;
      if(!window.confirm(isDatabaseEmpty?"将校验所选备份并恢复到当前空数据库。恢复前仍会自动创建安全备份，是否继续？":"将校验所选备份并与当前数据安全合并：相同事项合并记录，不同内容保留为冲突事项，备份中的软件设置会覆盖当前设置。恢复前会自动备份当前数据，是否继续？"))return;
      setBusy("import");
      const value=await api.importBackup(path);
      setHighlightedBackup(value.path);
      setVisibleBackups(current=>prioritizeBackups([value,...current],value.path));
      setBusy("");
      await mergeBackup(value);
    }catch(error){showBackupFailure(error,"import");setBusy("");}
  };
  const restore=async(value:BackupInfo)=>{
    if(!window.confirm("将所选备份与当前数据合并：相同事项合并记录，不同内容保留为“冲突”事项；备份中的软件设置会覆盖当前设置。系统会先自动备份当前数据，是否继续？"))return;
    await mergeBackup(value);
  };
  const remove=async(value:BackupInfo)=>{
    if(!window.confirm(`确定删除备份“${value.name}”吗？删除后无法恢复。`))return;
    setBusy(value.path);
    try{await api.deleteBackup(value.path);if(highlightedBackup===value.path)setHighlightedBackup(null);notify("备份已删除");await refreshBackups(false,highlightedBackup===value.path?null:highlightedBackup);}
    catch(error){notify("删除失败："+String(error));}
    finally{setBusy("");}
  };
  const saveWeekStart=async(value:"monday"|"sunday")=>{try{await api.setSetting("week_start_day",value);setWeekStart(value);}catch(error){notify("设置保存失败："+String(error));}};
  const saveRateMode=async(value:"closure"|"processing")=>{try{await api.setSetting("statistics_rate_mode",value);setRateMode(value);}catch(error){notify("设置保存失败："+String(error));}};
  const saveShortcut=async(value:string)=>{setBusy("shortcut");setShortcutFeedback({tone:"checking",message:`正在检查 ${value} 是否可用…`});try{await api.setGlobalShortcut(value);setShortcut(value);setShortcutFeedback({tone:"success",message:shortcutUsageHint(value)});notify(`全局快捷键已更新为 ${value}`);onChanged();}catch(error){const message=String(error);setShortcutFeedback({tone:"error",message:`存在冲突：${message}`});notify("快捷键设置失败："+message);}finally{setBusy("");setShortcutRecording(false);}};
  const captureShortcut=(event:React.KeyboardEvent<HTMLInputElement>)=>{
    event.preventDefault();event.stopPropagation();
    if(event.key==="Escape"){setShortcutRecording(false);setShortcutFeedback({tone:"idle",message:"已取消录入，继续使用原快捷键"});event.currentTarget.blur();return;}
    const result=shortcutFromKeyboardEvent(event.nativeEvent);
    if(result.error){setShortcutFeedback({tone:"error",message:result.error});return;}
    if(!result.shortcut){setShortcutFeedback({tone:"idle",message:"请继续按下字母、数字或功能键"});return;}
    void saveShortcut(result.shortcut);
  };
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
    <div className="setting-row shortcut-setting-row"><div><strong>全局快捷新增</strong><span>直接录入自定义组合；注册成功才会保存，发生系统占用时继续使用原快捷键</span></div><div className="shortcut-editor"><label className={`shortcut-recorder ${shortcutRecording?"recording":""}`}><Keyboard size={16}/><input readOnly value={busy==="shortcut"?"正在检查…":shortcut} disabled={busy==="shortcut"} onFocus={()=>{setShortcutRecording(true);setShortcutFeedback({tone:"idle",message:"请按住 Ctrl 或 Alt，再按一个字母、数字或功能键；Esc 取消"});}} onBlur={()=>setShortcutRecording(false)} onKeyDown={captureShortcut} aria-label="录入全局快捷新增组合"/></label><button type="button" className="button secondary small" disabled={busy==="shortcut"||shortcut==="Alt+I"} onClick={()=>void saveShortcut("Alt+I")}>恢复默认</button><p className={`shortcut-feedback ${shortcutFeedback.tone}`}>{shortcutFeedback.tone==="success"?<CheckCircle2 size={14}/>:shortcutFeedback.tone==="error"?<AlertTriangle size={14}/>:<Keyboard size={14}/>}<span>{shortcutFeedback.message}</span></p></div></div>
    <div className="setting-row"><div><strong>开机自动启动</strong><span>登录 Windows 后启动 In Line</span></div><label className="switch"><input type="checkbox" checked={launch} onChange={async event=>{const value=event.target.checked;await api.setLaunchAtLogin(value);setLaunch(value);}}/><span/></label></div>
    <div className="setting-row"><div><strong>AI MCP 接入</strong><span>复制通用接入信息，可直接交给 Codex 等 AI 客户端完成配置</span></div><div className="mcp-actions"><button className="button secondary" disabled={busy==="mcp"} onClick={()=>void showMcpContent({title:"通用 MCP 接入",summary:"一份适用于 stdio MCP 客户端的接入指令，包含本机程序路径、工具清单和只读权限范围。",scenario:"首次在 Codex 等 AI 客户端接入 In Line，安装路径改变后重新配置，或排查 MCP 启动问题时使用。",usage:"复制后交给目标客户端，按其中的启动命令完成接入。"},api.mcpConnectionGuide)}><Plug size={16}/>通用接入</button></div></div>
    <div className="setting-row"><div><strong>数据备份</strong><span>事项、办理记录和软件设置会统一写入本地数据库备份</span></div><button className="button secondary" disabled={busy!==""} onClick={()=>void backup()}><DatabaseBackup size={16}/>{busy==="backup"?"备份中…":"立即备份"}</button></div>
    <div className="backup-list">
      <div className="backup-list-header"><div><h2>可恢复备份 <span>{visibleBackups.length}</span></h2><small>列表显示全部备份；新导入文件会置顶并高亮。</small></div><div className="backup-toolbar"><button className="button primary" disabled={busy!==""} onClick={()=>void importAndRestore()}><FileInput size={16}/>{busy==="import"?"正在校验…":"导入并恢复"}</button><button className="button secondary" disabled={busy!==""} onClick={()=>void manualRefresh()}><RefreshCw className={busy==="refresh"?"spin":""} size={16}/>{busy==="refresh"?"刷新中…":"刷新列表"}</button><button className="button secondary" onClick={()=>void api.openBackupDirectory().catch(error=>notify("打开备份目录失败："+String(error)))}><FolderOpen size={16}/>备份目录</button></div></div>
      {isDatabaseEmpty&&<div className="backup-empty-guide"><DatabaseBackup size={18}/><div><strong>当前数据库为空</strong><span>选择备份后将自动校验并恢复，恢复前仍会创建安全备份。</span></div></div>}
      {visibleBackups.map(value=><article key={value.path} className={highlightedBackup===value.path?"latest-import":""}><div><strong>{value.name}{highlightedBackup===value.path&&<em>刚刚导入</em>}</strong><span>{new Date(value.modifiedAt).toLocaleString("zh-CN")} · {(value.size/1024).toFixed(0)} KB</span></div><div className="backup-actions"><button className="button secondary small restore-backup-button" disabled={busy!==""} aria-busy={busy===value.path} onClick={()=>void restore(value)}><RotateCcw className={busy===value.path?"spin":""} size={15}/>{busy===value.path?"恢复中…":"恢复数据"}</button><button className="icon-button danger" disabled={busy!==""} onClick={()=>void remove(value)} title="删除此备份" aria-label={`删除备份 ${value.name}`}><Trash2 size={16}/></button></div></article>)}{!visibleBackups.length&&<p className="muted">暂无备份，可立即备份或使用“导入并恢复”。</p>}
    </div>
    {mcpDialog&&<div className="modal-layer nested-modal" role="presentation" onMouseDown={event=>{if(event.target===event.currentTarget)setMcpDialog(null);}}><section className="mcp-content-dialog" role="dialog" aria-modal="true" aria-labelledby="mcp-dialog-title"><header><div><span>AI MCP 接入</span><h2 id="mcp-dialog-title">{mcpDialog.title}</h2></div><button className="icon-button" onClick={()=>setMcpDialog(null)} aria-label="关闭"><X size={18}/></button></header><div className="mcp-content-body"><dl className="mcp-content-help"><div><dt>简要解释</dt><dd>{mcpDialog.summary}</dd></div><div><dt>适用场景</dt><dd>{mcpDialog.scenario}</dd></div><div><dt>怎么使用</dt><dd>{mcpDialog.usage}</dd></div></dl><textarea readOnly value={mcpDialog.content} aria-label={mcpDialog.title}/></div><footer><button className="button secondary" onClick={()=>setMcpDialog(null)}>关闭</button><button className="button primary" onClick={()=>void copyMcpContent()}><Copy size={16}/>复制内容</button></footer></section></div>}
    {importConflicts.length>0&&<div className="modal-layer nested-modal" role="presentation"><section className="import-conflict-dialog" role="dialog" aria-modal="true" aria-labelledby="import-conflict-title"><header><div><span>备份合并完成</span><h2 id="import-conflict-title"><AlertTriangle size={19}/>发现 {importConflicts.length} 项导入冲突</h2></div><button className="icon-button" onClick={()=>setImportConflicts([])} aria-label="关闭"><X size={18}/></button></header><p>这些备份事项与当前数据同名但内容不同，已安全保留并添加“导入冲突”标识。请逐项查看后合并，或在确认无需处理时解除标识。</p><div className="import-conflict-list">{importConflicts.map(item=><button type="button" key={item.taskId} onClick={()=>{setImportConflicts([]);onOpenTask(item.taskId);}}><span><strong>{item.importedTitle}</strong><small>{item.permanentNumber} · 原始标题：{item.sourceTitle}</small></span><ChevronRight size={17}/></button>)}</div><footer><button className="button primary" onClick={()=>setImportConflicts([])}>知道了，稍后处理</button></footer></section></div>}
    {backupNotice&&<aside className={`backup-result-notice ${backupNotice.tone}`} role={backupNotice.tone==="error"?"alert":"status"} aria-live="polite"><header><span>{backupNotice.tone==="success"?<CheckCircle2 size={19}/>:<AlertTriangle size={19}/>}</span><div><small>{backupNotice.tone==="success"?"备份恢复完成":"需要处理"}</small><strong>{backupNotice.title}</strong></div><button type="button" onClick={()=>setBackupNotice(null)} aria-label="关闭通知"><X size={16}/></button></header><p>{backupNotice.message}</p><small>{backupNotice.recommendation}</small><footer>{backupNotice.tone==="success"?<><button type="button" onClick={()=>onNavigateBackupResult("queue")}>查看待办</button><button type="button" onClick={()=>onNavigateBackupResult("deferred")}>查看暂缓</button><button type="button" onClick={()=>onNavigateBackupResult("archive")}>查看归档</button></>:retryBackup&&<button type="button" disabled={busy!==""} onClick={()=>void mergeBackup(retryBackup)}><RotateCcw size={14}/>重试恢复</button>}</footer></aside>}
  </section>;
}
