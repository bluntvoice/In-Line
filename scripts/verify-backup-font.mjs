// Isolated browser integration: local fixtures only, no native user database.
import assert from "node:assert/strict";
import {pathToFileURL} from "node:url";
import {mkdir} from "node:fs/promises";
import {createServer} from "vite";
const {chromium}=await import(process.env.INLINE_PLAYWRIGHT_MODULE?pathToFileURL(process.env.INLINE_PLAYWRIGHT_MODULE).href:"playwright");
const mock=`
const task={id:1,title:"字体测试事项 Agjy 中华人民共和国",status:"pending",isScheduled:false,plannedDate:"2026-10-03",permanentNumber:"20261003-01",dailySequence:1,ticketDate:"2026-10-03",department:"测试",departments:["测试"],contact:"测试",contacts:["测试"],taskType:"合同审查",details:"测试",priority:"normal",workload:"standard",isUrgent:false,urgentRequester:"",urgentReason:"",requestedDeadline:null,requestedDeadlineLabel:null,internalNotes:"",createdAt:"2026-10-03T08:00:00+08:00",updatedAt:"2026-10-03T08:00:00+08:00",startedAt:null,completedAt:null,archivedAt:null,deletedAt:null,customSortOrder:1,processingRounds:0,hasActiveQueue:true,deferredEnteredAt:null,isImportConflict:false,parentTaskId:null,subtaskSortOrder:1};
const callbacks=new Set(),settings={ui_font_family:localStorage.getItem("backup-font-selection")??""};
const channel=new BroadcastChannel("backup-font-test");channel.onmessage=event=>{settings.ui_font_family=event.data;callbacks.forEach(cb=>cb())};
window.__cleanupCalls=0;window.__cleanupFail=false;window.__cleanupError=false;window.__fontReadFail=false;
const seed=count=>window.__backups=Array.from({length:count},(_,i)=>({name:"InLine-backup-"+(count-i)+"-"+(["manual","auto","import","before-restore"][i%4])+".db",path:"isolated/"+(count-i)+".db",size:1024,modifiedAt:new Date(1700000000000+(count-i)*1000).toISOString()}));seed(9);window.__seedBackups=seed;
const fonts=[{family:"Arial",displayName:"Arial",aliases:["Arial"],cjk:false},{family:"Sarasa UI SC",displayName:"更纱黑体 UI SC",aliases:["Sarasa UI SC","更纱黑体 UI SC"],cjk:true}];
const specific={bootstrap:async()=>({queue:[task],archive:[],trash:[],masters:{departments:["测试"],contacts:["测试"],taskTypes:["合同审查"]},settings,backups:window.__backups}),getVersion:async()=>"0.5.0",globalShortcutAvailable:async()=>true,launchAtLogin:async()=>false,getFloatingVisible:async()=>false,getUIScale:async()=>"100",getRecommendedFontStatus:async()=>({phase:"idle",downloadedBytes:0,totalBytes:25443648,percent:0,message:null}),onRecommendedFontProgress:()=>()=>{},onFloatingVisibilityChanged:()=>()=>{},listBackups:async()=>window.__backups,onDataChanged:cb=>{callbacks.add(cb);return()=>callbacks.delete(cb)},onNewTask:()=>()=>{},onTaskUiAction:()=>()=>{},getTask:async()=>task,listSubtasks:async()=>[],listParentTaskCandidates:async()=>[],getLogs:async()=>[],getWorkEvents:async()=>[],getTicketColors:async()=>null,listSystemFonts:async()=>fonts,getUIFontSelection:async()=>{if(window.__fontReadFail)throw new Error("读取失败");const requested=settings.ui_font_family,effective=fonts.some(f=>f.family===requested)?requested:"";return{requested,effective,missing:Boolean(requested&&!effective)}},setSetting:async(key,value)=>{settings[key]=value;if(key==="ui_font_family"){localStorage.setItem("backup-font-selection",value);channel.postMessage(value)}callbacks.forEach(cb=>cb())},getUpdateProgress:async()=>({phase:"idle",version:null,downloadedBytes:0,totalBytes:null,percent:null,message:null}),onUpdateProgress:()=>()=>{},cleanupBackups:async()=>{window.__cleanupCalls++;if(window.__cleanupError)throw new Error("隔离清理失败");if(window.__pauseCleanup)await new Promise(resolve=>window.__releaseCleanup=resolve);const old=window.__backups.slice(5),failed=window.__cleanupFail?old.slice(-1):[];window.__backups=[...window.__backups.slice(0,5),...failed];return{deletedCount:old.length-failed.length,failures:failed.map(b=>({name:b.name,reason:"文件被占用"})),backups:window.__backups}}};
window.__restoreFont=value=>{settings.ui_font_family=value;localStorage.setItem("backup-font-selection",value);callbacks.forEach(cb=>cb())};
specific.getRecommendedFontStatus=async()=>({phase:"idle",downloadedBytes:0,totalBytes:25443648,percent:0,message:null});specific.onRecommendedFontProgress=()=>()=>{};specific.getUIScale=async()=>"100";
export const api=new Proxy(specific,{get:(target,key)=>target[key]??(async()=>[])});
`;
const server=await createServer({server:{port:0,strictPort:false},plugins:[{name:"isolated-backup-font",enforce:"pre",load(id){if(id.replaceAll("\\","/").endsWith("/src/api.ts"))return mock;}}]});
await server.listen();
const browser=await chromium.launch({headless:true,executablePath:process.env.INLINE_BROWSER_PATH});
const errors=[],fontErrors=[];
try{
  const context=await browser.newContext({viewport:{width:1280,height:900},timezoneId:"Asia/Shanghai"});
  const page=await context.newPage();page.on("pageerror",error=>errors.push(error.message));page.on("response",response=>{if(response.url().endsWith(".woff2")&&response.status()>=400)fontErrors.push(response.status())});
  await page.goto(server.resolvedUrls.local[0],{waitUntil:"domcontentloaded"});
  await page.locator(".task-table tbody tr").first().waitFor();
  await page.locator(".settings-button").filter({hasText:"软件设置"}).click();
  const cleanup=page.getByRole("button",{name:"仅保留最近 5 个",exact:true});
  assert.match(await page.locator(".font-setting-trigger").innerText(),/系统默认字体/);
  page.once("dialog",dialog=>dialog.dismiss());await cleanup.click();assert.equal(await page.evaluate(()=>window.__cleanupCalls),0);
  await page.evaluate(()=>{window.__cleanupFail=true;window.__pauseCleanup=true});
  page.once("dialog",dialog=>{assert.match(dialog.message(),/删除 4 个/);return dialog.accept()});await cleanup.click();
  await page.waitForFunction(()=>Boolean(window.__releaseCleanup));
  assert.equal(await page.getByRole("button",{name:"立即备份",exact:true}).isDisabled(),true);
  await page.evaluate(()=>{window.__releaseCleanup();window.__pauseCleanup=false});
  await page.locator(".backup-cleanup-feedback").filter({hasText:"1 个删除失败"}).waitFor();
  assert.equal(await page.locator(".backup-list article").count(),6);
  await page.evaluate(()=>window.__cleanupFail=false);page.once("dialog",dialog=>dialog.accept());await cleanup.click();
  await page.locator(".backup-cleanup-feedback").filter({hasText:"保留 5 个"}).waitFor();
  assert.equal(await page.locator(".backup-list article").count(),5);assert.equal(await cleanup.isDisabled(),true);
  await page.evaluate(()=>{window.__seedBackups(7);window.__cleanupError=true});await page.getByRole("button",{name:"刷新列表",exact:true}).click();
  page.once("dialog",dialog=>dialog.accept());await cleanup.click();await page.locator(".backup-cleanup-feedback").filter({hasText:"隔离清理失败"}).waitFor();
  assert.equal(await page.locator(".backup-list article").count(),7);
  for(const count of [0,1,5]){await page.evaluate(count=>window.__seedBackups(count),count);await page.getByRole("button",{name:"刷新列表",exact:true}).click();assert.equal(await cleanup.isDisabled(),true);}
  await page.evaluate(()=>{window.__seedBackups(9);window.__cleanupError=false});await page.getByRole("button",{name:"刷新列表",exact:true}).click();
  await page.locator(".font-setting-trigger").click();await page.getByRole("option").filter({has:page.getByText("Arial",{exact:true})}).waitFor();
  assert.equal(await page.locator(".font-picker-list [role=option]").count(),3); // Default plus the two real system families.
  await mkdir("release/ui",{recursive:true});await page.screenshot({path:"release/ui/system-font-picker.png"});
  await page.getByRole("option").filter({has:page.getByText("Arial",{exact:true})}).click();await page.waitForFunction(()=>document.documentElement.getAttribute("data-ui-font")==="Arial");
  const secondary=await context.newPage();await secondary.goto(server.resolvedUrls.local[0]+"#floating");await secondary.waitForFunction(()=>document.documentElement.getAttribute("data-ui-font")==="Arial");
  await page.reload();await page.locator(".task-table tbody tr").first().waitFor();await page.waitForFunction(()=>document.documentElement.getAttribute("data-ui-font")==="Arial");
  await page.locator(".settings-button").filter({hasText:"软件设置"}).click();await page.locator(".font-setting-row").getByRole("button",{name:"恢复默认",exact:true}).click();
  await page.waitForFunction(()=>document.documentElement.getAttribute("data-ui-font")==="default");await secondary.waitForFunction(()=>document.documentElement.getAttribute("data-ui-font")==="default");
  await page.evaluate(()=>window.__restoreFont("Missing test font"));await page.locator(".font-fallback-note").filter({hasText:"不可用"}).waitFor();
  assert.match(await page.locator(".font-setting-trigger").innerText(),/系统默认字体/);assert.equal(await page.evaluate(()=>localStorage.getItem("backup-font-selection")),"Missing test font");
  await page.evaluate(()=>window.__restoreFont("Sarasa UI SC"));await page.waitForFunction(()=>document.documentElement.getAttribute("data-ui-font")==="Sarasa UI SC");
  await page.locator(".font-setting-row").getByRole("button",{name:"恢复默认",exact:true}).click();
  for(const hash of ["floating","quick-add","update-progress"]){await secondary.goto(server.resolvedUrls.local[0]+"#"+hash);await secondary.waitForFunction(()=>document.documentElement.getAttribute("data-ui-font")==="default");await secondary.evaluate(()=>document.fonts.ready);}
  await page.locator(".backup-list").scrollIntoViewIfNeeded();await page.screenshot({path:"release/ui/backup-cleanup.png"});
  for(const width of [1280,980]){await page.setViewportSize({width,height:900});assert.ok(await page.locator(".settings-page").evaluate(e=>e.scrollWidth<=e.clientWidth+1));}
  assert.deepEqual(errors,[]);assert.deepEqual(fontErrors,[]);
  console.log("PASS: system font default; default/custom/reload/missing/auxiliary windows; cleanup cancel/count/disable/partial failure/retry/errors; settings layout");
}finally{await browser.close();await server.close();}
