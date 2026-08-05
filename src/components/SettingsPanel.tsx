import { DatabaseBackup,MonitorUp,RotateCcw,Trash2 } from "lucide-react";
import { useEffect,useState } from "react";
import { api } from "../api";
import type { BackupInfo } from "../types";

export default function SettingsPanel({backups,settings,onChanged,notify}:{backups:BackupInfo[];settings:Record<string,string>;onChanged:()=>void;notify:(text:string)=>void}){
  const [launch,setLaunch]=useState(false);
  const [showDeferred,setShowDeferred]=useState(settings.show_deferred_in_queue==="true");
  const [weekStart,setWeekStart]=useState<"monday"|"sunday">(settings.week_start_day==="sunday"?"sunday":"monday");
  useEffect(()=>{void api.launchAtLogin().then(setLaunch);},[]);
  useEffect(()=>setShowDeferred(settings.show_deferred_in_queue==="true"),[settings]);
  useEffect(()=>setWeekStart(settings.week_start_day==="sunday"?"sunday":"monday"),[settings]);
  const saveWeekStart=async(value:"monday"|"sunday")=>{const previous=weekStart;setWeekStart(value);try{await api.setSetting("week_start_day",value);}catch(error){setWeekStart(previous);notify("设置保存失败："+String(error));}};
  const backup=async()=>{const value=await api.createBackup();notify("备份完成："+value.name);onChanged();};
  const restore=async(value:BackupInfo)=>{
    if(!window.confirm("恢复该备份会替换当前数据。系统将先自动备份当前数据库，是否继续？"))return;
    await api.restoreBackup(value.path);notify("数据恢复完成");onChanged();
  };
  const remove=async(value:BackupInfo)=>{
    if(!window.confirm(`确定删除备份“${value.name}”吗？删除后无法恢复。`))return;
    await api.deleteBackup(value.path);notify("备份已删除");onChanged();
  };
  return <section className="settings-page"><h1>系统设置</h1><p>所有事项与备份均保存在本机，不上传数据。</p>
    <div className="setting-row"><div><strong>桌面悬浮窗</strong><span>显示始终置顶的紧凑队列</span></div><button className="button secondary" onClick={()=>void api.toggleFloating()}><MonitorUp size={16}/>显示 / 隐藏</button></div>
    <div className="setting-row"><div><strong>显示暂缓事项</strong><span>在待办队列和悬浮窗中同时显示暂缓事项</span></div><label className="switch"><input type="checkbox" checked={showDeferred} onChange={async event=>{const value=event.target.checked;try{await api.setSetting("show_deferred_in_queue",value);setShowDeferred(value);}catch(error){notify("设置保存失败："+String(error));}}}/><span/></label></div>
    <div className="setting-row"><div><strong>每周起始日</strong><span>用于统计中心“本周”和“上一周”的日期范围</span></div><div className="week-start-options" role="group" aria-label="每周起始日"><button type="button" className={weekStart==="monday"?"active":""} onClick={()=>void saveWeekStart("monday")}>周一</button><button type="button" className={weekStart==="sunday"?"active":""} onClick={()=>void saveWeekStart("sunday")}>周日</button></div></div>
    <div className="setting-row"><div><strong>开机自动启动</strong><span>登录 Windows 后启动 In Line</span></div><label className="switch"><input type="checkbox" checked={launch} onChange={async event=>{const value=event.target.checked;await api.setLaunchAtLogin(value);setLaunch(value);}}/><span/></label></div>
    <div className="setting-row"><div><strong>数据备份</strong><span>备份统一命名为 InLine-backup-日期时间-类型.db</span></div><button className="button secondary" onClick={()=>void backup()}><DatabaseBackup size={16}/>立即备份</button></div>
    <div className="backup-list"><h2>可恢复备份</h2>{backups.slice(0,12).map(value=><article key={value.path}><div><strong>{value.name}</strong><span>{new Date(value.modifiedAt).toLocaleString("zh-CN")} · {(value.size/1024).toFixed(0)} KB</span></div><div className="backup-actions"><button className="icon-button" onClick={()=>void restore(value)} title="恢复此备份" aria-label={`恢复备份 ${value.name}`}><RotateCcw size={16}/></button><button className="icon-button danger" onClick={()=>void remove(value)} title="删除此备份" aria-label={`删除备份 ${value.name}`}><Trash2 size={16}/></button></div></article>)}{!backups.length&&<p className="muted">暂无备份</p>}</div>
  </section>;
}
