import {useRef,useState} from "react";
import {api} from "../api";
export default function McpCacheSetting(){
  const [data,setData]=useState<Awaited<ReturnType<typeof api.mcpCacheState>>|null>(null),[error,setError]=useState(""),[busy,setBusy]=useState(false);const lock=useRef(false);
  const run=async(action?:()=>Promise<unknown>)=>{if(lock.current)return;lock.current=true;setBusy(true);setError("");try{await action?.();setData(await api.mcpCacheState());}catch(e){setError(String(e));}finally{lock.current=false;setBusy(false);}};
  return <details className="mcp-security-setting" onToggle={e=>{if(e.currentTarget.open)void run();}}><summary>报告缓存与数据健康</summary>
    {error&&<p role="alert" className="mcp-security-error">{error}</p>}
    {data&&<><p>内存缓存 {data.cache.entries} 条 · {(data.cache.bytes/1024).toFixed(1)} KB · 最长 {data.cache.ttlSeconds} 秒{data.health.healthy?" · 索引正常":" · 索引需要检查"}</p>
      {data.cache.sources.length>0&&<p>最近计算：{new Date(Math.max(...data.cache.sources.map(s=>s.computedAt))*1000).toLocaleString()}。每次复用均重新核对数据及权限。</p>}
      {data.cache.capacityBypasses>=3&&<p role="alert">部分报告超过缓存容量，已直接返回计算结果；可清理缓存后再检查。</p>}
      <div className="mcp-security-controls"><button className="button secondary" disabled={busy} onClick={()=>void run(()=>api.mcpClearCache())}>清理报告缓存</button><button className="button secondary" disabled={busy} onClick={()=>void run()}>刷新状态</button>{!data.health.healthy&&data.health.repairable&&<button className="button secondary" disabled={busy} onClick={()=>void run(()=>api.mcpRebuildIndex())}>重建派生索引</button>}</div>
      {!data.health.healthy&&<p>重建只处理派生索引，旧游标需重新同步；原事项、号码和审计不变。</p>}</>}
    <small>清理只释放报告缓存，分页快照、进行中任务、审计及正式导出均保留。报告不落盘；任务检查点仅由 Windows 当前用户文件权限保护，同账户进程仍可访问。</small>
  </details>;
}
