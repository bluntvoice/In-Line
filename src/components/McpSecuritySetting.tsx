import {useEffect,useRef,useState} from "react";
import {api} from "../api";
import {canPrepare,defaultPermissions,parseScope,prepareAndCopy,receiptExpired} from "../lib/mcp-onboarding";
import type {McpClient,McpClientPreset,McpPermissions,McpReceipt,McpSecurityState} from "../lib/mcp-onboarding";

const labels:[keyof McpPermissions,string][]=[["regularRead","常规读取"],["fullRead","完整读取"],["write","日常写入"]];
export function ClientChoice({presets,value,busy,onChange}:{presets:McpClientPreset[];value:string;busy:boolean;onChange:(id:string)=>void}){
  return <label>AI 客户端<select value={value} disabled={busy} onChange={e=>onChange(e.target.value)}>{presets.map(p=><option key={p.id} value={p.id}>{p.label}{p.available?"":" · "+p.status}</option>)}</select></label>;
}
export default function McpSecuritySetting({notify}:{notify:(text:string)=>void}){
  const [state,setState]=useState<McpSecurityState|null>(null),[error,setError]=useState(""),[busy,setBusy]=useState(false);
  const [client,setClient]=useState("codex"),[permissions,setPermissions]=useState(defaultPermissions),[departments,setDepartments]=useState(""),[types,setTypes]=useState("");
  const [editing,setEditing]=useState<McpClient|null>(null),[receipt,setReceipt]=useState<McpReceipt|null>(null),[now,setNow]=useState(Date.now());
  const busyRef=useRef(false);
  const reload=async()=>setState(await api.mcpSecurityState());
  useEffect(()=>{let active=true;void api.mcpSecurityState().then(value=>{if(active)setState(value);}).catch(e=>{if(active)setError(String(e));});return()=>{active=false;};},[]);
  useEffect(()=>{if(!receipt)return;const timer=window.setInterval(()=>setNow(Date.now()),1000);return()=>window.clearInterval(timer);},[receipt]);
  const act=async(action:()=>Promise<unknown>)=>{if(busyRef.current)return;busyRef.current=true;setBusy(true);setError("");try{await action();}catch(e){setError(String(e));}finally{try{await reload();}catch(e){setError(String(e));}busyRef.current=false;setBusy(false);}};
  const reset=()=>{setEditing(null);setPermissions(defaultPermissions());setDepartments("");setTypes("");};
  const remember=(value:McpReceipt)=>{setReceipt(value);setNow(Date.now());};
  const copy=async(value:McpReceipt)=>{if(receiptExpired(value))throw new Error("接入提示词已过期，请重新发起接入");try{await api.copyText(value.prompt);notify("接入提示词已复制，请发送给所选 AI 客户端");}catch{throw new Error("提示词已准备，复制失败；请点击“再次复制”，无需重复授权");}};
  const save=()=>void act(async()=>{
    const scope={departments:parseScope(departments),taskTypes:parseScope(types)};
    if(editing){await api.mcpUpdateClient(editing.id,permissions,scope);reset();return;}
    const preset=state?.onboardingClients.find(p=>p.id===client);
    if(!canPrepare(preset,false))throw new Error("当前客户端自动接入仍在验证，入口暂未开放");
    if(receipt&&!receiptExpired(receipt)){await copy(receipt);return;}
    await prepareAndCopy(()=>api.mcpPrepareOnboarding({client,permissions,scope}),remember,async text=>{try{await api.copyText(text);notify("接入提示词已复制，请发送给所选 AI 客户端");}catch{throw new Error("提示词已准备，复制失败；请点击“再次复制”，无需重复授权");}});
    reset();
  });
  const edit=(value:McpClient)=>{setEditing(value);setPermissions({...value.permissions});setDepartments(value.scope.departments?.join("，")??"");setTypes(value.scope.taskTypes?.join("，")??"");};
  const preset=state?.onboardingClients.find(p=>p.id===client);
  const expired=receipt?receiptExpired(receipt,now):false;
  return <details className="mcp-security-setting"><summary>客户端授权与权限</summary>
    {error&&<p role="alert" className="mcp-security-error">{error}</p>}
    {state&&<>
      <div className="mcp-security-controls"><label><input type="checkbox" checked={state.paused} disabled={busy} onChange={e=>void act(()=>api.mcpSetPaused(e.target.checked))}/>暂停全部 MCP</label>{labels.map(([key,label])=><label key={key}><input type="checkbox" checked={state.groups[key]} disabled={busy} onChange={e=>void act(()=>api.mcpSetGroups({...state.groups,[key]:e.target.checked}))}/>{label}</label>)}</div>
      <p>每个客户端独立授权。有效权限同时受全局开关限制；完整读取包含办理说明，日常写入当前尚无可用工具。</p>
      <ul className="mcp-client-list">{state.clients.map(value=><li key={value.id}><div><strong>{value.name}</strong><small>{value.revoked?"已撤销":"已授权 · 连接仍需验证"}{value.scope.departments?" · 部门："+(value.scope.departments.join("、")||"无"):""}{value.scope.taskTypes?" · 类型："+(value.scope.taskTypes.join("、")||"无"):""}</small></div>{!value.revoked&&<div><button className="button secondary" disabled={busy} onClick={()=>edit(value)}>权限</button><button className="button secondary" disabled={busy||!canPrepare(state.onboardingClients.find(p=>p.id==="codex"),false)} onClick={()=>void act(async()=>{const next=await api.mcpRotateOnboarding(value.id);remember(next);await copy(next);})}>轮换并接入</button><button className="button secondary danger" disabled={busy} onClick={()=>void act(()=>api.mcpRevokeClient(value.id))}>撤销</button></div>}</li>)}</ul>
      <div className="mcp-grant-form">{editing?<strong>{editing.name} · 修改权限</strong>:<ClientChoice presets={state.onboardingClients} value={client} busy={busy||Boolean(receipt&&!expired)} onChange={setClient}/>}</div>
      {!editing&&!preset?.available&&<p role="status">当前 Codex 自动接入验证中，通过后开放授权并复制提示词。</p>}
      <div className="mcp-security-controls">{labels.map(([key,label])=><label key={key}><input type="checkbox" checked={permissions[key]} disabled={busy||(!editing&&Boolean(receipt&&!expired))} onChange={e=>setPermissions({...permissions,[key]:e.target.checked})}/>{label}</label>)}</div>
      <details className="mcp-scope-options"><summary>高级选项：限制部门或事项类型</summary><div className="mcp-grant-form"><label>部门范围<input value={departments} disabled={busy||(!editing&&Boolean(receipt&&!expired))} onChange={e=>setDepartments(e.target.value)} placeholder="留空不限制；多个用逗号分隔"/></label><label>事项类型范围<input value={types} disabled={busy||(!editing&&Boolean(receipt&&!expired))} onChange={e=>setTypes(e.target.value)} placeholder="留空不限制；多个用逗号分隔"/></label></div></details>
      <div className="mcp-security-controls"><button className="button secondary" disabled={editing?busy:!canPrepare(preset,busy)} onClick={save}>{editing?"保存客户端权限":receipt&&!expired?"再次复制接入提示词":"授权并复制接入提示词"}</button>{editing&&<button className="button secondary" disabled={busy} onClick={reset}>取消</button>}</div>
      {receipt&&<div className="mcp-issued"><p role="status">{expired?"接入提示词已过期，请重新发起接入。":"接入提示词已准备，10 分钟内有效。授权、配置验证和当前会话连接分别核验。"}</p><button className="button secondary" disabled={busy||expired} onClick={()=>void act(()=>copy(receipt))}>再次复制</button><button className="button secondary" disabled={busy} onClick={()=>setReceipt(null)}>收起提示词</button></div>}
    </>}
  </details>;
}
