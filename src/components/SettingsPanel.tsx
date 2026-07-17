import { DatabaseBackup,MonitorUp,RotateCcw } from "lucide-react";
import { useEffect,useState } from "react";
import { api } from "../api";
import type { BackupInfo } from "../types";

export default function SettingsPanel({backups,onChanged,notify}:{backups:BackupInfo[];onChanged:()=>void;notify:(text:string)=>void}){
  const [launch,setLaunch]=useState(false);
  useEffect(()=>{void api.launchAtLogin().then(setLaunch);},[]);
  const backup=async()=>{const value=await api.createBackup();notify("备份完成："+value.name);onChanged();};
  const restore=async(value:BackupInfo)=>{
    if(!window.confirm("恢复该备份会替换当前数据。系统将先自动备份当前数据库，是否继续？"))return;
    await api.restoreBackup(value.path);notify("数据恢复完成");onChanged();
  };
  return <section className="settings-page"><h1>系统设置</h1><p>所有事项与备份均保存在本机，不上传数据。</p>
    <div className="setting-row"><div><strong>桌面悬浮窗</strong><span>显示始终置顶的紧凑队列</span></div><button className="button secondary" onClick={()=>void api.toggleFloating()}><MonitorUp size={16}/>显示 / 隐藏</button></div>
    <div className="setting-row"><div><strong>开机自动启动</strong><span>登录 Windows 后启动 In Line</span></div><label className="switch"><input type="checkbox" checked={launch} onChange={async event=>{const value=event.target.checked;await api.setLaunchAtLogin(value);setLaunch(value);}}/><span/></label></div>
    <div className="setting-row"><div><strong>数据备份</strong><span>生成一致的 SQLite 在线备份</span></div><button className="button secondary" onClick={()=>void backup()}><DatabaseBackup size={16}/>立即备份</button></div>
    <div className="backup-list"><h2>可恢复备份</h2>{backups.slice(0,12).map(value=><article key={value.path}><div><strong>{value.name}</strong><span>{new Date(value.modifiedAt).toLocaleString("zh-CN")} · {(value.size/1024).toFixed(0)} KB</span></div><button className="icon-button" onClick={()=>void restore(value)} title="恢复此备份"><RotateCcw size={16}/></button></article>)}{!backups.length&&<p className="muted">暂无备份</p>}</div>
  </section>;
}
