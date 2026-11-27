import assert from "node:assert/strict";
import {spawn,execFile} from "node:child_process";
import {createInterface} from "node:readline";
import {promisify} from "node:util";
import {readFile,writeFile} from "node:fs/promises";
import {join,dirname} from "node:path";
const [mode,exe,db,root]=process.argv.slice(2);
const id=process.env.F4_GUI_CLIENT;
const token=process.env.F4_GUI_TOKEN;
delete process.env.F4_GUI_CLIENT;
delete process.env.F4_GUI_TOKEN;
assert.ok(id && token);
const exec=promisify(execFile);
const sql=async(q,params=[])=>JSON.parse((await exec("python",["-c",
  "import sqlite3,pathlib,sys,json; c=sqlite3.connect(pathlib.Path(sys.argv[1]).resolve().as_uri()+'?mode=ro',uri=True); print(json.dumps(c.execute(sys.argv[2],json.loads(sys.argv[3])).fetchall()))",
  db,q,JSON.stringify(params)],{timeout:8500,windowsHide:true})).stdout);
const sidecar=spawn(join(dirname(exe),"in-line-mcp.exe"),[],{
  env:{...process.env,IN_LINE_MCP_CLIENT_ID:id,IN_LINE_MCP_TOKEN:token},stdio:["pipe","pipe","ignore"],windowsHide:true,
});
const lines=createInterface({input:sidecar.stdout});
const pending=new Map();
let seq=0;
lines.on("line",line=>{try{const m=JSON.parse(line);pending.get(m.id)?.(m);}catch{}});
const rpc=async(method,params)=>new Promise((resolve,reject)=>{
  const id=++seq;
  const timer=setTimeout(()=>{pending.delete(id);reject(new Error("Endpoint timeout "+method));},12000);
  pending.set(id,m=>{pending.delete(id);clearTimeout(timer);m.error?reject(new Error("Protocol request failed")):resolve(m.result);});
  sidecar.stdin.write(JSON.stringify({jsonrpc:"2.0",id,method,params})+"\n");
});
try {
  await rpc("initialize",{protocolVersion:"2024-11-05",capabilities:{},clientInfo:{name:"F4-native-approval",version:"1"}});
  sidecar.stdin.write('{"jsonrpc":"2.0","method":"notifications/initialized"}\n');
  const call=async(name,args={})=>{
    const x=(await rpc("tools/call",{name,arguments:args})).structuredContent;
    assert.equal(x?.status,"ok","Unexpected structured status for "+name);
    return x.data;
  };
  const common={reason:"Synthetic F4 native approval",intent:{summary:"Synthetic F4 native approval",explicitUserRequest:true,confirmedRealWork:true}};
  const tasks=async()=>sql("select t.id,t.title,t.planned_date,v.version from tasks t join mcp_task_versions v on v.task_id=t.id where t.title like 'F4 approval %' order by t.id");
  const caps=await call("get_capabilities");
  assert.equal(caps.schemaVersion,15);assert.equal(caps.mcpVersion,1);assert.equal(caps.tools.length,16);assert.equal(caps.permissions.fullRead,false);
  assert.equal(caps.commit,"e276844fc08b3a308c14938f0c6cc232926bfa3c");
  if(mode==="seed"){
    for(const label of ["A","B"]){
      await call("mutate_task",{...common,idempotencyKey:"f4-seed-"+label,action:"create",task:{title:"F4 approval "+label,departments:["F4 approval"],contacts:["Fixture"],taskType:"F4 approval",details:"",internalNotes:""}});
    }
    assert.equal((await tasks()).length,2);
  }else if(mode==="approve" || mode==="reject"){
    const before=await tasks();
    assert.equal(before.length,2);
    const plannedDate=mode==="approve"?"2030-01-03":"2030-01-04";
    const raw={...common,idempotencyKey:"f4-"+mode,operations:before.map(([taskId,,,taskVersion])=>({operation:{action:"reschedule",target:{taskId},taskVersion,plannedDate}}))};
    const preview=await call("batch_mutate",{...raw,action:"dryRun"});
    const request={...raw,action:"apply",expectedBasis:preview.basis};
    const result=await call("batch_mutate",request);
    assert.equal(result.commitStatus,"needs_user_approval");
    assert.deepEqual(await tasks(),before);
    await writeFile(join(root,mode+".json"),JSON.stringify({request,before,plannedDate}));
  }else if(mode==="check-approve" || mode==="check-reject"){
    const action=mode.slice(6);
    const {request,before,plannedDate}=JSON.parse(await readFile(join(root,action+".json"),"utf8"));
    if(action==="approve"){assert.ok((await tasks()).every(row=>row[2]===plannedDate));}
    else assert.deepEqual(await tasks(),before);
    const statuses=await sql("select status from mcp_batch_approvals where request_key=?",[request.idempotencyKey]);
    assert.equal(statuses.length,1);assert.notEqual(statuses[0][0],"pending");
    const replay=await call("batch_mutate",request);
    assert.equal(replay.replayed,true);
  }else if(mode==="undo"){
    const [[taskId,title]]=await tasks();
    const [[version]]=await sql("select version from mcp_field_versions where task_id=? and field='title'",[taskId]);
    const patched=await call("mutate_task",{...common,idempotencyKey:"f4-title",action:"patch",target:{taskId},fieldBase:{title:{value:title,version}},patch:{title:"F4 approval A patched"}});
    const undo=await call("request_undo",{...common,auditId:patched.auditId,idempotencyKey:"f4-undo"});
    assert.equal(undo.status,"needs_user_approval");
    assert.deepEqual(await sql("select title from tasks where id=?",[taskId]),[["F4 approval A patched"]]);
    await writeFile(join(root,"undo.json"),JSON.stringify({taskId,title,auditId:patched.auditId}));
  }else if(mode==="check-undo"){
    const x=JSON.parse(await readFile(join(root,"undo.json"),"utf8"));
    assert.deepEqual(await sql("select title from tasks where id=?",[x.taskId]),[[x.title]]);
    const status=await sql("select status from mcp_undo_requests where audit_id=?",[x.auditId]);
    assert.equal(status.length,1);assert.notEqual(status[0][0],"pending");
  }else throw new Error("Unknown synthetic approval step");
  console.log("F4_NATIVE_APPROVAL "+mode+" PASS");
}finally{
  const done=new Promise(resolve=>{if(sidecar.exitCode!==null)resolve();else sidecar.once("exit",resolve);});
  sidecar.stdin.end();
  const timer=setTimeout(()=>sidecar.kill(),2500);
  await done;clearTimeout(timer);lines.close();
}
