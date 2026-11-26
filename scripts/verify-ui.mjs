// Isolated browser regression: never connects to Tauri or the user's database.
// Use a local playwright installation or INLINE_PLAYWRIGHT_MODULE and INLINE_BROWSER_PATH.
import assert from "node:assert/strict";
import { pathToFileURL } from "node:url";
import { createServer } from "vite";
import {execFile} from "node:child_process";
import {promisify} from "node:util";
import {writeFile} from "node:fs/promises";

const { chromium } = await import(process.env.INLINE_PLAYWRIGHT_MODULE
  ? pathToFileURL(process.env.INLINE_PLAYWRIGHT_MODULE).href : "playwright");
const mode = process.argv[2] ?? "trend";
const nativeFonts=mode==="fonts"?JSON.parse((await promisify(execFile)("cargo",["run","--locked","--manifest-path","src-tauri/Cargo.toml","--example","list_system_fonts"],{maxBuffer:4*1024*1024})).stdout):[];
const periods = mode === "periods";
const mock = `
${periods ? 'import {api as realApi} from "/src/api.ts?f1-wire";' : ""}
const task=(id,status="pending",parentTaskId=null)=>({id,permanentNumber:"IL-"+id,dailySequence:id,ticketDate:"2026-09-30",department:"测试部门",departments:["测试部门"],contact:"测试人员",contacts:["测试人员"],taskType:"合同审查",title:"隔离测试事项 "+id+" — 长标题与父子任务布局检查",details:"隔离测试详情",status,priority:"normal",workload:"standard",isUrgent:false,urgentRequester:"",urgentReason:"",requestedDeadline:null,requestedDeadlineLabel:null,internalNotes:"",createdAt:"2026-09-30T08:00:00+08:00",updatedAt:"2026-09-30T08:00:00+08:00",startedAt:null,completedAt:null,archivedAt:status==="completed"?"2026-09-30T08:00:00+08:00":null,deletedAt:null,customSortOrder:id,processingRounds:1,hasActiveQueue:status==="pending",deferredEnteredAt:status==="processed"?"2026-09-30T08:00:00+08:00":null,isImportConflict:false,parentTaskId,subtaskSortOrder:id});
const queue=Array.from({length:30},(_,i)=>task(i+1,i%3===2?"processed":"pending",i===1?1:null));
const archive=Array.from({length:10},(_,i)=>task(i+31,"completed",i===1?31:null));
const all=[...queue,...archive];
if (${JSON.stringify(mode)} === "history") {
  Object.assign(archive[0], {status:"completed",archivedAt:null,completedAt:"2026-09-30T08:00:00+08:00"});
  Object.assign(archive[1], {status:"cancelled",archivedAt:null});
  Object.assign(archive[2], {status:"archived",archivedAt:"2026-09-30T08:00:00+08:00"});
}
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
specific.getStatistics=async(...args)=>{window.__inlineCalls.push(["statistics",...args]);const day=localDay(args[0]);statisticsDays.add(day);const data=await originalStatistics(...args);data.trend[0].periodStart=day;if(new Date(args[1])-new Date(args[0])>62*86400000){const monday=new Date(args[0]);monday.setDate(monday.getDate()-(monday.getDay()+6)%7);data.trendGranularity="week";data.trend[0].periodStart=localDay(monday);}return data};
specific.getStatisticsTrendDetails=async(...args)=>{window.__inlineCalls.push(["trend",...args]);if(!statisticsDays.has(localDay(args[0])))return [];return args[2]?details.filter(t=>t.resultStatus===args[2]):details};
specific.copyText=async text=>{if(window.__inlineCopyFail)throw new Error("隔离剪贴板失败");window.__inlineCalls.push(["copyText",text]);window.__inlineClipboard=text};
if (${periods}) {
  queue[0].requestedDeadline="2025-03-01T00:00:00+08:00";
  window.__inlineRules={};
  window.__inlinePending={};
  window.__inlineSetWeek=value=>specific.setSetting("week_start_day",value);
  window.__inlineIpc=[];
  window.__inlineCanvas=[];
  const fillText=CanvasRenderingContext2D.prototype.fillText;
  CanvasRenderingContext2D.prototype.fillText=function(text,...args){
    window.__inlineCanvas.push(String(text));
    return fillText.call(this,text,...args);
  };
  const statistics=specific.getStatistics;
  const types=specific.getStatisticsDetails;
  const trend=specific.getStatisticsTrendDetails;
  const waitForRule=async(kind,start)=>{
    const key=kind+":"+localDay(start);
    const rule=window.__inlineRules[key];
    if(rule==="fail")throw new Error("合成查询失败");
    if(rule==="hold")await new Promise(resolve=>{(window.__inlinePending[key]??=[]).push(resolve)});
    return rule;
  };
  window.__TAURI_INTERNALS__={invoke:async (command,args)=>{
    window.__inlineIpc.push([command,command==="save_chart_export"?{path:args.path,size:args.bytes.length}:args]);
    if(command==="get_statistics"){
      const rule=await waitForRule("summary",args.start);
      const data=await statistics(args.start,args.end);
      if(rule==="empty"){
        data.summary.handledTasks=data.summary.topLevelTasks=data.summary.subtasks=0;
        data.summary.processed=data.summary.completed=data.summary.waitingMaterials=0;
        data.summary.rateNumerator=data.summary.rateDenominator=data.summary.completionRate=0;
        data.byDepartment=[];data.byTaskType=[];data.trend=[];
        return data;
      }
      const count=Number(localDay(args.start).slice(5,7))*10+Number(localDay(args.start).slice(8));
      data.summary.handledTasks=count;
      data.summary.topLevelTasks=count-1;
      data.summary.rateNumerator=count-1;
      data.summary.rateDenominator=count;
      data.summary.completionRate=(count-1)/count;
      data.byDepartment[0].handledTasks=count;
      data.byTaskType[0].handledTasks=count;
      data.trend[0].handledTasks=count;
      if (data.trendGranularity==="week") {
        const last=new Date(args.end);
        last.setDate(last.getDate()-1);
        const lastDay=localDay(last);
        last.setDate(last.getDate()-(last.getDay()+6)%7);
        if (lastDay!==localDay(args.start)) data.trend.push({periodStart:localDay(last),handledTasks:2});
      }
      return data;
    }
    if(command==="get_statistics_details"){
      await waitForRule("type",args.start);
      return types(args.start,args.end,args.taskType);
    }
    if(command==="get_statistics_trend_details"){
      await waitForRule("trend",args.start);
      return trend(args.start,args.end,args.resultStatus);
    }
    if(command==="save_chart_export"){window.__inlinePng=args.bytes;return;}
    throw new Error("Unexpected Tauri command "+command);
  }};
  for(const key of ["getStatistics","getStatisticsDetails","getStatisticsTrendDetails","saveChartExport"]) specific[key]=realApi[key];
}
if (${periods || mode === "trend"}) specific.getVersion=async()=>"0.5.0";
specific.getRecommendedFontStatus=async()=>({phase:"idle",downloadedBytes:0,totalBytes:25443648,percent:0,message:null});specific.onRecommendedFontProgress=()=>()=>{};specific.getUIScale=async()=>"100";
if (${JSON.stringify(mode)} === "history") {
  specific.getLogs=async id=>[{id:100+id,taskId:id,logType:"status",content:"保留的历史记录",createdAt:"2026-09-30T08:00:00+08:00"}];
  specific.reopenTask=async input=>{
    window.__inlineCalls.push(["reopenTask",input]);
    const reopened=all.find(t=>t.id===input.id);
    archive.splice(archive.indexOf(reopened),1);
    Object.assign(reopened,{status:"pending",archivedAt:null,completedAt:null,hasActiveQueue:true});
    queue.push(reopened);
  };
}
export const api=new Proxy(specific,{get:(target,key)=>target[key]??(async(...args)=>{window.__inlineCalls.push([key,...args]);return []})});
`;
const dialogStub="\0inline-statistics-dialog-stub";
const server = await createServer({ server: { host: "127.0.0.1", port: 0, strictPort: false, watch: { ignored: ["**/src-tauri/target/**"] } }, plugins: [{
  name: "isolated-ui-api", enforce: "pre",
  resolveId(source) { if(periods&&source==="@tauri-apps/plugin-dialog")return dialogStub; },
  load(id) {
    if (id.replaceAll("\\", "/").endsWith("/src/api.ts")) return mock;
    if (id===dialogStub) return `
      export const save=async options=>{
        window.__inlinePicker=options;
        if (window.__inlineHoldPicker) await new Promise(resolve=>window.__inlineResolvePicker=resolve);
        return "isolated-chart.png";
      };
      export const open=async()=>null;
      export const ask=async()=>false;
      export const confirm=async()=>false;`;
  }
}] });
await server.listen();
const browser = await chromium.launch({ headless: true, executablePath: process.env.INLINE_BROWSER_PATH });
const errors = [];
try {
  const mainContext = await browser.newContext({ viewport: { width: 1440, height: 900 }, timezoneId: "Asia/Shanghai" });
  if(periods||mode==="trend"){
    const localOrigin=new URL(server.resolvedUrls.local[0]).origin;
    await mainContext.route("**/*",route=>new URL(route.request().url()).origin===localOrigin?route.continue():route.abort());
  }
  const page = await mainContext.newPage();
  page.setDefaultTimeout(15000);
  page.on("pageerror", error => errors.push(error.message));
  if(periods||mode==="trend"){
    const date=periods?"2025-03-30":"2025-03-12";
    await page.clock.setFixedTime(new Date(date+"T04:00:00Z"));
  }
  await page.goto(server.resolvedUrls.local[0],{waitUntil:"domcontentloaded",timeout:45000});
  await page.locator(".task-table tbody tr").first().waitFor();
  if (mode === "history") {
    await page.locator(".sidebar nav button").filter({hasText:"历史归档"}).click();
    const rows=page.locator(".task-table tbody tr");
    assert.equal(await rows.count(),10);
    for (const [id,status] of [[31,"已完成"],[32,"已取消"],[33,"已归档"]]) {
      const row=rows.filter({has:page.locator(".task-title-line>strong").filter({hasText:`隔离测试事项 ${id} —`})});
      assert.equal(await row.locator(".status-badge").innerText(),status);
      await row.click({button:"right"});
      const menu=page.getByRole("menu");
      assert.equal(await menu.getByRole("button",{name:"归档",exact:true}).count(),0);
      assert.equal(await menu.getByRole("button",{name:"重新开启并加入今日队列",exact:true}).count(),id===32?0:1);
      await menu.getByRole("button",{name:"查看详情",exact:true}).click();
      const detail=page.locator(".detail-panel");
      await detail.getByText("保留的历史记录",{exact:true}).waitFor();
      assert.equal(await detail.getByRole("button",{name:"归档",exact:true}).count(),0);
      assert.equal(await detail.getByRole("button",{name:"重新开启并加入今日队列",exact:true}).count(),id===32?0:1);
      if(id===33) {
        await detail.getByRole("button",{name:"重新开启并加入今日队列",exact:true}).click();
        const dialog=page.getByRole("dialog");
        await dialog.getByPlaceholder("选填，将记录到事项时间线").fill("旧归档事项继续办理");
        assert.equal(await dialog.getByRole("checkbox").isChecked(),false);
        await dialog.getByRole("button",{name:"确认加入",exact:true}).click();
        await page.waitForFunction(()=>window.__inlineCalls.some(c=>c[0]==="reopenTask"));
        await page.waitForFunction(()=>document.querySelectorAll(".task-table tbody tr").length===9);
        await page.locator(".sidebar nav button").filter({hasText:"待办队列"}).click();
        await page.locator(".task-table tbody tr").filter({hasText:"隔离测试事项 33 —"}).click();
        await page.locator(".detail-panel").getByText("保留的历史记录",{exact:true}).waitFor();
        assert.deepEqual((await page.evaluate(()=>window.__inlineCalls)).find(c=>c[0]==="reopenTask"),["reopenTask",{id:33,inheritDeadline:false,reason:"旧归档事项继续办理"}]);
      }
      await detail.locator("header").first().getByRole("button",{name:"关闭",exact:true}).click();
    }
    assert.equal((await page.evaluate(()=>window.__inlineCalls)).filter(c=>/archiveTask/.test(c[0])).length,0);
    console.log("PASS history: completed/cancelled/legacy archived navigation, preserved logs, legacy reopen with default deadline and return to queue");
  }
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
  if (mode === "statistics-range") {
    const openStatistics=()=>page.locator(".sidebar nav button").filter({hasText:"统计中心"}).click();
    const tabs=page.locator(".preset-tabs");
    const periodText=()=>page.locator(".period-footer>small").innerText();
    const queries=()=>page.evaluate(()=>window.__inlineCalls.filter(call=>call[0]==="statistics").slice(-2));
    const openDetails=async(source,row)=>{
      await page.locator(source).first().click();
      await page.locator(".details-card tbody tr").last().waitFor();
      await page.locator(".details-card tbody tr").nth(row).click();
      await page.locator(".detail-panel").waitFor();
      assert.equal(await page.locator(".statistics-panel").count(),0);
      await page.locator(".detail-panel>header").getByRole("button",{name:"关闭",exact:true}).click();
    };
    await openStatistics();
    assert.equal(await tabs.locator(".active").innerText(),"本周");
    for(const [index,label] of ["上周","上月","上季度"].entries()) {
      await tabs.getByRole("button",{name:label,exact:true}).click();
      await page.locator(".type-stats button, .task-type-pie-legend button").first().waitFor();
      const expectedText=await periodText(),expectedQueries=await queries();
      await openDetails(".type-stats button, .task-type-pie-legend button",index);
      await openStatistics();
      await page.locator(".type-stats button, .task-type-pie-legend button").first().waitFor();
      assert.equal(await tabs.locator(".active").innerText(),label);
      assert.equal(await periodText(),expectedText);
      assert.deepEqual(await queries(),expectedQueries);
    }
    await tabs.getByRole("button",{name:"自定义",exact:true}).click();
    const dates=page.locator(".custom-range input");
    await dates.nth(0).fill("2026-08-01");
    await dates.nth(1).fill("2026-08-31");
    await page.locator(".trend-column .trend-total").first().waitFor();
    const expectedQueries=await queries();
    await openDetails(".trend-column .trend-total",0);
    await openStatistics();
    await page.locator(".trend-column .trend-total").first().waitFor();
    assert.equal(await tabs.locator(".active").innerText(),"自定义");
    assert.equal(await dates.nth(0).inputValue(),"2026-08-01");
    assert.equal(await dates.nth(1).inputValue(),"2026-08-31");
    assert.equal(await periodText(),"2026-08-01 至 2026-08-31");
    assert.deepEqual(await queries(),expectedQueries);
    await tabs.getByRole("button",{name:"上周",exact:true}).click();
    await tabs.getByRole("button",{name:"自定义",exact:true}).click();
    assert.equal(await dates.nth(0).inputValue(),"2026-08-01");
    assert.equal(await dates.nth(1).inputValue(),"2026-08-31");
    await page.locator(".sidebar nav button").filter({hasText:"待办队列"}).click();
    await openStatistics();
    assert.equal(await tabs.locator(".active").innerText(),"自定义");
    assert.equal(await periodText(),"2026-08-01 至 2026-08-31");
    console.log("PASS statistics-range: presets/custom dates and query bounds survive task details and page navigation");
  }
  if (periods) {
    const nav = label => page.locator(".sidebar nav button").filter({hasText:label}).click();
    const tabs = page.getByRole("group",{name:"统计时间范围"});
    const pick = label => tabs.getByRole("button",{name:label,exact:true}).click();
    const current = () => page.locator(".structure-summary-main strong").innerText();
    const ready = () => page.locator(".summary-grid").waitFor();
    const report = page.getByRole("button",{name:"生成报告",exact:true});
    const dateStart = page.getByLabel("开始日期",{exact:true});
    const dateEnd = page.getByLabel("结束日期",{exact:true});
    const iso = day => new Date(day+"T00:00:00+08:00").toISOString();
    const calls = () => page.evaluate(() => window.__inlineIpc);
    const lastStatistics = async () => (await calls()).filter(call=>call[0]==="get_statistics").slice(-2).map(call=>call[1]);
    const expectPair = async ([start,end,oldStart,oldEnd]) => {
      await ready();
      assert.deepEqual(await lastStatistics(), [
        {start:iso(start),end:iso(end),timezoneOffsetMinutes:480},
        {start:iso(oldStart),end:iso(oldEnd),timezoneOffsetMinutes:480}
      ]);
    };
    const cases = [
      ["本周","2025-03-24","2025-03-31","2025-03-17","2025-03-24","本周工作报告","本周周报"],
      ["上周","2025-03-17","2025-03-24","2025-03-10","2025-03-17","上一周工作报告","上周周报"],
      ["本月","2025-03-01","2025-03-31","2025-02-01","2025-03-01","本月工作报告","本月月报"],
      ["上月","2025-02-01","2025-03-01","2025-01-01","2025-02-01","月度工作报告","月报"],
      ["本季度","2025-01-01","2025-03-31","2024-10-01","2024-12-29","本季度工作报告","本季度工作报告"],
      ["上季度","2024-10-01","2025-01-01","2024-07-01","2024-10-01","季度工作报告","季度工作报告"],
      ["自定义","2025-03-02","2025-03-03","2025-03-01","2025-03-02","自定义周期工作报告","阶段工作报告"]
    ];
    await nav("统计中心");
    assert.deepEqual(await tabs.getByRole("button").allTextContents(),cases.map(row=>row[0]));
    assert.equal(await tabs.locator('[aria-pressed="true"]').innerText(),"本周");
    const inclusive = exclusive => new Date(Date.parse(exclusive+"T00:00:00Z")-86400000).toISOString().slice(0,10);
    for (const [label,start,end,oldStart,oldEnd,native,prompt] of cases) {
      await pick(label);
      if(label==="自定义"){
        await dateStart.fill("2025-03-02");
        await dateEnd.fill("2025-03-02");
      }
      await expectPair([start,end,oldStart,oldEnd]);
      const count=Number(start.slice(5,7))*10+Number(start.slice(8));
      const oldCount=Number(oldStart.slice(5,7))*10+Number(oldStart.slice(8));
      assert.equal(await current(),String(count));
      assert.deepEqual(await page.locator(".workload-comparison .comparison-row").first().locator(".comparison-bars b").allTextContents(),[String(count),String(oldCount)]);
      assert.equal(await page.locator(".type-stats button b").first().innerText(),String(count));
      assert.match(await page.locator(".department-comparison").innerText(),/测试部门/);
      await report.click();
      await page.getByRole("button",{name:"原生工作报告"}).click();
      await page.getByRole("button",{name:"预览报告"}).click();
      const preview=page.getByRole("dialog",{name:native});
      await preview.waitFor();
      assert.ok((await preview.innerText()).includes("统计期间为 "+start+" 至 "+inclusive(end)));
      assert.ok((await preview.innerText()).includes("共处理 "+count+" 项"));
      assert.ok((await preview.innerText()).includes("截至当前仍有 1 项逾期"));
      if(label==="本季度"){
        await preview.locator("select").selectOption("合同审查");
        await page.getByRole("dialog",{name:"本季度工作报告 · 合同审查"}).getByText(/本期共形成/).waitFor();
        assert.deepEqual((await calls()).filter(call=>call[0]==="get_statistics_details").at(-1),
          ["get_statistics_details",{start:iso(start),end:iso(end),taskType:"合同审查"}]);
        await page.getByRole("button",{name:"复制报告",exact:true}).click();
        assert.match(await page.evaluate(()=>window.__inlineClipboard),/本季度工作报告 · 合同审查/);
        if(process.env.INLINE_UI_SCREENSHOT)await page.screenshot({path:process.env.INLINE_UI_SCREENSHOT.replace(/\.png$/,"-report.png")});
        await page.getByRole("dialog",{name:"本季度工作报告 · 合同审查"}).getByRole("button",{name:"关闭",exact:true}).last().click();
      }else{
        await preview.getByRole("button",{name:"关闭",exact:true}).last().click();
      }
      await report.click();
      await page.getByRole("button",{name:"确认并生成",exact:true}).click();
      const copied=await page.evaluate(()=>window.__inlineClipboard);
      assert.ok(copied.includes(prompt));
      assert.ok(copied.includes(start));
      assert.ok(copied.includes(inclusive(end)));
      assert.doesNotMatch(copied,/\{\{(?:报告类型|开始日期|结束日期)\}\}/u);
    }
    const noResults = async () => {
      assert.equal(await page.locator(".summary-grid").count(),0);
      assert.equal(await report.isDisabled(),true);
      assert.equal(await page.getByRole("button",{name:"导出图表",exact:true}).count(),0);
    };
    const beforeInvalidTotal=(await calls()).filter(call=>call[0]==="get_statistics").length;
    await dateStart.fill("");
    await page.getByRole("alert").getByText("请输入有效的开始日期和结束日期").waitFor();
    await noResults();
    await dateStart.fill("2025-03-03");
    await page.getByRole("alert").getByText("开始日期不能晚于结束日期").waitFor();
    await noResults();
    assert.equal((await calls()).filter(call=>call[0]==="get_statistics").length,beforeInvalidTotal);
    await dateStart.fill("2025-03-02");
    await ready();
    await page.locator(".type-stats button").first().click();
    await page.locator(".details-card tbody tr").first().click();
    await page.locator(".detail-panel").waitFor();
    await page.locator(".detail-panel>header").getByRole("button",{name:"关闭",exact:true}).click();
    await nav("统计中心");
    await expectPair(cases.at(-1).slice(1,5));
    await nav("待办队列");
    await nav("统计中心");
    assert.equal(await dateStart.inputValue(),"2025-03-02");
    assert.equal(await dateEnd.inputValue(),"2025-03-02");
    await page.getByRole("button",{name:"软件设置",exact:true}).click();
    await page.getByRole("group",{name:"每周起始日"}).getByRole("button",{name:"周日",exact:true}).click();
    await nav("统计中心");
    await pick("本周");
    await expectPair(["2025-03-30","2025-03-31","2025-03-23","2025-03-24"]);
    await page.evaluate(() => window.__inlineSetWeek("monday"));
    await expectPair(cases[0].slice(1,5));
    await page.evaluate(() => window.__inlineSetWeek("sunday"));
    await expectPair(["2025-03-30","2025-03-31","2025-03-23","2025-03-24"]);
    await pick("上周");
    await expectPair(["2025-03-23","2025-03-30","2025-03-16","2025-03-23"]);
    await pick("自定义");
    await expectPair(cases.at(-1).slice(1,5));
    await pick("本周");
    await page.clock.setFixedTime(new Date("2025-03-30T16:01:00Z"));
    await page.evaluate(()=>window.dispatchEvent(new Event("focus")));
    await expectPair(["2025-03-30","2025-04-01","2025-03-23","2025-03-25"]);
    const future = page.locator(".trend-total:disabled");
    assert.ok(await future.count()>0);
    const drillCount=(await calls()).filter(call=>call[0]==="get_statistics_trend_details").length;
    await future.first().evaluate(button=>button.click());
    assert.equal((await calls()).filter(call=>call[0]==="get_statistics_trend_details").length,drillCount);
    await pick("自定义");
    await expectPair(cases.at(-1).slice(1,5));

    const rule = (key,value) => page.evaluate(([k,v])=>{window.__inlineRules[k]=v;},[key,value]);
    const waitHeld = key => page.waitForFunction(k=>Boolean(window.__inlinePending[k]?.length),key);
    const release = key => page.evaluate(async k=>{
      delete window.__inlineRules[k];
      window.__inlinePending[k]?.splice(0).forEach(resolve=>resolve());
      await new Promise(resolve=>setTimeout(resolve,0));
    },key);
    await rule("summary:2025-03-01","hold");
    await pick("本月");
    await waitHeld("summary:2025-03-01");
    await noResults();
    await rule("summary:2025-01-01","fail");
    await pick("本季度");
    await page.getByRole("alert").getByText("合成查询失败").waitFor();
    await release("summary:2025-03-01");
    await noResults();
    await rule("summary:2025-01-01",null);
    await page.getByRole("button",{name:"重新加载"}).click();
    await ready();
    assert.equal(await current(),"11");

    await rule("summary:2025-03-01","hold");
    await pick("本月");
    await waitHeld("summary:2025-03-01");
    await pick("上季度");
    await ready();
    await release("summary:2025-03-01");
    assert.equal(await current(),"101");
    await pick("本季度");
    await ready();

    await rule("type:2025-01-01","hold");
    await page.locator(".type-stats button").first().click();
    await waitHeld("type:2025-01-01");
    await page.locator(".trend-total").first().click();
    await page.locator(".details-card").getByText(/全部办理事项/).waitFor();
    await release("type:2025-01-01");
    assert.match(await page.locator(".details-card h2").innerText(),/全部办理事项/);
    await rule("trend:2025-01-01","hold");
    await page.locator(".trend-total").first().click();
    await page.locator(".trend-total").first().click();
    await waitHeld("trend:2025-01-01");
    await pick("上月");
    await ready();
    await release("trend:2025-01-01");
    assert.equal(await page.locator(".details-card").count(),0);

    await rule("summary:2025-03-01","hold");
    await pick("本月");
    await waitHeld("summary:2025-03-01");
    await nav("待办队列");
    await release("summary:2025-03-01");
    assert.equal(await page.locator(".statistics-panel").count(),0);
    await nav("统计中心");
    await ready();
    assert.equal(await tabs.locator('[aria-pressed="true"]').innerText(),"本月");

    await page.evaluate(()=>{window.__inlineHoldPicker=true;});
    await page.getByRole("button",{name:"导出图表",exact:true}).click();
    await page.getByRole("button",{name:"导出 300 DPI PNG",exact:true}).click();
    await page.waitForFunction(()=>typeof window.__inlineResolvePicker==="function");
    await tabs.getByRole("button",{name:"本季度",exact:true}).evaluate(button=>button.click());
    await ready();
    await page.evaluate(()=>{window.__inlineHoldPicker=false;window.__inlineResolvePicker();});
    await page.waitForFunction(()=>!document.querySelector(".chart-export-button")?.disabled);
    assert.ok(!(await calls()).some(call=>call[0]==="save_chart_export"));
    await page.getByRole("button",{name:"导出图表",exact:true}).click();
    await page.getByRole("button",{name:"导出 300 DPI PNG",exact:true}).click();
    await page.waitForFunction(()=>Boolean(window.__inlinePng));
    const png=await page.evaluate(()=>({picker:window.__inlinePicker,bytes:window.__inlinePng,texts:window.__inlineCanvas}));
    assert.match(png.picker.defaultPath,/2025-01-01_2025-03-31\.png$/);
    assert.ok(png.texts.includes("对比范围：2024-10-01 至 2024-12-29"));
    assert.ok(png.texts.some(text=>text.includes("上季度同期")));
    assert.ok(png.texts.includes("11")&&png.texts.includes("101"));
    const image=Buffer.from(png.bytes);
    assert.equal(image.subarray(1,4).toString(),"PNG");
    const phys=image.indexOf("pHYs");
    assert.ok(phys>0);
    assert.equal(image.readUInt32BE(phys+4),11811);
    assert.equal(image.readUInt32BE(phys+8),11811);
    assert.equal(image[phys+12],1);
    if(process.env.INLINE_UI_SCREENSHOT)await page.screenshot({path:process.env.INLINE_UI_SCREENSHOT});
    if(process.env.INLINE_UI_PNG)await writeFile(process.env.INLINE_UI_PNG,image);
    await page.locator(".trend-total").last().click();
    assert.deepEqual((await calls()).filter(call=>call[0]==="get_statistics_trend_details").at(-1),
      ["get_statistics_trend_details",{start:iso("2025-03-31"),end:iso("2025-04-01"),resultStatus:null}]);

    await page.setViewportSize({width:550,height:850});
    const layout=await tabs.getByRole("button").evaluateAll(buttons=>buttons.map(button=>{
      const bounds=button.getBoundingClientRect();
      const panel=button.closest(".statistics-panel").getBoundingClientRect();
      return {top:bounds.top,inside:bounds.left>=panel.left&&bounds.right<=panel.right};
    }));
    assert.ok(layout.every(button=>button.inside),JSON.stringify(layout));
    assert.ok(new Set(layout.map(button=>button.top)).size>1);
    assert.equal(await page.locator(".summary-grid").evaluate(grid=>getComputedStyle(grid).gridTemplateColumns.split(" ").length),2);
    if(process.env.INLINE_UI_SCREENSHOT)await page.screenshot({path:process.env.INLINE_UI_SCREENSHOT.replace(/\.png$/,"-narrow.png")});
    await pick("自定义");
    await rule("summary:2025-05-01","empty");
    await rule("summary:2025-02-27","empty");
    await dateEnd.fill("2025-07-02");
    await dateStart.fill("2025-05-01");
    await expectPair(["2025-05-01","2025-07-03","2025-02-27","2025-05-01"]);
    assert.equal(await current(),"0");
    assert.equal(await page.getByText("暂无趋势数据。",{exact:true}).count(),1);
    assert.equal(await page.getByText("当前及对比周期均无部门相关工作量。",{exact:true}).count(),1);
    console.log("PASS periods: seven preset IPC pairs, state/detail/navigation, validation, focus rollover, week setting, stale/failure/unmount isolation, all report modes, PNG pHYs and snapshot, narrow wrap");
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
    await page.getByRole("button",{name:"上季度",exact:true}).click();
    await page.getByText("按自然周去重 · 点击数量查看明细",{exact:true}).waitFor();
    const weeklyColumn=page.locator(".trend-column").first();
    const weeklyTitle=await weeklyColumn.getAttribute("title");
    assert.match(weeklyTitle,/^\d{4}-\d{2}-\d{2} 至 \d{4}-\d{2}-\d{2}：3 项$/);
    assert.doesNotMatch(await weeklyColumn.innerText(),/周[一二三四五六日]/);
    const [labelStart,labelEnd]=weeklyTitle.split("：")[0].split(" 至 ");
    assert.equal(await weeklyColumn.locator("small>span").innerText(),labelStart.slice(5));
    assert.equal(await weeklyColumn.locator("small>time").innerText(),`至 ${labelEnd.slice(5)}`);
    assert.equal(await weeklyColumn.locator("small>time").getAttribute("dateTime"),labelEnd);
    const exportedLabels=await page.evaluate(async()=>{
      const {api}=await import("/src/api.ts");
      const {renderStatisticsChartsPng}=await import("/src/lib/statistics-export.ts");
      const query=window.__inlineCalls.filter(call=>call[0]==="statistics").at(-2);
      const data=await api.getStatistics(query[1],query[2]);
      const localDate=value=>{const d=new Date(value);return `${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,"0")}-${String(d.getDate()).padStart(2,"0")}`};
      const range={start:localDate(query[1]),end:localDate(new Date(new Date(query[2]).getTime()-86400000))};
      const original=CanvasRenderingContext2D.prototype.fillText,texts=[];
      CanvasRenderingContext2D.prototype.fillText=function(text,...args){texts.push(text);return original.call(this,text,...args)};
      try{const png=await renderStatisticsChartsPng(["trend"],"隔离趋势回归",{data,comparisonData:data,range,comparisonRange:range,comparisonLabel:"测试",taskTypeMode:"list"});return {texts,bytes:png.length};}
      finally{CanvasRenderingContext2D.prototype.fillText=original;}
    });
    assert.ok(exportedLabels.bytes>1000);
    assert.ok(exportedLabels.texts.includes(labelStart.slice(5)));
    assert.ok(exportedLabels.texts.includes(`至 ${labelEnd.slice(5)}`));
    assert.ok(exportedLabels.texts.every(text=>!/^周[一二三四五六日]$/.test(text)));
    await page.locator(".trend-column .trend-total").first().click();
    await page.locator(".details-card tbody tr").last().waitFor();
    await page.getByRole("button",{name:"复制明细",exact:true}).click();
    assert.equal(await page.evaluate(()=>window.__inlineClipboard),copied);
    const weeklyCall=(await page.evaluate(()=>window.__inlineCalls)).filter(c=>c[0]==="trend").at(-1);
    assert.ok(new Date(weeklyCall[2])-new Date(weeklyCall[1])<=7*86400000);
    const asLocalDate=value=>{const d=new Date(value);return `${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,"0")}-${String(d.getDate()).padStart(2,"0")}`};
    assert.equal(asLocalDate(weeklyCall[1]),labelStart);
    assert.equal(asLocalDate(new Date(new Date(weeklyCall[2]).getTime()-86400000)),labelEnd);
    assert.match(await page.locator(".details-card h2").innerText(),/至.*全部办理事项/);
    console.log("PASS trend: total/bar/zero/type details, close/navigation, day/week clipped ranges, copy including later waiting/sort/newlines/failure/empty");
  }
  assert.deepEqual(errors, []);
} catch (error) { console.error("UI failure:", error.message, errors); throw error; }
finally { await browser.close(); await server.close(); }
