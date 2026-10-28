import {useEffect,useRef,useState,type ReactNode} from "react";
import {Plug} from "lucide-react";
import {api} from "../api";
import ConnectionDialog from "./ConnectionDialog";
import McpStorageGate from "./McpStorageGate";
import McpFaultSetting from "./McpFaultSetting";
import McpAuditSetting from "./McpAuditSetting";
import McpBatchSetting from "./McpBatchSetting";
import McpJobsSetting from "./McpJobsSetting";
import McpCacheSetting from "./McpCacheSetting";
import {canPrepare,defaultPermissions,initialMcpIdentity,reusableMcpClients,parseScope,prepareAndCopy,receiptExpired} from "../lib/mcp-onboarding";
import type {McpAuthorizationDiagnostic,McpClient,McpClientPreset,McpPermissions,McpReceipt,McpSecurityState} from "../lib/mcp-onboarding";

const labels:[keyof McpPermissions,string][]=[["regularRead","常规读取"],["fullRead","完整读取"],["write","日常写入"]];
export function ClientChoice({presets,value,busy,onChange}:{presets:McpClientPreset[];value:string;busy:boolean;onChange:(id:string)=>void}){
  return <label>客户端<select aria-label="客户端" value={value} disabled={busy} onChange={e=>onChange(e.target.value)}>{presets.map(p=><option key={p.id} value={p.id}>{p.label}{p.available?"":" · "+p.status}</option>)}</select></label>;
}
export function ClientPermissions({value,disabled,readOnly,onChange}:{value:McpPermissions;disabled:boolean;readOnly:boolean;onChange:(value:McpPermissions)=>void}){
  return <div className="mcp-security-controls">{labels.map(([key,label])=><label key={key}><input type="checkbox" checked={value[key]} disabled={disabled||(readOnly&&key==="write")} onChange={e=>onChange({...value,[key]:e.target.checked})}/>{label}</label>)}</div>;
}
const presetId=(value:McpClient)=>value.kind==="cli"?(value.name==="Codex CLI"?"codex-cli":"powershell-cli"):"codex";
type SettingProps={notify:(text:string)=>void;guide?:ReactNode};
export default function McpSecuritySetting(props:SettingProps){return <McpStorageGate><McpSecurityControls {...props}/></McpStorageGate>;}
function McpSecurityControls({notify,guide}:SettingProps){
  const [state,setState]=useState<McpSecurityState|null>(null),[error,setError]=useState(""),[busy,setBusy]=useState(false);
  const [open,setOpen]=useState(false),[resume,setResume]=useState(false),[enableGlobal,setEnableGlobal]=useState(false),[acknowledged,setAcknowledged]=useState(false);
  const [client,setClient]=useState("codex"),[permissions,setPermissions]=useState(defaultPermissions),[departments,setDepartments]=useState(""),[types,setTypes]=useState("");
  const [editing,setEditing]=useState<McpClient|null>(null),[receipt,setReceipt]=useState<McpReceipt|null>(null),[now,setNow]=useState(Date.now());
  const [identityId,setIdentityId]=useState("new");
  const busyRef=useRef(false);
  const [diagnostic,setDiagnostic]=useState<McpAuthorizationDiagnostic|null>(null);
  const diagnose=async(id:string|null)=>{if(busyRef.current)return;busyRef.current=true;setBusy(true);setError("");setDiagnostic(null);try{setDiagnostic(await api.mcpAuthorizationDiagnostic(id));}catch(e){setError(String(e));}finally{busyRef.current=false;setBusy(false);}};
  const [risk,setRisk]=useState(false);const pendingRisk=useRef<(()=>Promise<unknown>)|null>(null);
  const reload=async()=>{try{setState(await api.mcpSecurityState());}catch(e){setState(null);throw e;}};
  useEffect(()=>{let active=true;void api.mcpSecurityState().then(value=>{if(active)setState(value);}).catch(e=>{if(active)setError(String(e));});return()=>{active=false;};},[]);
  useEffect(()=>{if(!receipt)return;const timer=window.setInterval(()=>setNow(Date.now()),1000);return()=>window.clearInterval(timer);},[receipt]);
  const act=async(action:()=>Promise<unknown>)=>{if(busyRef.current)return;busyRef.current=true;setBusy(true);setError("");try{await action();}catch(e){setError(String(e));}finally{try{await reload();}catch(e){setError(String(e));}busyRef.current=false;setBusy(false);}};
  const sensitive=(p:McpPermissions,f:()=>Promise<unknown>)=>{if(!state?.sensitiveNoticeAcknowledged&&(p.fullRead||p.write)){pendingRisk.current=f;setRisk(true);}else void act(f);};
  const reset=()=>{setEditing(null);setPermissions(defaultPermissions());setDepartments("");setTypes("");setResume(false);setEnableGlobal(false);setAcknowledged(false);};
  const chooseIdentity=(id:string,clients=state?.clients??[])=>{reset();setReceipt(null);setIdentityId(id);const existing=clients.find(c=>c.id===id);if(existing){setPermissions({...existing.permissions});setDepartments(existing.scope.departments?.join("，")??"");setTypes(existing.scope.taskTypes?.join("，")??"");}};
  const remember=(value:McpReceipt)=>{setReceipt(value);setNow(Date.now());if(client==="codex")setIdentityId(value.clientId);};
  const copyPrompt=async(text:string)=>{try{await api.copyText(text);notify("接入提示词已复制，请发送给所选 AI 客户端");}catch{throw new Error("提示词已准备，复制失败；请点击“再次复制”，无需重复授权");}};
  const copy=async(value:McpReceipt)=>{if(receiptExpired(value))throw new Error("接入提示词已过期，请重新发起接入");await copyPrompt(value.prompt);};
  const save=()=>void act(async()=>{
    if(!state)throw new Error("授权状态尚未读取，请稍后重试");
    if(receipt&&!receiptExpired(receipt)&&!editing){await copy(receipt);return;}
    if(!editing&&client==="codex"&&identityId!=="new"){
      if(!identityId)throw new Error("请选择需要复用的客户端身份，或明确选择新增客户端");
      await prepareAndCopy(()=>api.mcpReuseOnboarding(identityId),remember,copyPrompt);
      return;
    }
    const blocked=labels.some(([key])=>permissions[key]&&!state.groups[key]);
    if(blocked&&!enableGlobal)throw new Error("请确认开启本次所需的全局权限");
    if(state.paused&&!resume)throw new Error("请确认解除全部接入暂停");
    if(!state.sensitiveNoticeAcknowledged&&(permissions.fullRead||permissions.write)){
      if(!acknowledged)throw new Error("请阅读并确认敏感权限说明");
      await api.mcpAcknowledgeSensitiveNotice();
    }
    if(blocked)await api.mcpSetGroups(Object.fromEntries(labels.map(([key])=>[key,state.groups[key]||permissions[key]])) as McpPermissions);
    if(state.paused)await api.mcpSetPaused(false);
    const scope={departments:parseScope(departments),taskTypes:parseScope(types)};
    if(editing){await api.mcpUpdateClient(editing.id,permissions,scope);reset();setOpen(false);return;}
    const preset=state.onboardingClients.find(p=>p.id===client);
    if(!canPrepare(preset,false))throw new Error("当前客户端尚不支持安全授权与配置导入");
    await prepareAndCopy(()=>api.mcpPrepareOnboarding({client,permissions,scope}),remember,copyPrompt);
  });
  const edit=(value:McpClient)=>{reset();setReceipt(null);setError("");setEditing(value);setPermissions({...value.permissions});setDepartments(value.scope.departments?.join("，")??"");setTypes(value.scope.taskTypes?.join("，")??"");setOpen(true);};
  const preset=state?.onboardingClients.find(p=>p.id===client);
  const reusing=!editing&&client==="codex"&&identityId!=="new";
  const expired=receipt?receiptExpired(receipt,now):false;
  const locked=busy||Boolean(!editing&&receipt&&!expired);
  const blocked=state?labels.filter(([key])=>permissions[key]&&!state.groups[key]):[];
  const needsNotice=Boolean(!reusing&&state&&!state.sensitiveNoticeAcknowledged&&(permissions.fullRead||permissions.write));
  const confirmMissing=reusing?!identityId:(blocked.length>0&&!enableGlobal)||(Boolean(state?.paused)&&!resume)||(needsNotice&&!acknowledged);
  return <><div className="setting-row mcp-connection-row"><div><strong>AI 接入（MCP / CLI）</strong><span>在接入窗口选择客户端和权限，再复制提示词交给 AI</span></div><button className="button secondary" disabled={busy} onClick={()=>{setError("");setState(null);setOpen(true);void api.mcpSecurityState().then(next=>{setState(next);if(!receipt&&client==="codex")chooseIdentity(initialMcpIdentity(next.clients),next.clients);}).catch(e=>{setState(null);setError(String(e));});}}><Plug size={16}/>连接 AI</button></div>
  <details className="mcp-security-setting"><summary>已授权客户端与高级管理</summary>
    <p>接入失败时，可对照当前窗口与导入器看到的授权存储。诊断不执行接入，不代表连接验证通过。</p>
    <button className="button secondary" disabled={busy} onClick={()=>void diagnose(null)}>检查授权存储</button>
    {diagnostic&&<div className="mcp-issued" role="region" aria-label="授权存储脱敏诊断"><p>仅含存储位置、文件标识、授权版本和客户端匹配摘要；包含本机路径。配置与当前会话连接均未由此诊断验证。</p><textarea readOnly aria-label="脱敏诊断结果" value={JSON.stringify(diagnostic,null,2)}/><button className="button secondary" disabled={busy} onClick={()=>void api.copyText(JSON.stringify(diagnostic,null,2)).then(()=>notify("脱敏诊断已复制，包含本机路径")).catch(e=>setError(String(e)))}>复制脱敏诊断</button></div>}
    {risk&&<div role="dialog" aria-label="敏感权限首次提示" className="mcp-issued"><strong>完整读取及写入权限</strong><p>完整读取会向所选 AI 客户端开放授权范围内的办理说明等内容；写入会实际修改事项和统计。请只授权可信客户端并确认范围。历史撤销须在软件内批准。</p><div className="mcp-security-controls"><button className="button secondary" disabled={busy} onClick={()=>void act(async()=>{await api.mcpAcknowledgeSensitiveNotice();setRisk(false);const f=pendingRisk.current;pendingRisk.current=null;await f?.();})}>了解并继续</button><button className="button secondary" disabled={busy} onClick={()=>{pendingRisk.current=null;setRisk(false);}}>取消</button></div></div>}
    {!open&&error&&<p role="alert" className="mcp-security-error">{error}</p>}
    {state&&<fieldset className="mcp-permission-fields" disabled={busy||risk}>
      <div className="mcp-security-controls"><label><input type="checkbox" checked={state.paused} onChange={e=>void act(()=>api.mcpSetPaused(e.target.checked))}/>暂停全部外部接入</label>{labels.map(([key,label])=><label key={key}><input type="checkbox" checked={state.groups[key]} onChange={e=>{const next={...state.groups,[key]:e.target.checked};sensitive(next,()=>api.mcpSetGroups(next));}}/>{label}</label>)}</div>
      <p>每个客户端独立授权，有效权限同时受全局开关限制。CLI 连接与聊天中的 MCP 连接分别验证。</p>
      <ul className="mcp-client-list">{state.clients.map(value=><li key={value.id}><div><strong>{value.name}</strong><small>{value.revoked?"已撤销":"已授权 · 连接仍需验证"}{value.scope.departments?" · 部门："+(value.scope.departments.join("、")||"无"):""}{value.scope.taskTypes?" · 类型："+(value.scope.taskTypes.join("、")||"无"):""}</small></div><div className="mcp-button-actions"><button className="button secondary" onClick={()=>void diagnose(value.id)}>接入诊断</button>{!value.revoked&&<><button className="button secondary" onClick={()=>edit(value)}>权限</button><button className="button secondary" disabled={!canPrepare(state.onboardingClients.find(p=>p.id===presetId(value)),busy)} onClick={()=>void act(async()=>{const next=await api.mcpRotateOnboarding(value.id);reset();remember(next);setClient(presetId(value));setIdentityId(value.id);setPermissions({...value.permissions});setDepartments(value.scope.departments?.join("，")??"");setTypes(value.scope.taskTypes?.join("，")??"");setOpen(true);await copy(next);})}>轮换并接入</button><button className="button secondary danger" onClick={()=>void act(()=>api.mcpRevokeClient(value.id))}>撤销</button></>}</div></li>)}</ul>
    </fieldset>}
    {guide}
  </details>
  {open&&<ConnectionDialog title={editing?"修改客户端权限":"连接 AI · MCP / CLI"} busy={busy} onClose={()=>{if(editing)reset();setOpen(false);}}>
    {error&&<p role="alert" className="mcp-security-error">{error}</p>}
    {!state?<p role="status">{error?"授权状态读取失败，请关闭后重试。":"正在读取授权状态…"}</p>:<>
      <div className="mcp-grant-form">{editing?<strong>{editing.name} · 修改权限</strong>:<ClientChoice presets={state.onboardingClients} value={client} busy={locked} onChange={value=>{setClient(value);chooseIdentity(value==="codex"?initialMcpIdentity(state.clients):"new");}}/>}
      {!editing&&client==="codex"&&<label>授权身份<select aria-label="授权身份" value={identityId} disabled={locked} onChange={e=>chooseIdentity(e.target.value)}><option value="" disabled>请选择已有身份或新增客户端</option>{reusableMcpClients(state.clients).map((c,i)=><option key={c.id} value={c.id}>{c.name} · 已有身份 {i+1}</option>)}<option value="new">新增客户端（创建独立授权）</option></select></label>}</div>
      {!editing&&!preset?.available&&<p role="status">{canPrepare(preset,false)?"可先授权并导入配置；当前会话连接须另行实测，尚未验证自动接入成功。":"当前客户端尚不支持安全授权与配置导入。"}</p>}
      <p>常规读取用于查询与统计；完整读取额外包含办理说明；日常写入会修改事项。默认只选常规读取。</p>
      {reusing&&<p role="status">复用所选身份及原授权，不改变全局开关、权限或范围。暂停和全局权限限制继续生效；权限调整请使用高级管理。无法安全取得有效凭证时，可明确选择同一身份轮换，旧凭证会立即失效。</p>}
      <ClientPermissions value={permissions} disabled={locked||reusing} readOnly={false} onChange={setPermissions}/>
      <details className="mcp-scope-options"><summary>限制部门或事项类型（可选）</summary><div className="mcp-grant-form"><label>部门范围<input value={departments} disabled={locked||reusing} onChange={e=>setDepartments(e.target.value)} placeholder="留空不限制；多个用逗号分隔"/></label><label>事项类型范围<input value={types} disabled={locked||reusing} onChange={e=>setTypes(e.target.value)} placeholder="留空不限制；多个用逗号分隔"/></label></div></details>
      {!reusing&&blocked.length>0&&<label className="mcp-connect-confirm"><input type="checkbox" checked={enableGlobal} disabled={locked} onChange={e=>setEnableGlobal(e.target.checked)}/><span>同时开启全局权限：{blocked.map(([,label])=>label).join("、")}。其他已有相应授权的客户端也将恢复这些权限。</span></label>}
      {!reusing&&state.paused&&<label className="mcp-connect-confirm"><input type="checkbox" checked={resume} disabled={locked} onChange={e=>setResume(e.target.checked)}/><span>解除全部外部接入暂停；其他有效授权也将恢复访问。</span></label>}
      {needsNotice&&<label className="mcp-connect-confirm"><input type="checkbox" checked={acknowledged} disabled={locked} onChange={e=>setAcknowledged(e.target.checked)}/><span>我了解完整读取会开放范围内的办理说明等内容，写入会修改事项和统计；仅授权可信客户端。历史撤销须在软件内批准。</span></label>}
      <div className="mcp-security-controls"><button className="button primary" disabled={(editing?busy:!canPrepare(preset,busy))||(!(receipt&&!expired)&&confirmMissing)} onClick={save}>{editing?"保存客户端权限":receipt&&!expired?"再次复制接入提示词":reusing?"复用授权并复制接入提示词":"授权并复制接入提示词"}</button>{reusing&&identityId&&!(receipt&&!expired)&&<button className="button secondary" disabled={busy} onClick={()=>void act(async()=>{const next=await api.mcpRotateOnboarding(identityId);remember(next);await copy(next);})}>轮换并接入（旧凭证立即失效）</button>}{editing&&<button className="button secondary" disabled={busy} onClick={()=>{reset();setOpen(false);}}>取消</button>}</div>
      {receipt&&<div className="mcp-issued"><label>接入提示词<textarea readOnly aria-label="接入提示词" value={receipt.prompt}/></label><p role="status">{expired?"接入提示词已过期；重新准备时优先复用原身份，不要重放旧指令。":"提示词 10 分钟内有效。配置验证不代表当前会话可用；须由原对话真实调用核验，自动恢复仍未验证。"}</p><div className="mcp-button-actions"><button className="button secondary" disabled={busy||expired} onClick={()=>void act(()=>copy(receipt))}>再次复制</button><button className="button secondary" disabled={busy} onClick={()=>{if(client==="codex")chooseIdentity(receipt.clientId);else{setReceipt(null);reset();}}}>发起新的接入</button></div></div>}
    </>}
  </ConnectionDialog>}
  <McpBatchSetting/><McpAuditSetting/><McpJobsSetting/><McpCacheSetting/><McpFaultSetting/></>;
}
