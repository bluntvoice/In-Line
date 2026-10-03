// Isolated UI regression, no user database or Windows font installation.
// Fixture fonts must match docs/fonts/source.json in release/font-cache-fixture.
import assert from "node:assert/strict";
import {readFile,mkdir,writeFile} from "node:fs/promises";
import {createHash} from "node:crypto";
import {pathToFileURL} from "node:url";
import {createServer} from "vite";
const {chromium}=await import(process.env.INLINE_PLAYWRIGHT_MODULE?pathToFileURL(process.env.INLINE_PLAYWRIGHT_MODULE).href:"playwright");
const manifest=JSON.parse(await readFile("docs/fonts/source.json","utf8"));
const fontBytes=new Map();
for(const asset of manifest.files){const bytes=await readFile(`release/font-cache-fixture/${asset.file}`);assert.equal(bytes.length,asset.bytes);assert.equal(createHash("sha256").update(bytes).digest("hex"),asset.sha256);fontBytes.set(asset.file,bytes);}
const mock=`
const settings={ui_font_family:localStorage.getItem("recommended-font-selection")??"",ui_scale:localStorage.getItem("recommended-ui-scale")??"100"};
const callbacks=new Set(),fontCallbacks=new Set(),channel=new BroadcastChannel("recommended-font-test");
const emit=()=>callbacks.forEach(cb=>cb());
let progress={phase:localStorage.getItem("recommended-font-cached")?"ready":"idle",downloadedBytes:0,totalBytes:25443648,percent:0,message:null},previous="";
window.__TAURI_INTERNALS__={...window.__TAURI_INTERNALS__,convertFileSrc:(file,protocol)=>"/__font/"+file};
window.__downloadCalls=0;
window.__finishFont=(success=true)=>{progress={...progress,phase:success?"ready":"error",downloadedBytes:success?25443648:8343180,percent:success?100:32,message:success?null:"隔离下载中断"};if(success){localStorage.setItem("recommended-font-cached","yes");if(settings.ui_font_family===previous){settings.ui_font_family="in-line:sarasa-ui-sc";localStorage.setItem("recommended-font-selection",settings.ui_font_family);channel.postMessage(settings);emit();}}fontCallbacks.forEach(cb=>cb(progress));};
window.__restoreFont=value=>{settings.ui_font_family=value;localStorage.setItem("recommended-font-selection",value);emit();};
window.__forgetCache=()=>{localStorage.removeItem("recommended-font-cached");progress={...progress,phase:"idle"};fontCallbacks.forEach(cb=>cb(progress));emit();};
channel.onmessage=event=>{Object.assign(settings,event.data);emit();};
const fonts=[{family:"Sarasa UI SC",displayName:"更纱黑体 UI SC",aliases:["Sarasa UI SC","更纱黑体 UI SC"],cjk:true},{family:"Arial",displayName:"Arial",aliases:["Arial"],cjk:false}];
const task=(id,status="pending")=>({id,permanentNumber:"IL-"+id,dailySequence:id,ticketDate:"2026-10-03",plannedDate:"2026-10-03",isScheduled:false,department:"测试部门",departments:["测试部门"],contact:"测试人员",contacts:["测试人员"],taskType:"合同审查",title:"隔离字体测试事项 "+id+" Agjy 中文长标题",details:"隔离测试详情",status,priority:"normal",workload:"standard",isUrgent:false,urgentRequester:"",urgentReason:"",requestedDeadline:null,requestedDeadlineLabel:null,internalNotes:"",createdAt:"2026-10-03T08:00:00+08:00",updatedAt:"2026-10-03T08:00:00+08:00",startedAt:null,completedAt:null,archivedAt:null,deletedAt:null,customSortOrder:id,processingRounds:0,hasActiveQueue:status==="pending",deferredEnteredAt:status==="processed"?"2026-10-03T08:00:00+08:00":null,isImportConflict:false,parentTaskId:null,subtaskSortOrder:id});
const queue=Array.from({length:20},(_,i)=>task(i+1,i%3===2?"processed":"pending"));
const api={bootstrap:async()=>({queue,archive:[],trash:[],masters:{departments:["测试部门"],contacts:["测试人员"],taskTypes:["合同审查"]},settings:{...settings},backups:[]}),getVersion:async()=>"0.5.0",globalShortcutAvailable:async()=>true,launchAtLogin:async()=>false,listBackups:async()=>[],getTask:async id=>queue.find(t=>t.id===id),getLogs:async()=>[],getWorkEvents:async()=>[],getTicketColors:async()=>null,listSystemFonts:async()=>fonts,getUIFontSelection:async()=>{const requested=settings.ui_font_family;const effective=requested==="in-line:sarasa-ui-sc"?(localStorage.getItem("recommended-font-cached")?requested:""):fonts.some(f=>f.family===requested)?requested:"";return{requested,effective,missing:Boolean(requested&&!effective)};},getUIScale:async()=>{await window.__simulateNativeZoom(Number(settings.ui_scale),window.__physicalWidth??1280);return settings.ui_scale;},getRecommendedFontStatus:async()=>progress,onRecommendedFontProgress:cb=>{fontCallbacks.add(cb);return()=>fontCallbacks.delete(cb);},downloadRecommendedFont:async()=>{window.__downloadCalls++;previous=settings.ui_font_family;progress={...progress,phase:"downloading",downloadedBytes:123456,percent:0,message:null};fontCallbacks.forEach(cb=>cb(progress));},setSetting:async(key,value)=>{settings[key]=value;if(key==="ui_font_family")localStorage.setItem("recommended-font-selection",value);if(key==="ui_scale")localStorage.setItem("recommended-ui-scale",value);channel.postMessage(settings);emit();},onDataChanged:cb=>{callbacks.add(cb);return()=>callbacks.delete(cb)},onNewTask:()=>()=>{},onTaskUiAction:()=>()=>{},getUpdateProgress:async()=>({phase:"idle",version:null,downloadedBytes:0,totalBytes:null,percent:null,message:null}),onUpdateProgress:()=>()=>{}};
export {api};
`;
let requests=0;
const server=await createServer({server:{port:0},plugins:[{name:"isolated-recommended-font",enforce:"pre",load(id){if(id.replaceAll("\\","/").endsWith("/src/api.ts"))return mock;},configureServer(server){server.middlewares.use((request,response,next)=>{if(!request.url?.startsWith("/__font/"))return next();requests++;const bytes=fontBytes.get(request.url.slice(8));if(!bytes){response.statusCode=404;return response.end();}response.setHeader("Content-Type","font/woff2");response.end(bytes);});}}]});
await server.listen();
const browser=await chromium.launch({headless:true,executablePath:process.env.INLINE_BROWSER_PATH});
const errors=[];
try{
  const context=await browser.newContext({viewport:{width:1280,height:900},timezoneId:"Asia/Shanghai"});
  const sessions=new WeakMap();
  // Emulate WebView zoom with the same CSS viewport and device pixel ratio.
  await context.exposeBinding("__simulateNativeZoom",async({page},scale,physicalWidth)=>{let cdp=sessions.get(page);if(!cdp){cdp=await context.newCDPSession(page);sessions.set(page,cdp);}const factor=scale/100,hash=new URL(page.url()).hash;const auxiliary=hash.includes("floating")||hash.includes("update-progress");const width=hash.includes("floating")?444:hash.includes("update-progress")?360:hash.includes("quick-add")?780:physicalWidth;const height=hash.includes("floating")?564:hash.includes("update-progress")?188:hash.includes("quick-add")?760:900;await cdp.send("Emulation.setDeviceMetricsOverride",{width:Math.round(width/(auxiliary?1:factor)),height:Math.round(height/(auxiliary?1:factor)),deviceScaleFactor:factor,mobile:false});});
  const page=await context.newPage();page.on("pageerror",error=>errors.push(error.message));
  const base=server.resolvedUrls.local[0];
  await page.goto(base,{waitUntil:"domcontentloaded"});await page.locator(".task-table tbody tr").first().waitFor();
  assert.equal(requests,0);assert.equal(await page.getAttribute("html","data-ui-font"),"default");
  await page.getByRole("button",{name:"软件设置",exact:true}).click();
  await page.locator(".font-setting-trigger").click();await page.getByRole("option").filter({has:page.getByText("Arial",{exact:true})}).click();await page.waitForFunction(()=>document.documentElement.dataset.uiFont==="Arial");
  await page.getByRole("button",{name:"下载并启用",exact:true}).click();
  await page.getByRole("button",{name:"下载处理中…",exact:true}).waitFor();assert.equal(await page.getByRole("button",{name:"下载处理中…",exact:true}).isDisabled(),true);
  await page.evaluate(()=>window.__finishFont(false));await page.getByRole("alert").filter({hasText:"隔离下载中断"}).waitFor();assert.equal(await page.getAttribute("html","data-ui-font"),"Arial");
  await page.getByRole("button",{name:"重新下载并启用",exact:true}).click();await page.evaluate(()=>window.__finishFont(true));await page.waitForFunction(()=>document.documentElement.dataset.uiFont==="In Line Sarasa UI SC");
  assert.equal(requests,3);assert.equal(await page.evaluate(()=>window.__downloadCalls),2);
  assert.deepEqual(await page.evaluate(()=>Array.from(document.fonts).filter(f=>f.family.replaceAll('"',"")==="In Line Sarasa UI SC").map(f=>f.weight).sort()),["400","600","700"]);
  await page.getByRole("button",{name:"待办队列",exact:false}).first().click();
  const cdp=sessions.get(page);await cdp.send("DOM.enable");await cdp.send("CSS.enable");const document=await cdp.send("DOM.getDocument");const node=await cdp.send("DOM.querySelector",{nodeId:document.root.nodeId,selector:".task-title-line strong"});const actual=await cdp.send("CSS.getPlatformFontsForNode",{nodeId:node.nodeId});assert.ok(actual.fonts.some(font=>font.isCustomFont&&font.familyName==="Sarasa UI SC"&&font.glyphCount>0),JSON.stringify(actual));
  await page.getByRole("button",{name:"软件设置",exact:true}).click();
  for(const scale of [100,110,120,130,140,150]){
    await page.getByLabel("界面大小",{exact:true}).selectOption(String(scale));await page.waitForFunction(expected=>Number(localStorage.getItem("recommended-ui-scale"))===expected,scale);
    await page.waitForFunction(expected=>Math.abs(window.innerWidth-1280/(expected/100))<2,scale);
    assert.ok(await page.locator(".settings-page").evaluate(element=>element.scrollWidth<=element.clientWidth+1),`settings overflow at ${scale}%`);
    const actions=page.locator(".ui-scale-actions");await actions.scrollIntoViewIfNeeded();assert.ok(await actions.getByRole("button",{name:"恢复默认",exact:true}).isVisible());
  }
  await mkdir("release/ui",{recursive:true});
  await writeFile("release/ui/recommended-font-scale-150.png",Buffer.from((await cdp.send("Page.captureScreenshot",{format:"png",captureBeyondViewport:false})).data,"base64"));
  await page.getByRole("button",{name:"软件设置",exact:true}).scrollIntoViewIfNeeded();
  assert.ok(await page.getByRole("button",{name:"软件设置",exact:true}).isVisible());
  await page.evaluate(()=>window.__physicalWidth=980);await page.getByLabel("界面大小",{exact:true}).selectOption("140");await page.getByLabel("界面大小",{exact:true}).selectOption("150");
  await page.waitForFunction(()=>Math.abs(window.innerWidth-980/1.5)<2);
  assert.ok(await page.locator(".settings-page").evaluate(e=>e.scrollWidth<=e.clientWidth+1),"settings overflow at minimum window / 150%");
  await page.getByRole("button",{name:"软件设置",exact:true}).scrollIntoViewIfNeeded();
  await page.evaluate(()=>window.__physicalWidth=1280);await page.getByLabel("界面大小",{exact:true}).selectOption("140");await page.getByLabel("界面大小",{exact:true}).selectOption("150");
  const secondary=await context.newPage();await secondary.goto(base+"#floating");await secondary.waitForFunction(()=>document.documentElement.dataset.uiFont==="In Line Sarasa UI SC");assert.equal(await secondary.evaluate(()=>window.devicePixelRatio),1.5);
  await page.reload();await page.getByRole("button",{name:"软件设置",exact:true}).click();await page.waitForFunction(()=>document.documentElement.dataset.uiFont==="In Line Sarasa UI SC");assert.equal(await page.getByLabel("界面大小",{exact:true}).inputValue(),"150");assert.equal(await page.evaluate(()=>window.__downloadCalls),0);
  await page.locator(".ui-scale-setting").getByRole("button",{name:"恢复默认",exact:true}).click();await page.waitForFunction(()=>window.devicePixelRatio===1);await secondary.waitForFunction(()=>window.devicePixelRatio===1);
  await page.locator(".font-setting-row").getByRole("button",{name:"恢复默认",exact:true}).click();await page.waitForFunction(()=>document.documentElement.dataset.uiFont==="default");await secondary.waitForFunction(()=>document.documentElement.dataset.uiFont==="default");
  await page.getByRole("button",{name:"使用推荐字体",exact:true}).click();await page.waitForFunction(()=>document.documentElement.dataset.uiFont==="In Line Sarasa UI SC");assert.equal(await page.evaluate(()=>window.__downloadCalls),0);
  await page.evaluate(()=>window.__forgetCache());await page.waitForFunction(()=>document.documentElement.dataset.uiFont==="default");await page.locator(".font-fallback-note").filter({hasText:"不可用"}).waitFor();assert.equal(await page.evaluate(()=>window.__downloadCalls),0);
  await page.screenshot({path:"release/ui/recommended-font-missing-cache.png"});
  assert.deepEqual(errors,[]);console.log("PASS: no startup download; failures preserve prior font; retry; all 3 actual custom-font weights despite installed Sarasa; offline reload/reuse; missing cache fallback; scale 100-150%, persistence/reset and cross-window sync; settings layout.");
}finally{await browser.close();await server.close();}
