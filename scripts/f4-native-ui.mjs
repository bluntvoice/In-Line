import assert from "node:assert/strict";
import {execFile,spawn} from "node:child_process";
import {createInterface} from "node:readline";
import {promisify} from "node:util";
import {createHash} from "node:crypto";
import {readFile,copyFile,readdir,writeFile,utimes} from "node:fs/promises";
import {dirname,join} from "node:path";
import {fileURLToPath,pathToFileURL} from "node:url";
const exec=promisify(execFile);
const lab=process.env.F4_NATIVE_LAB;
const output=process.env.F4_NATIVE_EVIDENCE;
const db=process.env.F4_NATIVE_DATABASE;
const appPid=process.env.F4_NATIVE_APP_PID;
const syntheticId=process.env.F4_SYNTHETIC_ID;
const syntheticToken=process.env.F4_SYNTHETIC_TOKEN;
delete process.env.F4_SYNTHETIC_ID;
delete process.env.F4_SYNTHETIC_TOKEN;
const {chromium}=await import(pathToFileURL(join(lab,"node_modules/playwright/index.mjs")).href);
const ps=fileURLToPath(new URL("./f4-win-controls.ps1",import.meta.url));
const os=(mode,...rest)=>exec(process.env.F4_NATIVE_PWSH,
  ["-NoProfile","-STA","-File",ps,"-Mode",mode,"-ProductPid",appPid,...rest],{timeout:22000,windowsHide:true});
const digest=async(path)=>createHash("sha256").update(await readFile(path)).digest("hex");
const browser=await chromium.connectOverCDP(process.env.F4_NATIVE_CDP,{timeout:12000});
const page=browser.contexts().flatMap(context=>context.pages()).find(item=>!item.url().includes("#"));
assert.ok(page && /tauri|localhost/i.test(page.url()),"Real Tauri document missing");
page.setDefaultTimeout(12000);
const evidence=[];
const exact=(name,scope=page)=>scope.getByRole("button",{name,exact:true});
const nav=label=>page.locator(".sidebar nav button").filter({hasText:label}).click();
const detail=page.locator(".detail-panel");
const form=page.locator(".task-form-panel");
const row=title=>page.locator(".task-table tbody tr").filter({has:page.getByText(title,{exact:true})});
const create=async(title,notes="")=>{
  await page.locator(".new-ticket").click();
  await form.getByPlaceholder("输入或选择部门 / 团队").fill("F4 synthetic");
  await form.getByPlaceholder("输入或选择部门 / 团队").press("Enter");
  await form.getByPlaceholder("输入或选择对接人").fill("F4 reviewer");
  await form.getByPlaceholder("输入或选择对接人").press("Enter");
  await form.getByPlaceholder("输入或选择事项类型").fill("F4 acceptance");
  await form.getByPlaceholder("一句话说明要处理的事项").fill(title);
  if(notes) await form.getByPlaceholder("记录判断、风险或后续计划，仅保存在本机").fill(notes);
  await exact("保存并取号",form).click();
  await form.waitFor({state:"hidden"});
  await row(title).waitFor();
};
const closeDetail=()=>detail.locator("header").first().getByRole("button",{name:"关闭",exact:true}).click();
const phase=async(name,fn)=>{
  try{const observed=await fn();evidence.push({name,status:"PASS",observed});console.log("F4_NATIVE PASS "+name);}
  catch(error){
    evidence.push({name,status:"FAIL",error:String(error).split("\n")[0]});
    console.error("F4_NATIVE FAIL "+name+": "+String(error).split("\n")[0]);
    await page.screenshot({path:join(output,name+"-failure.png"),timeout:4000}).catch(()=>{});
  }
  await writeFile(join(output,"native-ui.json"),JSON.stringify(evidence,null,2));
};
const readDb=async()=>{
  const q="import sqlite3, pathlib,sys,json; c=sqlite3.connect(pathlib.Path(sys.argv[1]).resolve().as_uri()+'?mode=ro',uri=True); print(json.dumps({'items':c.execute('select title,is_import_conflict from tasks order by id').fetchall(),'integrity':c.execute('pragma integrity_check').fetchone()[0],'fk':c.execute('pragma foreign_key_check').fetchall(),'schema':c.execute('select version from schema_meta').fetchone()[0]}))";
  return JSON.parse((await exec("python",["-c",q,db],{timeout:12000,windowsHide:true})).stdout);
};
const sql=async(query,params=[])=>{
  const python="import sqlite3,pathlib,sys,json; c=sqlite3.connect(pathlib.Path(sys.argv[1]).resolve().as_uri()+'?mode=ro',uri=True); print(json.dumps(c.execute(sys.argv[2],json.loads(sys.argv[3])).fetchall()))";
  return JSON.parse((await exec("python",["-c",python,db,query,JSON.stringify(params)],{timeout:10000,windowsHide:true})).stdout);
};
async function withSidecar(body){
  assert.ok(syntheticId && syntheticToken,"Missing isolated synthetic authorization");
  const sidecar=spawn(join(dirname(process.env.F4_NATIVE_EXE),"in-line-mcp.exe"),[],{
    env:{...process.env,IN_LINE_MCP_CLIENT_ID:syntheticId,IN_LINE_MCP_TOKEN:syntheticToken},
    windowsHide:true,stdio:["pipe","pipe","ignore"],
  });
  const lines=createInterface({input:sidecar.stdout});
  const pending=new Map();
  let id=0;
  lines.on("line",line=>{
    try{
      const message=JSON.parse(line);
      pending.get(message.id)?.(message);
    }catch{}
  });
  const request=async(method,params)=>new Promise((resolve,reject)=>{
    const key=++id;
    const timer=setTimeout(()=>{pending.delete(key);reject(new Error("MCP endpoint timeout "+method));},12000);
    pending.set(key,message=>{
      pending.delete(key);clearTimeout(timer);
      if(message.error){reject(new Error("MCP endpoint JSON-RPC error "+message.error.code));}
      else resolve(message.result);
    });
    sidecar.stdin.write(JSON.stringify({jsonrpc:"2.0",id:key,method,params})+"\n");
  });
  try{
    await request("initialize",{protocolVersion:"2024-11-05",capabilities:{},clientInfo:{name:"f4-native-fixture",version:"1"}});
    sidecar.stdin.write(JSON.stringify({jsonrpc:"2.0",method:"notifications/initialized"})+"\n");
    await body(async(name,args)=> (await request("tools/call",{name,arguments:args})).structuredContent);
  }finally{
    const settled=new Promise(resolve=>{if(sidecar.exitCode!==null)resolve();else sidecar.once("exit",resolve);});
    sidecar.stdin.end();
    const timer=setTimeout(()=>sidecar.kill(),3000);
    await settled;clearTimeout(timer);lines.close();
  }
}
await page.locator(".sidebar").waitFor();
await phase("actual-about-and-window",async()=>{
  await exact("关于").click();
  await page.getByText(/v?0\.5\.0/).first().waitFor();
  const native=JSON.parse((await os("window")).stdout.trim());
  assert.ok(native.visible && native.dpi===96);
  await nav("待办队列");
  return native;
});
await phase("real-restore-three-rounds",async()=>{
  await nav("待办队列");
  await create("F4 restore task","source note");
  await exact("软件设置").click();
  await exact("立即备份").click();
  await page.locator(".backup-list article").first().waitFor();
  const backupDir=join(dirname(db),"backups");
  const saved=(await readdir(backupDir)).find(name=>name.endsWith("-manual.db"));
  assert.ok(saved,"Actual GUI backup missing");
  const external=join(lab,"f4-source.db");
  await copyFile(join(backupDir,saved),external);
  const sha=await digest(external);
  await utimes(external,new Date("2024-01-01"),new Date("2024-01-01"));
  for(let i=1;i<=13;i++){
    const p=join(backupDir,"InLine-backup-20261001-0101"+String(i).padStart(2,"0")+"-manual.db");
    await copyFile(external,p);
  }
  await nav("待办队列");
  await row("F4 restore task").click();
  await exact("编辑",detail).click();
  await form.getByPlaceholder("记录判断、风险或后续计划，仅保存在本机").fill("different local task");
  await exact("保存修改",form).click();
  await form.waitFor({state:"hidden"});
  if(await detail.isVisible())await closeDetail();
  await create("F4 restore task（冲突）");
  await exact("软件设置").click();
  await page.waitForFunction(()=>document.querySelectorAll(".backup-list article").length>12);
  const restore=exact("导入并恢复");
  const notice=page.locator(".backup-result-notice");
  const outcomes=[];
  for(let i=0;i<3;i++){
    page.once("dialog",dialog=>dialog.accept());
    await Promise.all([restore.click(),os("open","-Path",external)]);
    const msg=i===0?"新增 1 项，合并 0 项，冲突保留 1 项。":"新增 0 项，合并 1 项，冲突保留 0 项。";
    await notice.getByText(msg,{exact:true}).waitFor();
    const review=page.locator(".import-conflict-dialog");
    if(i===0){
      await review.getByText("F4 restore task（冲突 2）",{exact:true}).waitFor();
      await exact("知道了，稍后处理",review).click();
    }
    await review.waitFor({state:"hidden"});
    await page.locator(".backup-list article").first().getByText("刚刚导入").waitFor();
    const persisted=await readDb();
    assert.deepEqual(persisted.items,[["F4 restore task",0],["F4 restore task（冲突）",0],["F4 restore task（冲突 2）",1]]);
    assert.equal(persisted.integrity,"ok");assert.deepEqual(persisted.fk,[]);assert.equal(persisted.schema,15);
    assert.equal(await digest(external),sha);
    outcomes.push({msg,...persisted});
    await page.screenshot({path:join(output,"restore-"+(i+1)+".png")});
  }
  await exact("查看待办",notice).click();
  await row("F4 restore task（冲突 2）").waitFor();
  return {sha,outcomes};
});
await phase("timeline-and-processing",async()=>{
  await nav("待办队列");
  await create("F4 work task");
  await row("F4 work task").click();
  const timeline=detail.locator(".timeline");
  const note=timeline.getByPlaceholder("补充一条普通处理备注");
  await note.fill("F4 original note");
  await exact("添加",timeline).click();
  const entry=timeline.locator(".timeline-entry").filter({hasText:"F4 original note"});
  await entry.getByRole("button",{name:"编辑",exact:true}).click();
  await entry.locator("textarea, input").fill("F4 edited note");
  await entry.getByRole("button",{name:"保存",exact:true}).click();
  await timeline.getByText("F4 edited note",{exact:true}).waitFor();
  await exact("本轮已处理",detail).click();
  const group=timeline.locator("button[aria-expanded]").first();
  await group.waitFor();
  await group.focus();await group.press("Enter");
  assert.equal(await group.getAttribute("aria-expanded"),"true");
  await group.press("Enter");
  assert.equal(await group.getAttribute("aria-expanded"),"false");
  await page.screenshot({path:join(output,"timeline.png")});
  await closeDetail();
});
await phase("statistics-real-window-and-range",async()=>{
  await nav("统计中心");
  const tabs=page.locator(".preset-tabs");
  assert.equal(await tabs.getByRole("button").count(),7);
  const periods=[];
  for(const name of ["本周","上周","本月","上月","本季度","上季度","自定义"]){
    const b=exact(name,tabs);await b.click();
    assert.equal(await b.getAttribute("aria-pressed"),"true");
    await page.locator(".summary-grid").waitFor();
    periods.push(await page.locator(".period-footer>small").innerText());
  }
  await exact("软件设置").click();
  await exact("周日",page.getByRole("group",{name:"每周起始日"})).click();
  await nav("统计中心");
  await exact("本周",tabs).click();
  assert.match(await page.locator(".period-footer>small").innerText(),/周日起算/);
  await page.locator(".trend-column .trend-total").first().click();
  await page.locator(".details-card tbody tr").first().waitFor();
  await page.locator(".details-card tbody tr").first().click();
  await detail.waitFor();
  await closeDetail();
  await nav("统计中心");
  assert.equal(await exact("本周",tabs).getAttribute("aria-pressed"),"true");
  const narrow=JSON.parse((await os("window","-Width","900","-Height","720")).stdout.trim());
  const fit=await tabs.evaluate(el=>[...el.querySelectorAll("button")].every(b=>b.getBoundingClientRect().right<=el.getBoundingClientRect().right+1));
  assert.ok(fit);
  await page.screenshot({path:join(output,"statistics-native.png")});
  return {periods,narrow};
});
await phase("native-report-clipboard",async()=>{
  await nav("统计中心");
  await exact("生成报告").click();
  await page.locator(".report-template-dialog").getByText("原生工作报告",{exact:true}).click();
  await exact("预览报告").click();
  const report=page.locator(".native-report-dialog");
  const title=await report.locator("h2").innerText();
  const range=await report.locator(".native-report-toolbar small").innerText();
  await exact("复制报告",report).click();
  const text=Buffer.from((await os("clipboard")).stdout.trim(),"base64").toString("utf8");
  assert.ok(text.includes(title) && text.includes("F4") && text.length>100);
  await exact("关闭",report.locator("footer")).click();
  return {title,range,chars:text.length};
});
await phase("real-mcp-native-approval",async()=>{
  await nav("待办队列");
  await create("F4 approval A");await create("F4 approval B");
  const read=()=>sql("SELECT t.id,t.title,t.planned_date,v.version FROM tasks t JOIN mcp_task_versions v ON v.task_id=t.id WHERE t.title LIKE 'F4 approval %' ORDER BY t.id");
  const pendingBatch=()=>sql("SELECT status FROM mcp_batch_approvals WHERE request_key IN ('f4-approve','f4-reject') ORDER BY id");
  const initial=await read();
  assert.equal(initial.length,2);
  const intent={summary:"Synthetic F4 integration approval",explicitUserRequest:true,confirmedRealWork:true};
  const common={intent,reason:"Synthetic F4 integration approval"};
  await withSidecar(async call=>{
    const capabilities=(await call("get_capabilities",{})).data;
    assert.equal(capabilities.schemaVersion,15);
    assert.equal(capabilities.mcpVersion,1);assert.equal(capabilities.tools.length,16);
    assert.equal(capabilities.permissions.fullRead,false);
    const dates=["2030-01-03","2030-01-04"];
    for(const [i,key] of ["f4-approve","f4-reject"].entries()){
      const old=await read();
      const raw={...common,idempotencyKey:key,operations:old.map(([taskId,, ,taskVersion])=>({operation:{action:"reschedule",target:{taskId},taskVersion,plannedDate:dates[i]}}))};
      const preview=await call("batch_mutate",{...raw,action:"dryRun"});
      assert.equal(preview.status,"ok");
      const apply={...raw,action:"apply",expectedBasis:preview.data.basis};
      const result=await call("batch_mutate",apply);
      assert.equal(result.data.commitStatus,"needs_user_approval");
      assert.deepEqual(await read(),old);
      await exact("软件设置").click();
      const batch=page.locator(".mcp-security-setting").filter({has:page.locator("summary").filter({hasText:/^批量操作/})});
      const summary=batch.locator(":scope > summary");
      if(await batch.getAttribute("open")===null)await summary.click();
      const card=batch.locator(".mcp-issued").filter({hasText:"批量改期"});
      await card.getByText("查看整批影响").click();
      await card.getByText(new RegExp(dates[i])).first().waitFor();
      if(i===0){
        await exact("审阅并确认",card).click();await exact("确认整批执行",card).click();
      } else {await exact("拒绝",card).click();}
      await card.waitFor({state:"hidden"});
      if(i===0)assert.ok((await read()).every(x=>x[2]===dates[0]));
      else assert.deepEqual(await read(),old);
      const replay=(await call("batch_mutate",apply)).data;
      assert.equal(replay.replayed,true);
    }
    const current=(await read())[0];
    const status=await call("mutate_task",{...common,idempotencyKey:"f4-status",action:"setStatus",target:{taskId:current[0]},taskVersion:current[3],status:"waiting_materials"});
    assert.equal(status.status,"ok");
    assert.ok(status.data.auditId);
    const undo=await call("request_undo",{...common,idempotencyKey:"f4-undo",auditId:status.data.auditId});
    assert.equal(undo.data.status,"needs_user_approval");
    assert.deepEqual(await sql("SELECT status FROM tasks WHERE id=?",[current[0]]),[["waiting_materials"]]);
    await nav("待办队列");await exact("软件设置").click();
    const audit=page.locator(".mcp-security-setting").filter({has:page.locator("summary").filter({hasText:/^AI 操作记录与撤销申请/})});
    await audit.locator(":scope > summary").click();
    const card=audit.locator(".mcp-issued").filter({hasText:"撤销申请 #"+status.data.auditId});
    await card.getByText("查看原操作前后变化").click();
    await exact("审阅并撤销",card).click();
    await exact("确认撤销",card).click();
    await card.waitFor({state:"hidden"});
    assert.deepEqual(await sql("SELECT status FROM tasks WHERE id=?",[current[0]]),[["pending"]]);
  });
  const decision=await pendingBatch();
  assert.equal(decision.length,2);
  assert.ok(decision.every(x=>x[0]!=="pending"));
  return {batchDecisions:decision.map(x=>x[0]),userUndoVerified:true};
});
await phase("native-png-save",async()=>{
  await nav("统计中心");
  const file=join(lab,"statistics-actual.png");
  const label=await page.locator(".period-footer>small").innerText();
  await exact("导出图表").click();
  await Promise.all([exact("导出 300 DPI PNG").click(),os("save","-Path",file)]);
  await page.getByText(/图表已导出/).first().waitFor();
  const bytes=await readFile(file);
  assert.equal(bytes.toString("hex",0,8),"89504e470d0a1a0a");
  let ppm;
  for(let offset=8;offset+12<=bytes.length;){
    const n=bytes.readUInt32BE(offset);
    if(bytes.toString("ascii",offset+4,offset+8)==="pHYs"){
      ppm=[bytes.readUInt32BE(offset+8),bytes.readUInt32BE(offset+12),bytes[offset+16]];break;
    }
    offset+=n+12;
  }
  assert.deepEqual(ppm,[11811,11811,1]);
  await copyFile(file,join(output,"statistics-actual.png"));
  return {label,bytes:bytes.length,dpi:ppm[0]*0.0254,sha:await digest(file)};
});
await browser.close();
if(evidence.some(x=>x.status==="FAIL"))process.exitCode=1;
