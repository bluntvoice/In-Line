// Synthetic browser/API fixtures only; never calls the installed app or user database.
import assert from "node:assert/strict";
import { pathToFileURL } from "node:url";
import { createServer } from "vite";
const {chromium}=await import(pathToFileURL(process.env.INLINE_PLAYWRIGHT_MODULE).href);
const mock=`
const day=(d=new Date())=>d.getFullYear()+"-"+String(d.getMonth()+1).padStart(2,"0")+"-"+String(d.getDate()).padStart(2,"0");
const later=new Date();later.setDate(later.getDate()+3);
const task=(id,scheduled=false,urgent=false)=>({id,title:"配色事项"+id,status:"pending",plannedDate:scheduled?day(later):day(),isScheduled:scheduled,scheduleAction:"",scheduleActionAt:null,permanentNumber:"FIXED-"+id,dailySequence:id,ticketDate:scheduled?day(later):day(),department:"测试部门",departments:["测试部门"],contact:"测试人员",contacts:["测试人员"],taskType:"合同审查",details:"隔离数据",priority:"normal",workload:"standard",isUrgent:urgent,urgentRequester:"",urgentReason:"",requestedDeadline:null,requestedDeadlineLabel:null,internalNotes:"",createdAt:new Date().toISOString(),updatedAt:new Date().toISOString(),startedAt:null,completedAt:null,archivedAt:null,deletedAt:null,customSortOrder:id,processingRounds:0,hasActiveQueue:!scheduled,deferredEnteredAt:null,isImportConflict:false,parentTaskId:null,subtaskSortOrder:0,ticketColor:null});
const key="inline-single-color-test",read=()=>JSON.parse(localStorage.getItem(key)||JSON.stringify([task(1),task(2,false,true),task(3,true)]));
const callbacks=new Set(),channel=new BroadcastChannel(key),emit=()=>callbacks.forEach(cb=>cb());channel.onmessage=()=>{window.__channelEvents=(window.__channelEvents||0)+1;emit()};window.__readFixture=read;window.__callbacks=callbacks;
// Renderer localStorage replication can arrive after BroadcastChannel; emit on committed storage too.
window.addEventListener("storage",event=>{if(event.key===key)emit()});
const change=(id,values)=>{localStorage.setItem(key,JSON.stringify(read().map(t=>t.id===id?{...t,...values}:t)));emit();channel.postMessage("changed")};
window.__colorSaveCalls=0;window.__updateTask=change;
const specific={bootstrap:async()=>{const data={queue:read(),archive:[],trash:[],masters:{departments:["测试部门"],contacts:["测试人员"],taskTypes:["合同审查"]},settings:{},backups:[]};window.__lastBootstrap=data;return data},getTask:async id=>read().find(t=>t.id===id),setTaskTicketColor:async(id,color)=>{window.__colorSaveCalls++;if(window.__failColorSave)throw Error("隔离保存失败");change(id,{ticketColor:color})},getVersion:async()=>"0.5.0",getTicketColors:async()=>null,onDataChanged:cb=>{callbacks.add(cb);return()=>callbacks.delete(cb)},onNewTask:()=>()=>{},onTaskUiAction:()=>()=>{},getUIFontSelection:async()=>({requested:"",effective:"",missing:false}),getUIScale:async()=>"100"};
specific.createSubtask=async input=>{window.__lastSubtask=input;const current=read(),created={...task(4,input.plannedDate>day()),...input,id:4,title:input.title,parentTaskId:input.parentTaskId,isScheduled:input.plannedDate>day(),hasActiveQueue:input.plannedDate===day()&&input.enqueueToday};localStorage.setItem(key,JSON.stringify([...current,created]));emit();channel.postMessage("changed");return created};
export const api=new Proxy(specific,{get:(target,key)=>target[key]??(async()=>[])});`;
const server=await createServer({server:{host:"127.0.0.1",port:0,watch:{ignored:["**/src-tauri/**","**/release/**","**/docs/**"]}},plugins:[{name:"isolated-single-color",enforce:"pre",load(id){if(id.replaceAll("\\","/").endsWith("/src/api.ts"))return mock;}}]});
await server.listen();
const browser=await chromium.launch({headless:true,executablePath:process.env.INLINE_BROWSER_PATH,args:["--disable-background-timer-throttling","--disable-backgrounding-occluded-windows","--disable-renderer-backgrounding"]});
const errors=[];
try{
  const context=await browser.newContext({viewport:{width:1440,height:1000},timezoneId:"Asia/Shanghai"});
  const page=await context.newPage();page.on("pageerror",e=>errors.push(e.message));
  await page.goto(server.resolvedUrls.local[0],{waitUntil:"domcontentloaded"});
  const row=id=>page.locator(".task-table tbody tr").filter({hasText:"配色事项"+id});
  await row(1).waitFor();
  const floating=await context.newPage();floating.on("pageerror",e=>errors.push(e.message));
  await floating.setViewportSize({width:444,height:564});await floating.goto(server.resolvedUrls.local[0]+"#/floating",{waitUntil:"domcontentloaded"});
  const card=id=>floating.locator(".floating-card").filter({hasText:"配色事项"+id});await card(1).waitFor();
  const rgb=async(locator,color,text)=>{
    const start=Date.now();let actual;
    while(Date.now()-start<5000){
      actual=await locator.evaluate(e=>({background:getComputedStyle(e).backgroundColor,text:getComputedStyle(e).color}));
      if(actual.background===color&&(!text||actual.text===text))return;
      await new Promise(resolve=>setTimeout(resolve,30));
    }
    console.error("fixture diagnostics",await page.evaluate(()=>({callbacks:[...window.__callbacks].map(c=>c.toString()),events:window.__channelEvents,colors:window.__readFixture().map(t=>({id:t.id,color:t.ticketColor,priority:t.priority})),bootstrap:window.__lastBootstrap.queue.map(t=>({id:t.id,color:t.ticketColor,priority:t.priority})),html:[...document.querySelectorAll('.task-table .ticket-number')].map(e=>e.outerHTML)})));
    assert.fail("color mismatch "+JSON.stringify({actual,color,text}));
  };
  const open=async(target,source)=>{await source.bringToFront();await target.click({button:"right"});await source.getByRole("button",{name:"编号配色",exact:true}).click();await source.getByRole("dialog").waitFor();};
  await open(row(1),page);
  assert.equal(await page.locator(".task-ticket-color-picker .ticket-color-options button").count(),16);
  const input=page.getByRole("textbox",{name:"事项编号 HEX 编码"});
  await input.fill("invalid");await input.press("Enter");await page.getByRole("alert").filter({hasText:"请输入"}).waitFor();
  assert.equal(await page.evaluate(()=>window.__colorSaveCalls),0);
  await page.evaluate(()=>window.__failColorSave=true);await input.fill("abc");await input.press("Enter");await page.getByRole("alert").filter({hasText:"保存失败"}).waitFor();
  await rgb(row(1).locator(".ticket-number"),"rgb(11, 58, 130)");
  await page.evaluate(()=>window.__failColorSave=false);await input.press("Enter");await page.getByRole("dialog").waitFor({state:"hidden"});
  await rgb(row(1).locator(".ticket-number"),"rgb(170, 187, 204)");await rgb(card(1).locator(".ticket-number"),"rgb(170, 187, 204)");
  await rgb(row(2).locator(".ticket-number"),"rgb(196, 61, 75)");
  await open(card(2),floating);await floating.getByRole("button",{name:"奶油黄 #F3D98B",exact:true}).click();await floating.getByRole("dialog").waitFor({state:"hidden"});
  await rgb(card(2).locator(".ticket-number"),"rgb(243, 217, 139)","rgb(0, 0, 0)");await page.bringToFront();await rgb(row(2).locator(".ticket-number"),"rgb(243, 217, 139)");
  await page.reload({waitUntil:"domcontentloaded"});await row(1).waitFor();await rgb(row(1).locator(".ticket-number"),"rgb(170, 187, 204)");
  await page.locator(".sidebar nav button").filter({hasText:"暂缓事项"}).click();
  await open(row(3),page);await page.getByRole("button",{name:"灰紫 #7A6687",exact:true}).click();await page.getByRole("dialog").waitFor({state:"hidden"});
  await rgb(row(3).locator(".ticket-number"),"rgb(122, 102, 135)");
  await page.locator(".sidebar nav button").filter({hasText:"待办队列"}).click();
  await page.evaluate(()=>window.__updateTask(1,{priority:"critical"}));await rgb(row(1).locator(".ticket-number"),"rgb(196, 61, 75)");
  await page.evaluate(()=>window.__updateTask(1,{priority:"normal",requestedDeadline:new Date(Date.now()-60000).toISOString()}));await rgb(row(1).locator(".ticket-number"),"rgb(196, 61, 75)");
  await page.evaluate(()=>window.__updateTask(1,{requestedDeadline:null}));await rgb(row(1).locator(".ticket-number"),"rgb(170, 187, 204)");
  await open(row(1),page);await page.getByRole("button",{name:"恢复自动配色",exact:true}).click();await page.getByRole("dialog").waitFor({state:"hidden"});await rgb(row(1).locator(".ticket-number"),"rgb(11, 58, 130)");
  await open(card(1),floating);assert.equal(await floating.locator(".task-ticket-color-picker").evaluate(e=>e.scrollWidth<=e.clientWidth),true);await floating.keyboard.press("Escape");await floating.getByRole("dialog").waitFor({state:"hidden"});
  await page.bringToFront();await row(1).click({button:"right"});await page.getByRole("button",{name:"添加子任务",exact:true}).click();
  const subtask=page.locator(".subtask-create-panel"),date=subtask.locator('input[type="date"]'),toggle=subtask.locator('.subtask-enqueue-switch input');
  await subtask.waitFor();assert.equal(await toggle.isChecked(),true);assert.equal(await toggle.isEnabled(),true);await toggle.uncheck();
  const dates=await page.evaluate(()=>({today:window.__readFixture().find(t=>t.id===1).plannedDate,future:window.__readFixture().find(t=>t.id===3).plannedDate}));
  await date.fill(dates.future);assert.equal(await toggle.count(),0);await subtask.getByRole("status",{name:"未来日期自动预占号码已开启"}).waitFor();
  await date.fill(dates.today);assert.equal(await toggle.isChecked(),true);assert.equal(await toggle.isEnabled(),true);
  await date.fill(dates.future);await subtask.locator('input[maxlength="100"]').fill("未来子任务");await subtask.getByRole("button",{name:"创建并提前取号",exact:true}).click();await subtask.waitFor({state:"hidden"});
  assert.deepEqual(await page.evaluate(()=>({date:window.__lastSubtask.plannedDate,enqueue:window.__lastSubtask.enqueueToday})),{date:dates.future,enqueue:false});
  await page.locator(".sidebar nav button").filter({hasText:"暂缓事项"}).click();await row(3).click({button:"right"});await page.getByRole("button",{name:"添加子任务",exact:true}).click();await subtask.waitFor();
  assert.equal(await date.inputValue(),dates.future);assert.equal(await toggle.count(),0);await subtask.getByRole("status",{name:"未来日期自动预占号码已开启"}).waitFor();await subtask.getByRole("button",{name:"取消",exact:true}).click();
  assert.deepEqual(errors,[]);
  console.log("PASS: right-click main/floating, 16 colors, invalid HEX, failed save/retry, per-task isolation, cross-window sync, reload, future/urgent custom colors, overdue/critical overrides and restoration, reset, narrow dialog, Escape; future subtask auto-reservation status, date switching, inherited future date, today toggle and submit payload; synthetic API only.");
}finally{await browser.close();await server.close();}
