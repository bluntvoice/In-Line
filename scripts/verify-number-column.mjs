// Isolated browser regression for number clipping. No native database access.
import assert from "node:assert/strict";
import {pathToFileURL} from "node:url";
import {readFile,mkdir,writeFile} from "node:fs/promises";
import {createServer} from "vite";
const {chromium}=await import(process.env.INLINE_PLAYWRIGHT_MODULE?pathToFileURL(process.env.INLINE_PLAYWRIGHT_MODULE).href:"playwright");
const baseline=process.argv.includes("--baseline");
const mock=`
const callbacks=new Set(),settings={};const day=value=>value.getFullYear()+"-"+String(value.getMonth()+1).padStart(2,"0")+"-"+String(value.getDate()).padStart(2,"0");const earlier=new Date();earlier.setDate(earlier.getDate()-27);
const task=(id,sequence,status="pending",urgent=false)=>({id,title:"号码完整显示 "+id,status,plannedDate:day(earlier),isScheduled:false,permanentNumber:"TEST-"+id,dailySequence:sequence,ticketDate:day(earlier),department:"测试部门",departments:["测试部门"],contact:"测试人员",contacts:["测试人员"],taskType:"合同审核",details:"隔离布局测试",priority:urgent?"critical":"normal",workload:"standard",isUrgent:urgent,urgentRequester:"测试",urgentReason:"测试",requestedDeadline:null,requestedDeadlineLabel:null,internalNotes:"",createdAt:new Date().toISOString(),updatedAt:new Date().toISOString(),startedAt:null,completedAt:status==="completed"?new Date().toISOString():null,archivedAt:status==="completed"?new Date().toISOString():null,deletedAt:status==="deleted"?new Date().toISOString():null,customSortOrder:id,processingRounds:0,hasActiveQueue:status==="pending",deferredEnteredAt:status==="processed"?new Date().toISOString():null,isImportConflict:false,parentTaskId:null,subtaskSortOrder:0});
const queue=[task(1,2),task(2,9999,"pending",true),task(3,123456),task(4,123456,"processed",true)],archive=[task(5,123456,"completed",true)],trash=[task(6,123456,"deleted",true)],all=[...queue,...archive,...trash];
const api={bootstrap:async()=>({queue,archive,trash,masters:{departments:["测试部门"],contacts:["测试人员"],taskTypes:["合同审核"]},settings,backups:[]}),getVersion:async()=>"0.5.0",globalShortcutAvailable:async()=>true,getTicketColors:async()=>null,getUIFontSelection:async()=>({requested:settings.ui_font_family??"",effective:settings.ui_font_family??"",missing:false}),getUIScale:async()=>"100",onDataChanged:cb=>{callbacks.add(cb);return()=>callbacks.delete(cb)},onNewTask:()=>()=>{},onTaskUiAction:()=>()=>{},getTask:async id=>all.find(value=>value.id===id),getLogs:async()=>[],getWorkEvents:async()=>[],listSubtasks:async()=>[],listParentTaskCandidates:async()=>[],setSetting:async(key,value)=>{settings[key]=value;callbacks.forEach(cb=>cb());}};
window.__setTestFont=family=>api.setSetting("ui_font_family",family);export {api};`;
const server=await createServer({server:{port:0},plugins:[{name:"isolated-number-column",enforce:"pre",async load(id){const path=id.replaceAll("\\","/");if(path.endsWith("/src/api.ts"))return mock;if(baseline&&path.endsWith("/src/App.tsx"))return(await readFile(id,"utf8")).replace("tableViewportWidth-1,numberColumnMinimum","tableViewportWidth-1");}}]});
await server.listen();
const browser=await chromium.launch({headless:true,executablePath:process.env.INLINE_BROWSER_PATH});
const errors=[],records=[];
try{
  const context=await browser.newContext({viewport:{width:980,height:900},timezoneId:"Asia/Shanghai"});
  const page=await context.newPage();page.on("pageerror",error=>errors.push(error.message));
  await page.goto(server.resolvedUrls.local[0]);await page.locator(".task-table tbody tr").first().waitFor();
  const measure=()=>page.locator(".task-table").evaluate(table=>[...table.querySelectorAll('tbody td:first-child')].map(cell=>{const badge=cell.querySelector('.ticket-number'),text=badge.querySelector('strong'),s=getComputedStyle(cell),a=cell.getBoundingClientRect(),b=badge.getBoundingClientRect(),t=text.getBoundingClientRect();return{ticket:badge.textContent,column:a.width,badge:b.width,left:b.left-a.left,right:a.right-b.right,paddingRight:parseFloat(s.paddingRight),clipped:b.right>a.right-parseFloat(s.paddingRight)+.5||t.right>b.right-.5};}));
  await page.waitForTimeout(80);
  if(baseline){const values=await measure();assert.ok(values.some(value=>value.clipped));console.log("REPRODUCED old fitting:",JSON.stringify(values));await writeFile("release/number-column-baseline.json",JSON.stringify(values,null,2));}
  else{
    const check=async(label)=>{console.log("CHECK",label);await page.waitForFunction(()=>[...document.querySelectorAll('tbody td:first-child')].every(cell=>{const badge=cell.querySelector('.ticket-number');return badge.getBoundingClientRect().right<=cell.getBoundingClientRect().right-parseFloat(getComputedStyle(cell).paddingRight)+.5;}));const values=await measure();assert.ok(values.length);assert.ok(values.every(value=>!value.clipped),label+JSON.stringify(values));records.push({label,values});};
    for(const width of [980,1100,1280,1920]){await page.setViewportSize({width,height:900});await check("width "+width);}
    const cdp=await context.newCDPSession(page);
    for(const scale of [1.1,1.25,1.4,1.5]){await cdp.send("Emulation.setDeviceMetricsOverride",{width:Math.round(980/scale),height:Math.round(900/scale),deviceScaleFactor:scale,mobile:false});await check("scale "+scale);}
    for(const label of ["暂缓事项","历史归档","回收站","待办队列"]){await page.locator(".sidebar nav button").filter({hasText:label}).click();await check(label);}
    const dimensions=await page.locator('.table-scroll').evaluate(node=>({viewport:node.clientWidth,scroll:node.scrollWidth}));assert.ok(dimensions.scroll>dimensions.viewport);
    await page.locator('.table-scroll').evaluate(node=>node.scrollLeft=node.scrollWidth);await page.locator('.table-scroll').evaluate(node=>node.scrollLeft=0);await check("horizontal scrolling");
    await cdp.send("Emulation.clearDeviceMetricsOverride");await page.setViewportSize({width:1440,height:900});await page.locator('.task-table tbody tr').first().click();await page.locator('.detail-panel').waitFor();await check("detail open");await page.locator('.detail-panel>header').getByRole('button',{name:'关闭',exact:true}).click();
    await page.evaluate(()=>localStorage.setItem('in-line-task-column-layouts',JSON.stringify({version:1,pages:{queue:{number:70,title:420}}})));await page.reload();await page.locator('.task-table tbody tr').first().waitFor();await check("legacy narrow custom number column");
    const saved=await page.evaluate(()=>JSON.parse(localStorage.getItem('in-line-task-column-layouts')).pages.queue);assert.equal(saved.number,70);assert.equal(saved.title,420);
    for(const family of ["Impact","Times New Roman","Arial"]){await page.evaluate(value=>window.__setTestFont(value),family);await page.waitForFunction(value=>document.documentElement.dataset.uiFont===value,family);await check("font "+family);}
    const handle=page.getByRole('separator',{name:'调整号码列宽',exact:true});for(let index=0;index<8;index++)await handle.press('ArrowLeft');await check("manual minimum");assert.equal(await page.evaluate(()=>JSON.parse(localStorage.getItem('in-line-task-column-layouts')).pages.queue.title),420);
    await page.reload();await page.locator('.task-table tbody tr').first().waitFor();await check("reload");
    await page.locator('.task-table th').first().click({button:'right'});await page.getByRole('menuitem',{name:'恢复本页默认列宽'}).click();await check("restore default");
    await page.setViewportSize({width:980,height:900});await mkdir('release/ui',{recursive:true});await page.screenshot({path:'release/ui/number-column-small-window.png'});await writeFile('release/number-column-results.json',JSON.stringify(records,null,2));
    console.log("PASS number column: full badge/text, multi-letter prefix, large sequences, warning marker, default/custom widths, four pages, small/maximized windows, horizontal scrolling, detail, zoom 110%-150%, font changes, manual minimum, reload/reset. Native APIs and zoom are simulated.");
  }
  assert.deepEqual(errors,[]);
}finally{await browser.close();await server.close();}
