// Anonymous smoke test only: never launches a business host or supplies production credentials.
import {spawn} from 'node:child_process';
import {createInterface} from 'node:readline';
import {resolve} from 'node:path';
const executable=resolve(process.argv[2]??'src-tauri/target/release/in-line-mcp.exe');
const env={...process.env};
for(const key of ['IN_LINE_MCP_CLIENT_ID','IN_LINE_MCP_TOKEN','IN_LINE_MCP_TEST_HOST_EXE','IN_LINE_MCP_TEST_SECURITY_ROOT','IN_LINE_MCP_TEST_DATA_ROOT'])delete env[key];
const child=spawn(executable,[],{env,stdio:['pipe','pipe','pipe'],windowsHide:true});
const pending=new Map();
const send=(value)=>child.stdin.write(JSON.stringify(value)+'\n');
let protocolFailure;
createInterface({input:child.stdout}).on('line',line=>{
  try {const message=JSON.parse(line);if(message?.jsonrpc!=='2.0')throw new Error('Non-protocol stdout');if(message.id&&pending.has(message.id)){pending.get(message.id).resolve(message);pending.delete(message.id);}}
  catch {protocolFailure=new Error('Non-JSON stdout');for(const waiter of pending.values())waiter.reject(protocolFailure);pending.clear();}
});
child.stderr.resume();
const request=(id,method,params)=>new Promise((resolve,reject)=>{pending.set(id,{resolve,reject});send({jsonrpc:'2.0',id,method,params});});
const timer=setTimeout(()=>{for(const waiter of pending.values())waiter.reject(new Error('Protocol deadline'));child.kill();},15000);
try {
 const initialize=await request(1,'initialize',{protocolVersion:'2025-11-25',capabilities:{},clientInfo:{name:'isolated-anonymous-smoke',version:'1'}});
 if(!initialize.result)throw new Error('initialize failed');
 send({jsonrpc:'2.0',method:'notifications/initialized'});
 const list=await request(2,'tools/list',{});
 const names=list.result.tools.map(tool=>tool.name).sort();
 if(JSON.stringify(names)!==JSON.stringify(['get_capabilities','get_report_summary','list_report_items']))throw new Error('Unexpected tools');
 for(const [index,name] of names.entries()) {
   const result=await request(3+index,'tools/call',{name,arguments:name==='get_capabilities'?{}:{startDate:'2026-08-08',endDate:'2026-08-08'}});
   const body=result.result;
   if(body?.isError!==true||body.structuredContent?.status!=='error'||body.structuredContent.data!==null||body.structuredContent.error.code!=='unauthenticated')throw new Error('Anonymous authorization gate failed');
 }
 for(const [index,expected] of ['unauthenticated','unauthenticated','rate_limited'].entries()) {
   const result=await request(6+index,'tools/call',{name:'get_capabilities',arguments:{}});
   const body=result.result;
   if(body?.isError!==true||body.structuredContent?.status!=='error'||body.structuredContent.data!==null||body.structuredContent.error.code!==expected)throw new Error('Release preflight rate limit gate failed');
 }
 if(protocolFailure)throw protocolFailure;
 console.log('PASS: release sidecar initialize/tools/list/3 anonymous tools + 3 repeat calls (6th rate_limited); stdout JSON only; structured errors with isError=true and no business data.');
} finally {clearTimeout(timer);child.kill();}
