// Isolated browser regression: never connects to Tauri or the user's database.
// Use a local playwright installation or INLINE_PLAYWRIGHT_MODULE and INLINE_BROWSER_PATH.
import assert from "node:assert/strict";
import { pathToFileURL } from "node:url";
import { createServer } from "vite";

const { chromium } = await import(process.env.INLINE_PLAYWRIGHT_MODULE
  ? pathToFileURL(process.env.INLINE_PLAYWRIGHT_MODULE).href : "playwright");
const mode = process.argv[2] ?? "trend";
const mock = `
const task=(id,status="pending",parentTaskId=null)=>({id,permanentNumber:"IL-"+id,dailySequence:id,ticketDate:"2026-09-30",department:"测试部门",departments:["测试部门"],contact:"测试人员",contacts:["测试人员"],taskType:"合同审查",title:"隔离测试事项 "+id+" — 长标题与父子任务布局检查",details:"隔离测试详情",status,priority:"normal",workload:"standard",isUrgent:false,urgentRequester:"",urgentReason:"",requestedDeadline:null,requestedDeadlineLabel:null,internalNotes:"",createdAt:"2026-09-30T08:00:00+08:00",updatedAt:"2026-09-30T08:00:00+08:00",startedAt:null,completedAt:null,archivedAt:status==="completed"?"2026-09-30T08:00:00+08:00":null,deletedAt:null,customSortOrder:id,processingRounds:1,hasActiveQueue:status==="pending",deferredEnteredAt:status==="processed"?"2026-09-30T08:00:00+08:00":null,isImportConflict:false,parentTaskId,subtaskSortOrder:id});
const queue=Array.from({length:30},(_,i)=>task(i+1,i%3===2?"processed":"pending",i===1?1:null));
const archive=Array.from({length:10},(_,i)=>task(i+31,"completed",i===1?31:null));
const all=[...queue,...archive];
const settings={week_start_day:"monday",statistics_rate_mode:"processing"};
const callbacks=new Set();
window.__inlineCalls=[];
const detail=(id,status)=>({taskId:id,permanentNumber:"IL-"+id,title:all.find(t=>t.id===id).title,department:"测试部门",contact:"测试人员",resultStatus:status,firstHandledAt:"2026-09-28T08:00:00+08:00",lastHandledAt:"2026-09-28T08:00:00+08:00",handlingCount:1});
const details=[detail(1,"processed"),detail(2,"completed"),detail(3,"waiting_materials")];
const specific={bootstrap:async()=>({queue,archive,trash:[],masters:{departments:["测试部门"],contacts:["测试人员"],taskTypes:["合同审查"]},settings:{...settings},backups:[]}),getVersion:async()=>"0.4.0",globalShortcutAvailable:async()=>true,launchAtLogin:async()=>false,getTask:async id=>all.find(t=>t.id===id),listParentTaskCandidates:async()=>all.filter(t=>!t.parentTaskId),listSubtasks:async id=>all.filter(t=>t.parentTaskId===id),getLogs:async()=>[],getWorkEvents:async()=>[],listBackups:async()=>[],onDataChanged:cb=>{callbacks.add(cb);return()=>callbacks.delete(cb)},onNewTask:()=>()=>{},onTaskUiAction:()=>()=>{},setSetting:async(key,value)=>{settings[key]=String(value);callbacks.forEach(cb=>cb())},getStatistics:async(start,end)=>({range:{start,end},summary:{handledTasks:3,topLevelTasks:2,subtasks:1,processed:1,completed:1,waitingMaterials:1,waitingConfirmation:0,waitingCounterpartyConfirmation:0,rateMode:"processing",rateNumerator:2,rateDenominator:3,completionRate:66.67},byTaskType:[{taskType:"合同审查",handledTasks:3,completed:1,pendingFollowUp:2}],byDepartment:[{department:"测试部门",handledTasks:3,completed:1,pendingFollowUp:2}],trend:[{periodStart:start.slice(0,10),handledTasks:3,processed:1,completed:1}],trendGranularity:"day"}),getStatisticsDetails:async(...args)=>{window.__inlineCalls.push(["type",...args]);return details},getStatisticsTrendDetails:async(...args)=>{window.__inlineCalls.push(["trend",...args]);return args[2]?details.filter(t=>t.resultStatus===args[2]):details}};
const originalStatistics=specific.getStatistics;
const statisticsDays=new Set();
const localDay=value=>{const d=new Date(value);return d.getFullYear()+"-"+String(d.getMonth()+1).padStart(2,"0")+"-"+String(d.getDate()).padStart(2,"0")};
specific.getStatistics=async(...args)=>{const day=localDay(args[0]);statisticsDays.add(day);const data=await originalStatistics(...args);data.trend[0].periodStart=day;return data};
specific.getStatisticsTrendDetails=async(...args)=>{window.__inlineCalls.push(["trend",...args]);if(!statisticsDays.has(localDay(args[0])))return [];return args[2]?details.filter(t=>t.resultStatus===args[2]):details};
export const api=new Proxy(specific,{get:(target,key)=>target[key]??(async(...args)=>{window.__inlineCalls.push([key,...args]);return []})});
`;
const server = await createServer({ server: { port: 0, strictPort: false }, plugins: [{
  name: "isolated-ui-api", enforce: "pre",
  load(id) { if (id.replaceAll("\\", "/").endsWith("/src/api.ts")) return mock; }
}] });
await server.listen();
const browser = await chromium.launch({ headless: true, executablePath: process.env.INLINE_BROWSER_PATH });
const errors = [];
try {
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 }, timezoneId: "Asia/Shanghai" });
  page.setDefaultTimeout(15000);
  page.on("pageerror", error => errors.push(error.message));
  await page.goto(server.resolvedUrls.local[0]);
  await page.locator(".task-table tbody tr").first().waitFor();
  if (mode === "measure") {
    for (const width of [1440, 1280, 1200, 1920]) {
      await page.setViewportSize({width, height:900});
      await page.waitForTimeout(100);
      const dimensions=await page.locator(".table-scroll").evaluate(e=>({viewport:e.clientWidth,scroll:e.scrollWidth,table:e.querySelector("table").getBoundingClientRect().width,columns:[...e.querySelectorAll("th")].map(t=>t.getBoundingClientRect().width),lastHandle:e.querySelector("th:last-child .column-resize-handle").getBoundingClientRect().right-e.getBoundingClientRect().right}));
      console.log("BASELINE", width, dimensions);
    }
    await page.setViewportSize({width:1280,height:900});
    await page.addStyleTag({content:".task-table th:last-child .column-resize-handle{right:0}"});
    console.log("HANDLE INSIDE",await page.locator(".table-scroll").evaluate(e=>({viewport:e.clientWidth,scroll:e.scrollWidth})));
  }
  if (mode === "layout") {
    const dimensions=()=>page.locator(".table-scroll").evaluate(e=>({viewport:e.clientWidth,scroll:e.scrollWidth,table:e.querySelector("table").getBoundingClientRect().width}));
    const noOverflow=async()=>{await page.waitForFunction(()=>{const e=document.querySelector(".table-scroll");return e&&e.scrollWidth===e.clientWidth});const d=await dimensions();assert.equal(d.scroll,d.viewport,JSON.stringify(d));};
    for(const width of [1200,1280,1440,1920,1140]) {await page.setViewportSize({width,height:900});await noOverflow();}
    await page.setViewportSize({width:1101,height:900});await page.waitForTimeout(80);
    let narrow=await dimensions();assert.ok(narrow.viewport<854&&narrow.scroll>=854);
    await page.setViewportSize({width:1440,height:900});
    await page.locator(".task-table tbody tr").first().click();
    await page.locator(".detail-panel").waitFor();
    await page.waitForTimeout(80);
    let d=await dimensions();assert.ok(d.scroll>d.viewport,JSON.stringify(d));
    await page.locator(".detail-panel>header").getByRole("button",{name:"关闭",exact:true}).click();
    await noOverflow();
    // A real user resize (keyboard-accessible handle), then restart and read back storage.
    const handle=page.getByRole("separator",{name:"调整事项标题列宽",exact:true});
    for(let i=0;i<40;i++)await handle.press("ArrowRight");
    const saved=await page.evaluate(()=>localStorage.getItem("in-line-task-column-layouts"));
    await page.reload();await page.locator(".task-table tbody tr").first().waitFor();
    await page.waitForTimeout(80);d=await dimensions();assert.ok(d.scroll>d.viewport);
    assert.equal(await page.evaluate(()=>localStorage.getItem("in-line-task-column-layouts")),saved);
    await page.locator(".task-table th").first().click({button:"right"});
    await page.getByRole("menuitem",{name:"恢复本页默认列宽"}).click();
    await noOverflow();
    for(const label of ["暂缓事项","历史归档","待办队列"]){await page.locator(".sidebar nav button").filter({hasText:label}).click();await noOverflow();}
    for(const scale of [1.25,1.5,2]){
      const context=await browser.newContext({viewport:{width:1280,height:900},deviceScaleFactor:scale});
      const scaled=await context.newPage();await scaled.goto(server.resolvedUrls.local[0]);await scaled.locator(".task-table tbody tr").first().waitFor();await scaled.waitForTimeout(80);
      await scaled.waitForFunction(()=>{const e=document.querySelector(".table-scroll");return e&&e.scrollWidth===e.clientWidth});
      assert.equal(await scaled.locator(".table-scroll").evaluate(e=>e.scrollWidth-e.clientWidth),0);
      await context.close();
    }
    console.log("PASS layout: default/resize/large/detail/custom/restart/reset/pages/DPI 125%-200%");
  }
  if (mode === "trend") {
    await page.locator(".sidebar nav button").filter({ hasText: "统计中心" }).click();
    const first = page.locator(".trend-column").first();
    await first.locator(".trend-result-controls button").first().click();
    await page.locator(".details-card .statistics-table tbody tr").waitFor();
    assert.equal(await page.locator(".details-card tbody tr").count(), 1);
    assert.match(await page.locator(".details-card").innerText(), /已处理/);
    await first.locator(".trend-result-controls button").last().click();
    await page.locator(".details-card h2").filter({ hasText: "已完成" }).waitFor();
    assert.equal(await page.locator(".details-card tbody tr").count(), 1);
    await first.locator(".trend-total").click();
    await page.locator(".details-card tbody tr").last().waitFor();
    assert.equal(await page.locator(".details-card tbody tr").count(), 3);
    await page.getByRole("button", { name: "收起明细" }).click();
    assert.equal(await page.locator(".details-card").count(), 0);
    await page.locator(".trend-column").nth(1).locator(".trend-result-controls button").first().click();
    await page.getByText("该范围内没有对应事项。").waitFor();
    // Same API fixture only returns events on the first day.
    await page.locator(".type-stats button, .task-type-pie-legend button").first().click();
    await page.locator(".details-card tbody tr").last().waitFor();
    assert.equal(await page.locator(".details-card tbody tr").count(), 3);
    const calls = await page.evaluate(() => window.__inlineCalls);
    assert.deepEqual(calls.filter(c => c[0] === "trend").slice(0, 3).map(c => c[3]), ["processed", "completed", null]);
    const firstCall = calls.find(c => c[0] === "trend");
    assert.equal(new Date(firstCall[2]) - new Date(firstCall[1]), 86400000);
    await page.locator(".details-card tbody tr").first().click();
    await page.locator(".detail-panel").waitFor();
    console.log("PASS trend: category/total/zero/type details, close, task navigation, day range");
  }
  assert.deepEqual(errors, []);
} catch (error) { console.error("UI failure:", error.message, errors); throw error; }
finally { await browser.close(); await server.close(); }
