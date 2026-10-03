// Isolated browser regression: never connects to Tauri or the user's database.
// Use a local playwright installation or INLINE_PLAYWRIGHT_MODULE and INLINE_BROWSER_PATH.
import assert from "node:assert/strict";
import { pathToFileURL } from "node:url";
import { createServer } from "vite";
import {execFile} from "node:child_process";
import {promisify} from "node:util";

const { chromium } = await import(process.env.INLINE_PLAYWRIGHT_MODULE
  ? pathToFileURL(process.env.INLINE_PLAYWRIGHT_MODULE).href : "playwright");
const mode = process.argv[2] ?? "trend";
const nativeFonts=mode==="fonts"?JSON.parse((await promisify(execFile)("cargo",["run","--locked","--manifest-path","src-tauri/Cargo.toml","--example","list_system_fonts"],{maxBuffer:4*1024*1024})).stdout):[];
const mock = `
const task=(id,status="pending",parentTaskId=null)=>({id,permanentNumber:"IL-"+id,dailySequence:id,ticketDate:"2026-09-30",department:"测试部门",departments:["测试部门"],contact:"测试人员",contacts:["测试人员"],taskType:"合同审查",title:"隔离测试事项 "+id+" — 长标题与父子任务布局检查",details:"隔离测试详情",status,priority:"normal",workload:"standard",isUrgent:false,urgentRequester:"",urgentReason:"",requestedDeadline:null,requestedDeadlineLabel:null,internalNotes:"",createdAt:"2026-09-30T08:00:00+08:00",updatedAt:"2026-09-30T08:00:00+08:00",startedAt:null,completedAt:null,archivedAt:status==="completed"?"2026-09-30T08:00:00+08:00":null,deletedAt:null,customSortOrder:id,processingRounds:1,hasActiveQueue:status==="pending",deferredEnteredAt:status==="processed"?"2026-09-30T08:00:00+08:00":null,isImportConflict:false,parentTaskId,subtaskSortOrder:id});
const queue=Array.from({length:30},(_,i)=>task(i+1,i%3===2?"processed":"pending",i===1?1:null));
const archive=Array.from({length:10},(_,i)=>task(i+31,"completed",i===1?31:null));
const all=[...queue,...archive];
queue.find(t=>t.id===9).parentTaskId=3;
const settings={week_start_day:"monday",statistics_rate_mode:"processing"};
settings.ui_font_family=localStorage.getItem("isolated-ui-font")??"";
const callbacks=new Set();
window.__inlineCalls=[];
const detail=(id,status)=>({taskId:id,taskType:"合同审查",hasProcessedOrCompleted:true,permanentNumber:"IL-"+id,title:all.find(t=>t.id===id).title,department:"测试部门",contact:"测试人员",resultStatus:status,firstHandledAt:"2026-09-28T08:00:00+08:00",lastHandledAt:"2026-09-28T08:00:00+08:00",handlingCount:id===3?2:1});
const details=[detail(1,"processed"),detail(2,"completed"),detail(3,"waiting_materials")];
const specific={bootstrap:async()=>({queue,archive,trash:[],masters:{departments:["测试部门"],contacts:["测试人员"],taskTypes:["合同审查"]},settings:{...settings},backups:[]}),getVersion:async()=>"0.4.0",globalShortcutAvailable:async()=>true,launchAtLogin:async()=>false,getFloatingVisible:async()=>false,getUIScale:async()=>"100",getRecommendedFontStatus:async()=>({phase:"idle",downloadedBytes:0,totalBytes:25443648,percent:0,message:null}),onRecommendedFontProgress:()=>()=>{},onFloatingVisibilityChanged:()=>()=>{},getTask:async id=>all.find(t=>t.id===id),listParentTaskCandidates:async()=>all.filter(t=>!t.parentTaskId),listSubtasks:async id=>all.filter(t=>t.parentTaskId===id),getLogs:async()=>[],getWorkEvents:async()=>[],listBackups:async()=>[],onDataChanged:cb=>{callbacks.add(cb);return()=>callbacks.delete(cb)},onNewTask:()=>()=>{},onTaskUiAction:()=>()=>{},setSetting:async(key,value)=>{settings[key]=String(value);callbacks.forEach(cb=>cb())},getStatistics:async(start,end)=>({range:{start,end},summary:{handledTasks:3,topLevelTasks:2,subtasks:1,processed:1,completed:1,waitingMaterials:1,waitingConfirmation:0,waitingCounterpartyConfirmation:0,rateMode:"processing",rateNumerator:2,rateDenominator:3,completionRate:66.67},byTaskType:[{taskType:"合同审查",handledTasks:3,completed:1,pendingFollowUp:2}],byDepartment:[{department:"测试部门",handledTasks:3,completed:1,pendingFollowUp:2}],trend:[{periodStart:start.slice(0,10),handledTasks:3,processed:1,completed:1}],trendGranularity:"day"}),getStatisticsDetails:async(...args)=>{window.__inlineCalls.push(["type",...args]);return details},getStatisticsTrendDetails:async(...args)=>{window.__inlineCalls.push(["trend",...args]);return args[2]?details.filter(t=>t.resultStatus===args[2]):details}};
const originalStatistics=specific.getStatistics;
const fonts=${JSON.stringify(nativeFonts)};
specific.listSystemFonts=async()=>{window.__inlineCalls.push(["fonts"]);return fonts};
specific.getUIFontSelection=async()=>{const requested=settings.ui_font_family??"",effective=fonts.find(f=>f.family===requested)?.family??"";return{requested,effective,missing:Boolean(requested&&!effective)}};
const originalSet=specific.setSetting;
specific.setSetting=async(key,value)=>{if(key==="ui_font_family")localStorage.setItem("isolated-ui-font",String(value));return originalSet(key,value)};
window.addEventListener("storage",event=>{if(event.key==="isolated-ui-font"){settings.ui_font_family=event.newValue??"";callbacks.forEach(cb=>cb())}});
window.__inlineRestoreFont=value=>{settings.ui_font_family=value;localStorage.setItem("isolated-ui-font",value);callbacks.forEach(cb=>cb())};
specific.getUpdateProgress=async()=>({phase:"idle",version:null,downloadedBytes:0,totalBytes:null,percent:null,message:null});
specific.onUpdateProgress=()=>()=>{};
const statisticsDays=new Set();
const localDay=value=>{const d=new Date(value);return d.getFullYear()+"-"+String(d.getMonth()+1).padStart(2,"0")+"-"+String(d.getDate()).padStart(2,"0")};
specific.getStatistics=async(...args)=>{const day=localDay(args[0]);statisticsDays.add(day);const data=await originalStatistics(...args);data.trend[0].periodStart=day;if(new Date(args[1])-new Date(args[0])>62*86400000){const monday=new Date(args[0]);monday.setDate(monday.getDate()-(monday.getDay()+6)%7);data.trendGranularity="week";data.trend[0].periodStart=localDay(monday);}return data};
specific.getStatisticsTrendDetails=async(...args)=>{window.__inlineCalls.push(["trend",...args]);if(!statisticsDays.has(localDay(args[0])))return [];return args[2]?details.filter(t=>t.resultStatus===args[2]):details};
specific.copyText=async text=>{if(window.__inlineCopyFail)throw new Error("隔离剪贴板失败");window.__inlineCalls.push(["copyText",text]);window.__inlineClipboard=text};
specific.getRecommendedFontStatus=async()=>({phase:"idle",downloadedBytes:0,totalBytes:25443648,percent:0,message:null});specific.onRecommendedFontProgress=()=>()=>{};specific.getUIScale=async()=>"100";
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
  const mainContext = await browser.newContext({ viewport: { width: 1440, height: 900 }, timezoneId: "Asia/Shanghai" });
  const page = await mainContext.newPage();
  page.setDefaultTimeout(15000);
  page.on("pageerror", error => errors.push(error.message));
  await page.goto(server.resolvedUrls.local[0],{waitUntil:"domcontentloaded",timeout:45000});
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
    for(const width of [1200,1280,1440,1920,1170]) {await page.setViewportSize({width,height:900});await noOverflow();}
    await page.setViewportSize({width:1101,height:900});await page.waitForTimeout(80);
    let narrow=await dimensions();assert.ok(narrow.viewport<882&&narrow.scroll>=882);
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
  if (mode === "rows") {
    for(const width of [1440,1280]){
      await page.setViewportSize({width,height:900});
      for(const label of ["待办队列","暂缓事项","历史归档"]){
        await page.locator(".sidebar nav button").filter({hasText:label}).click();
        await page.locator(".task-table tbody tr").first().waitFor();
        const result=await page.locator(".task-table").evaluate(table=>{
          const rows=[...table.querySelectorAll("tbody tr")];
          const heights=rows.map(row=>row.getBoundingClientRect().height);
          const clipped=rows.flatMap(row=>[...row.querySelectorAll(".ticket-number,.status-badge,.row-actions button,.task-title-line>strong,.parent-task-line")].filter(e=>{const a=e.getBoundingClientRect(),b=row.getBoundingClientRect();return a.top<b.top||a.bottom>b.bottom}).map(e=>e.className));
          return{heights,clipped,parents:table.querySelectorAll(".parent-task-line").length};
        });
        assert.ok(result.heights.length>0&&result.heights.every(height=>height===64),JSON.stringify(result));
        assert.deepEqual(result.clipped,[]);assert.ok(result.parents>0);
      }
    }
    await page.reload();await page.locator(".task-table tbody tr").first().waitFor();
    assert.equal(await page.locator(".task-table tbody tr").first().evaluate(e=>e.getBoundingClientRect().height),64);
    console.log("PASS rows: three pages, ordinary/parent/child, text/badge/button bounds, resize/restart");
  }
  if (mode === "fonts") {
    assert.ok(nativeFonts.length>20);assert.ok(nativeFonts[0].cjk);
    const openPicker=async()=>{await page.locator(".settings-button").filter({hasText:"软件设置"}).click();await page.locator(".font-setting-trigger").click();await page.locator(".font-picker-list [role=option]").nth(1).waitFor();};
    await openPicker();
    assert.equal(await page.locator(".font-picker-list [role=option]").count(),nativeFonts.length+1);
    if(process.env.INLINE_UI_SCREENSHOT)await page.screenshot({path:process.env.INLINE_UI_SCREENSHOT});
    const chinese=nativeFonts.find(f=>f.cjk&&f.aliases.some(a=>/[\u4e00-\u9fff]/.test(a)))??nativeFonts.find(f=>f.cjk);
    const english=nativeFonts.find(f=>f.family==="Arial")??nativeFonts.find(f=>!f.cjk);
    const wider=nativeFonts.find(f=>/wide|black|impact/i.test(f.family))??english;
    const tall=nativeFonts.find(f=>/cambria|times|ming/i.test(f.family))??chinese;
    const search=page.getByRole("textbox",{name:"搜索字体名称"});
    for(const query of [chinese.family,chinese.family.slice(-4).toUpperCase(),chinese.displayName.slice(0,2)]){
      await search.fill(query);assert.ok(await page.locator(".font-picker-list [role=option]").count()>=2);
    }
    await search.fill("In-Line nonexistent 8933");await page.getByText("没有匹配的字体，请尝试其他名称。").waitFor();
    await search.fill("");assert.equal(await page.locator(".font-picker-list [role=option]").count(),nativeFonts.length+1);
    await page.getByRole("button",{name:"关闭字体选择"}).click();
    for(const font of [chinese,english,wider,tall]){
      await openPicker();await search.fill(font.family);
      await page.getByRole("option").filter({has:page.getByText(font.displayName,{exact:true})}).click();
      await page.waitForFunction(family=>document.documentElement.getAttribute("data-ui-font")===family,font.family);
      const buttonFamily=await page.locator(".new-ticket").evaluate(e=>getComputedStyle(e).fontFamily);
      assert.ok(buttonFamily.includes(font.family));
      for(const label of ["待办队列","暂缓事项","历史归档"]){
        await page.locator(".sidebar nav button").filter({hasText:label}).click();
        await page.waitForFunction(()=>{const e=document.querySelector(".table-scroll");return e&&e.scrollWidth===e.clientWidth});
        const bounds=await page.locator(".task-table tbody tr").evaluateAll(rows=>rows.map(row=>{const rect=row.getBoundingClientRect();return{height:rect.height,clipped:[...row.querySelectorAll(".task-title-line>strong,.parent-task-line,.status-badge,.row-actions button")].some(e=>{const r=e.getBoundingClientRect();return r.top<rect.top||r.bottom>rect.bottom})}}));
        assert.ok(bounds.every(row=>row.height===64&&!row.clipped),JSON.stringify(bounds));
      }
      assert.ok(await page.locator(".sidebar svg path").count()>0);
      await page.locator(".task-table tbody tr").first().click();await page.locator(".detail-panel").waitFor();
      assert.ok((await page.locator(".detail-title-copy").evaluate(e=>getComputedStyle(e).fontFamily)).includes(font.family));
      await page.locator(".detail-panel>header").getByRole("button",{name:"关闭",exact:true}).click();
      await page.locator(".sidebar nav button").filter({hasText:"统计中心"}).click();
      await page.locator(".summary-grid").waitFor();
      assert.ok((await page.locator(".summary-grid strong").first().evaluate(e=>getComputedStyle(e).fontFamily)).includes(font.family));
      // Font ink can exceed the nominal line box while still fitting the control; test the real clipping boundary.
      await page.evaluate(()=>document.fonts.ready);
      const glyphs=await page.evaluate(()=>{const e=document.querySelector(".new-ticket"),s=getComputedStyle(e),c=document.createElement("canvas").getContext("2d");c.font=s.font;const m=c.measureText("事项 Agjy中");return{height:m.actualBoundingBoxAscent+m.actualBoundingBoxDescent,available:e.getBoundingClientRect().height-parseFloat(s.borderTopWidth)-parseFloat(s.borderBottomWidth),font:s.font}});
      assert.ok(glyphs.height<=glyphs.available,JSON.stringify(glyphs));
    }
    await page.reload();await page.locator(".task-table tbody tr").first().waitFor();
    await page.waitForFunction(family=>document.documentElement.getAttribute("data-ui-font")===family,tall.family);
    const secondary=await page.context().newPage();
    for(const hash of ["floating","quick-add","update-progress"]){
      await secondary.goto(server.resolvedUrls.local[0]+"#"+hash);
      await secondary.waitForFunction(family=>document.documentElement.getAttribute("data-ui-font")===family,tall.family);
    }
    await openPicker();assert.equal(await page.getByRole("option").filter({has:page.getByText(tall.displayName,{exact:true})}).getAttribute("aria-selected"),"true");
    await page.getByRole("button",{name:"关闭字体选择"}).click();
    await page.locator(".font-setting-row").getByRole("button",{name:"恢复默认",exact:true}).click();
    await page.waitForFunction(()=>document.documentElement.getAttribute("data-ui-font")==="default");
    await secondary.waitForFunction(()=>document.documentElement.getAttribute("data-ui-font")==="default");
    await secondary.close();
    await page.evaluate(()=>window.__inlineRestoreFont("In-Line nonexistent font 8933"));
    await page.locator(".font-fallback-note").filter({hasText:"不可用"}).waitFor();
    assert.equal(await page.locator(".font-setting-trigger").innerText(),"系统默认字体");
    assert.equal(await page.evaluate(()=>document.documentElement.getAttribute("data-ui-font")),"default");
    await page.locator(".font-setting-row").getByRole("button",{name:"恢复默认",exact:true}).click();
    await page.waitForFunction(()=>localStorage.getItem("isolated-ui-font")==="");
    assert.equal((await page.evaluate(()=>window.__inlineCalls)).filter(c=>c[0]==="fonts").length,1);
    await page.reload();await page.locator(".task-table tbody tr").first().waitFor();
    await page.locator(".settings-button").filter({hasText:"软件设置"}).click();
    assert.equal(await page.locator(".font-setting-trigger").innerText(),"系统默认字体");
    assert.equal(await page.locator(".font-setting-row").getByRole("button",{name:"恢复默认",exact:true}).isDisabled(),true);
    console.log("PASS fonts:",nativeFonts.length,"native families; aliases/search/local cache/live global/default/missing/restart/auxiliary routes; 4 font row and overflow regression");
  }
  if (mode === "trend") {
    await page.locator(".sidebar nav button").filter({ hasText: "统计中心" }).click();
    const first = page.locator(".trend-column").first();
    await first.locator(".trend-bar").click();
    await page.locator(".details-card tbody tr").last().waitFor();
    assert.equal(await page.locator(".details-card tbody tr").count(), 3);
    assert.equal(await page.locator(".trend-result-controls").count(),0);
    assert.match(await page.locator(".details-card").innerText(),/待补材料/);
    await page.getByRole("button",{name:"复制明细",exact:true}).click();
    const copied=await page.evaluate(()=>window.__inlineClipboard);
    assert.equal(copied,"合同审查-测试部门-隔离测试事项 1 — 长标题与父子任务布局检查\n合同审查-测试部门-隔离测试事项 2 — 长标题与父子任务布局检查\n合同审查-测试部门-隔离测试事项 3 — 长标题与父子任务布局检查\n");
    await page.evaluate(()=>window.__inlineCopyFail=true);
    await page.getByRole("button",{name:"复制明细",exact:true}).click();
    await page.getByText("复制明细失败：隔离剪贴板失败",{exact:true}).waitFor();
    assert.equal(await page.evaluate(()=>window.__inlineClipboard),copied);
    await page.evaluate(()=>window.__inlineCopyFail=false);
    if(process.env.INLINE_UI_SCREENSHOT)await page.screenshot({path:process.env.INLINE_UI_SCREENSHOT});
    await page.getByRole("button", { name: "收起明细" }).click();
    assert.equal(await page.locator(".details-card").count(), 0);
    await page.locator(".trend-column").nth(1).locator(".trend-total").click();
    await page.getByText("该范围内没有对应事项。").waitFor();
    assert.equal(await page.getByRole("button",{name:"复制明细",exact:true}).isDisabled(),true);
    // Same API fixture only returns events on the first day.
    await page.locator(".type-stats button, .task-type-pie-legend button").first().click();
    await page.locator(".details-card tbody tr").last().waitFor();
    assert.equal(await page.locator(".details-card tbody tr").count(), 3);
    const calls = await page.evaluate(() => window.__inlineCalls);
    assert.equal(await page.getByRole("button",{name:"复制明细",exact:true}).count(),0);
    assert.ok(calls.filter(c=>c[0]==="trend").every(c=>c[3]===null));
    const firstCall = calls.find(c => c[0] === "trend");
    assert.equal(new Date(firstCall[2]) - new Date(firstCall[1]), 86400000);
    await page.locator(".details-card tbody tr").first().click();
    await page.locator(".detail-panel").waitFor();
    await page.locator(".detail-panel>header").getByRole("button",{name:"关闭",exact:true}).click();
    await page.locator(".sidebar nav button").filter({hasText:"统计中心"}).click();
    await page.getByRole("button",{name:"上一季度",exact:true}).click();
    await page.getByText("按自然周去重 · 点击数量查看明细",{exact:true}).waitFor();
    await page.locator(".trend-column .trend-total").first().click();
    await page.locator(".details-card tbody tr").last().waitFor();
    await page.getByRole("button",{name:"复制明细",exact:true}).click();
    assert.equal(await page.evaluate(()=>window.__inlineClipboard),copied);
    const weeklyCall=(await page.evaluate(()=>window.__inlineCalls)).filter(c=>c[0]==="trend").at(-1);
    assert.ok(new Date(weeklyCall[2])-new Date(weeklyCall[1])<=7*86400000);
    assert.match(await page.locator(".details-card h2").innerText(),/至.*全部办理事项/);
    console.log("PASS trend: total/bar/zero/type details, close/navigation, day/week clipped ranges, copy including later waiting/sort/newlines/failure/empty");
  }
  assert.deepEqual(errors, []);
} catch (error) { console.error("UI failure:", error.message, errors); throw error; }
finally { await browser.close(); await server.close(); }
