import {useEffect,useRef,useState,type ReactNode} from "react";
import {api} from "../api";
import type {McpStorageState} from "../lib/mcp-onboarding";

export default function McpStorageGate({children}:{children:ReactNode}){
  const [state,setState]=useState<McpStorageState|null>(null),[error,setError]=useState("");
  const [confirmed,setConfirmed]=useState(false),[busy,setBusy]=useState(false),[finished,setFinished]=useState(false);
  const running=useRef(false);
  const load=async()=>{setState(null);setError("");setConfirmed(false);try{setState(await api.mcpStorageState());}catch(e){setError(String(e));}};
  useEffect(()=>{let active=true;void api.mcpStorageState().then(s=>{if(active)setState(s);}).catch(e=>{if(active)setError(String(e));});return()=>{active=false;};},[]);
  if(state?.ready)return <>{children}</>;
  const migrate=async()=>{
    if(running.current||!confirmed||!state?.canMigrate||!state.legacyFileIdentity)return;
    running.current=true;setBusy(true);setError("");
    try{await api.mcpMigrateStorage(state.legacyFileIdentity,true);setFinished(true);}
    catch(e){setError(String(e));setConfirmed(false);}
    finally{running.current=false;setBusy(false);}
  };
  return <section className="setting-row"><div>
    <strong>AI 接入存储升级</strong>
    {!state&&!error&&<p role="status">正在检查安全存储…</p>}
    {state&&<><p>为让桌面软件和 AI 客户端读取同一份授权，需要先迁移旧版安全存储。迁移会保留原文件、现有权限和撤销状态；不会新增授权或导入客户端配置。</p>
    <p>来源：{state.legacyRoot??"未发现旧存储"}<br/>目标：{state.root}</p>
    {state.legacyRedirected&&<p role="alert">当前启动环境读取的是隔离副本，已停止迁移。请从桌面打开 In Line 后再检查。</p>}
    {state.errorCode&&<p role="alert">存储状态需要核对：{state.errorCode}。未覆盖或合并文件。</p>}
    {state.canMigrate&&<><label className="mcp-connect-confirm"><input type="checkbox" checked={confirmed} disabled={busy||finished} onChange={e=>setConfirmed(e.target.checked)}/><span>我已保存当前工作，确认迁移并重新启动 In Line</span></label><button className="button secondary" disabled={!confirmed||busy||finished} onClick={()=>void migrate()}>{busy?"正在迁移…":"迁移并重启软件"}</button></>}
    </>}
    {error&&<p role="alert">{error}</p>}
    {finished&&<p role="status">迁移完成，原文件已保留，软件即将重新启动。AI 当前会话连接仍需单独验证。</p>}
    {!busy&&!finished&&<button className="button secondary" onClick={()=>void load()}>重新检查存储</button>}
  </div></section>;
}
