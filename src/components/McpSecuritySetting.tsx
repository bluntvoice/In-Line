import {useEffect,useRef,useState} from "react";
import {api} from "../api";
import McpFaultSetting from "./McpFaultSetting";
import McpAuditSetting from "./McpAuditSetting";
import McpBatchSetting from "./McpBatchSetting";
import McpJobsSetting from "./McpJobsSetting";
import McpCacheSetting from "./McpCacheSetting";
import {canPrepare,defaultPermissions,parseScope,prepareAndCopy,receiptExpired} from "../lib/mcp-onboarding";
import type {McpClient,McpClientPreset,McpPermissions,McpReceipt,McpSecurityState} from "../lib/mcp-onboarding";

const labels:[keyof McpPermissions,string][]=[["regularRead","常规读取"],["fullRead","完整读取"],["write","日常写入"]];
export function ClientChoice({presets,value,busy,onChange}:{presets:McpClientPreset[];value:string;busy:boolean;onChange:(id:string)=>void}){
  return <label>客户端<select value={value} disabled={busy} onChange={e=>onChange(e.target.value)}>{presets.map(p=><option key={p.id} value={p.id}>{p.label}{p.available?"":" · "+p.status}</option>)}</select></label>;
}
export function ClientPermissions({value,disabled,readOnly,onChange}:{value:McpPermissions;disabled:boolean;readOnly:boolean;onChange:(value:McpPermissions)=>void}){
  return <div className="mcp-security-controls">{labels.map(([key,label])=><label key={key}><input type="checkbox" checked={value[key]} disabled={disabled||(readOnly&&key==="write")} onChange={e=>onChange({...value,[key]:e.target.checked})}/>{label}</label>)}</div>;
}
export default function McpSecuritySetting({notify}:{notify:(text:string)=>void}){
  const [state,setState]=useState<McpSecurityState|null>(null),[error,setError]=useState(""),[busy,setBusy]=useState(false);
  const [client,setClient]=useState("codex"),[permissions,setPermissions]=useState(defaultPermissions),[departments,setDepartments]=useState(""),[types,setTypes]=useState("");
  const [editing,setEditing]=useState<McpClient|null>(null),[receipt,setReceipt]=useState<McpReceipt|null>(null),[now,setNow]=useState(Date.now());
  const busyRef=useRef(false);
  const [risk,setRisk]=useState(false);const pendingRisk=useRef<(()=>Promise<unknown>)|null>(null);
  const sensitive=(p:McpPermissions,f:()=>Promise<unknown>)=>{if(!state?.sensitiveNoticeAcknowledged&&(p.fullRead||p.write)){pendingRisk.current=f;setRisk(true);}else void act(f);};
  const reload=async()=>setState(await api.mcpSecurityState());
  useEffect(()=>{let active=true;void api.mcpSecurityState().then(value=>{if(active)setState(value);}).catch(e=>{if(active)setError(String(e));});return()=>{active=false;};},[]);
  useEffect(()=>{if(!receipt)return;const timer=window.setInterval(()=>setNow(Date.now()),1000);return()=>window.clearInterval(timer);},[receipt]);
  const act=async(action:()=>Promise<unknown>)=>{if(busyRef.current)return;busyRef.current=true;setBusy(true);setError("");try{await action();}catch(e){setError(String(e));}finally{try{await reload();}catch(e){setError(String(e));}busyRef.current=false;setBusy(false);}};
  const reset=()=>{setEditing(null);setPermissions(defaultPermissions());setDepartments("");setTypes("");};
  const remember=(value:McpReceipt)=>{setReceipt(value);setNow(Date.now());};
  const copy=async(value:McpReceipt)=>{if(receiptExpired(value))throw new Error("接入提示词已过期，请重新发起接入");try{await api.copyText(value.prompt);notify("接入提示词已复制，请发送给所选 AI 客户端");}catch{throw new Error("提示词已准备，复制失败；请点击“再次复制”，无需重复授权");}};
  const save=()=>sensitive(permissions,async()=>{
    const scope={departments:parseScope(departments),taskTypes:parseScope(types)};
    if(editing){await api.mcpUpdateClient(editing.id,permissions,scope);reset();return;}
    const preset=state?.onboardingClients.find(p=>p.id===client);
    if(!canPrepare(preset,false))throw new Error("当前客户端尚不支持安全授权与配置导入");
    if(receipt&&!receiptExpired(receipt)){await copy(receipt);return;}
    await prepareAndCopy(()=>api.mcpPrepareOnboarding({client,permissions,scope}),remember,async text=>{try{await api.copyText(text);notify("接入提示词已复制，请发送给所选 AI 客户端");}catch{throw new Error("提示词已准备，复制失败；请点击“再次复制”，无需重复授权");}});
    reset();
  });
  const edit=(value:McpClient)=>{setEditing(value);setPermissions({...value.permissions});setDepartments(value.scope.departments?.join("，")??"");setTypes(value.scope.taskTypes?.join("，")??"");};
  const preset=state?.onboardingClients.find(p=>p.id===client);
  const cliReadOnly=editing?.kind==="cli"||(!editing&&client.endsWith("-cli"));
  const expired=receipt?receiptExpired(receipt,now):false;
  return <><details className="mcp-security-setting"><summary>客户端授权与权限</summary>
    {risk&&<div role="dialog" aria-label="敏感权限首次提示" className="mcp-issued"><strong>完整读取及写入权限</strong><p>完整读取会向所选AI客户端开放授权范围内的办理说明等业务内容；写入会实际修改事项和统计。请只授权可信客户端，并确认范围。撤销权限可立即停止后续访问，已执行的业务操作须另行审阅撤销。</p><div className="mcp-security-controls"><button className="button secondary" disabled={busy} onClick={()=>void act(async()=>{await api.mcpAcknowledgeSensitiveNotice();setRisk(false);const f=pendingRisk.current;pendingRisk.current=null;await f?.();})}>了解并继续</button><button className="button secondary" disabled={busy} onClick={()=>{pendingRisk.current=null;setRisk(false);}}>取消</button></div></div>}
    {error&&<p role="alert" className="mcp-security-error">{error}</p>}
    {state&&<fieldset className="mcp-permission-fields" disabled={busy||risk}>
      <div className="mcp-security-controls"><label><input type="checkbox" checked={state.paused} disabled={busy} onChange={e=>void act(()=>api.mcpSetPaused(e.target.checked))}/>暂停全部外部接入</label>{labels.map(([key,label])=><label key={key}><input type="checkbox" checked={state.groups[key]} disabled={busy} onChange={e=>{const next={...state.groups,[key]:e.target.checked};sensitive(next,()=>api.mcpSetGroups(next));}}/>{label}</label>)}</div>
      <p>每个客户端独立授权。有效权限同时受全局开关限制；完整读取包含办理说明，日常写入支持单事项操作。历史撤销需要你在软件内批准。CLI 当前仅支持只读接入，CLI 连接与聊天中的 MCP 连接分别验证。</p>
      <ul className="mcp-client-list">{state.clients.map(value=><li key={value.id}><div><strong>{value.name}</strong><small>{value.revoked?"已撤销":"已授权 · 连接仍需验证"}{value.scope.departments?" · 部门："+(value.scope.departments.join("、")||"无"):""}{value.scope.taskTypes?" · 类型："+(value.scope.taskTypes.join("、")||"无"):""}</small></div>{!value.revoked&&<div><button className="button secondary" disabled={busy} onClick={()=>edit(value)}>权限</button><button className="button secondary" disabled={busy||!canPrepare(state.onboardingClients.find(p=>p.id===(value.kind==="cli"?(value.name==="Codex CLI"?"codex-cli":"powershell-cli"):"codex")),false)} onClick={()=>void act(async()=>{const next=await api.mcpRotateOnboarding(value.id);remember(next);await copy(next);})}>轮换并接入</button><button className="button secondary danger" disabled={busy} onClick={()=>void act(()=>api.mcpRevokeClient(value.id))}>撤销</button></div>}</li>)}</ul>
      <div className="mcp-grant-form">{editing?<strong>{editing.name} · 修改权限</strong>:<ClientChoice presets={state.onboardingClients} value={client} busy={busy||Boolean(receipt&&!expired)} onChange={value=>{setClient(value);setPermissions(defaultPermissions());}}/>}</div>
      {!editing&&!preset?.available&&<p role="status">{canPrepare(preset,false)?"可先授权并复制接入提示词。配置导入与当前会话连接分别验证，只有实际验证后才显示接入成功。":"当前客户端尚不支持安全授权与配置导入。"}</p>}
      <ClientPermissions value={permissions} disabled={busy||(!editing&&Boolean(receipt&&!expired))} readOnly={cliReadOnly} onChange={setPermissions}/>
      <details className="mcp-scope-options"><summary>高级选项：限制部门或事项类型</summary><div className="mcp-grant-form"><label>部门范围<input value={departments} disabled={busy||(!editing&&Boolean(receipt&&!expired))} onChange={e=>setDepartments(e.target.value)} placeholder="留空不限制；多个用逗号分隔"/></label><label>事项类型范围<input value={types} disabled={busy||(!editing&&Boolean(receipt&&!expired))} onChange={e=>setTypes(e.target.value)} placeholder="留空不限制；多个用逗号分隔"/></label></div></details>
      <div className="mcp-security-controls"><button className="button secondary" disabled={editing?busy:!canPrepare(preset,busy)} onClick={save}>{editing?"保存客户端权限":receipt&&!expired?"再次复制接入提示词":"授权并复制接入提示词"}</button>{editing&&<button className="button secondary" disabled={busy} onClick={reset}>取消</button>}</div>
      {receipt&&<div className="mcp-issued"><p role="status">{expired?"接入提示词已过期，请重新发起接入。":"接入提示词已准备，10 分钟内有效。授权、配置验证和当前会话连接分别核验。"}</p><button className="button secondary" disabled={busy||expired} onClick={()=>void act(()=>copy(receipt))}>再次复制</button><button className="button secondary" disabled={busy} onClick={()=>setReceipt(null)}>收起提示词</button></div>}
    </fieldset>}
  </details><McpBatchSetting/><McpAuditSetting/><McpJobsSetting/><McpCacheSetting/><McpFaultSetting/></>;
}
