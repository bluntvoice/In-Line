// Synthetic renderer only; installed font cache is read, never changed.
import assert from 'node:assert/strict';
import {readFile,mkdir} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {pathToFileURL} from 'node:url';
import {createServer} from 'vite';
const {chromium}=await import(pathToFileURL(process.env.INLINE_PLAYWRIGHT_MODULE).href);
const manifest=JSON.parse(await readFile('docs/fonts/source.json','utf8'));
const fonts=new Map();
for(const f of manifest.files){const bytes=await readFile(`${process.env.INLINE_FONT_FIXTURE}/${f.file}`);assert.equal(bytes.length,f.bytes);assert.equal(createHash('sha256').update(bytes).digest('hex'),f.sha256);fonts.set(f.file,bytes);}
const mock=`
const day=new Date().toISOString().slice(0,10);
const task=(id,status='pending')=>({id,permanentNumber:'FIXED-'+id,dailySequence:id,ticketDate:day,plannedDate:day,isScheduled:false,department:'测试',departments:['测试'],contact:'',contacts:[],taskType:'测试',title:'隔离事项'+id,details:'',status,priority:'normal',workload:'standard',isUrgent:false,urgentRequester:'',urgentReason:'',requestedDeadline:id===7?'2020-01-01T00:00:00+08:00':null,requestedDeadlineLabel:null,internalNotes:'',createdAt:day+'T08:00:00+08:00',updatedAt:day+'T08:00:00+08:00',startedAt:null,completedAt:null,archivedAt:status==='completed'?day:null,deletedAt:null,customSortOrder:id,processingRounds:0,hasActiveQueue:status==='pending',deferredEnteredAt:null,isImportConflict:false,parentTaskId:null,subtaskSortOrder:0});
const data={queue:[...Array.from({length:6},(_,i)=>task(i+1)),...Array.from({length:79},(_,i)=>task(i+7,'paused'))],archive:Array.from({length:182},(_,i)=>task(i+100,'completed')),trash:[],masters:{departments:['测试'],contacts:[],taskTypes:['测试']},settings:{},backups:[]};
const specific={bootstrap:async()=>data,getVersion:async()=>'0.5.0',getTicketColors:async()=>null,getUIFontSelection:async()=>({requested:'',effective:'',missing:false}),getUIScale:async()=>'100',onDataChanged:()=>()=>{},onNewTask:()=>()=>{},onTaskUiAction:()=>()=>{}};
export const api=new Proxy(specific,{get:(target,key)=>target[key]??(async()=>[])});`;
const server=await createServer({server:{host:'127.0.0.1',port:0,watch:{ignored:['**/src-tauri/**','**/release/**','**/docs/**']}},plugins:[{name:'capsule-fixture',enforce:'pre',load(id){if(id.replaceAll('\\','/').endsWith('/src/api.ts'))return mock;},configureServer(s){s.middlewares.use((req,res,next)=>{const name=req.url?.split('/__capsulefont/')[1];if(!fonts.has(name))return next();res.setHeader('Content-Type','font/woff2');res.end(fonts.get(name));});}}]});
await server.listen();
const browser=await chromium.launch({headless:true,executablePath:process.env.INLINE_BROWSER_PATH});
const errors=[];const measurements=[];
try{
 const context=await browser.newContext({viewport:{width:1280,height:900}});const page=await context.newPage();page.on('pageerror',e=>errors.push(e.message));
 await page.goto(server.resolvedUrls.local[0],{waitUntil:'domcontentloaded'});await page.locator('.sidebar nav button b').first().waitFor();
 await page.evaluate(async assets=>{for(const f of assets){const face=new FontFace('Capsule Sarasa',`url(/__capsulefont/${f.file})`,{weight:String(f.weight)});document.fonts.add(await face.load());}},manifest.files);
 const cdp=await context.newCDPSession(page);await cdp.send('DOM.enable');await cdp.send('CSS.enable');
 await mkdir('release/ui/capsules',{recursive:true});
 for(const font of ['Capsule Sarasa','Microsoft YaHei UI','Segoe UI','Arial']){
  for(const scale of [1,1.5]){
   await page.setViewportSize({width:Math.round(1280/scale),height:Math.round(900/scale)});
   await page.evaluate(async font=>{document.documentElement.style.setProperty('--ui-font-family',JSON.stringify(font)+',sans-serif');await document.fonts.ready;},font);
   const values=await page.locator('.sidebar nav button b,.sidebar nav .nav-counts em').evaluateAll(nodes=>nodes.map(el=>{const s=getComputedStyle(el),r=el.getBoundingClientRect(),range=document.createRange();range.selectNodeContents(el);const t=range.getBoundingClientRect();const svg=el.querySelector('svg')?.getBoundingClientRect();return{text:el.textContent,display:s.display,height:r.height,lineHeight:s.lineHeight,font:s.fontFamily,left:r.left,center:r.y+r.height/2,textCenter:t.y+t.height/2,centerX:r.x+r.width/2,textCenterX:t.x+t.width/2,svgCenter:svg?svg.y+svg.height/2:null};}));
   assert.deepEqual(values.map(v=>v.text),['6','79','1','182']);
   // Flex items blockify inline-flex to computed flex; both retain flex centering.
   values.forEach(v=>{assert.ok(['flex','inline-flex'].includes(v.display));assert.equal(v.height,22);assert.equal(v.lineHeight,'12px');assert.ok(Math.abs(v.center-v.textCenter)<=2,JSON.stringify(v));assert.ok(Math.abs(v.centerX-v.textCenterX)<0.2,JSON.stringify(v));if(v.svgCenter!==null)assert.ok(Math.abs(v.center-v.svgCenter)<0.1);});
   assert.ok(Math.abs(values[1].center-values[2].center)<0.1);
   if(font==='Capsule Sarasa'){const {root}=await cdp.send('DOM.getDocument');const {nodeId}=await cdp.send('DOM.querySelector',{nodeId:root.nodeId,selector:'.sidebar nav button b'});const actual=await cdp.send('CSS.getPlatformFontsForNode',{nodeId});assert.ok(actual.fonts.some(f=>f.isCustomFont&&f.glyphCount>0),JSON.stringify(actual));}
   measurements.push({font,scale,values});await page.locator('.sidebar nav').screenshot({path:`release/ui/capsules/${font.replaceAll(' ','-')}-${scale}.png`});
  }
 }
 assert.deepEqual(errors,[]);console.log(JSON.stringify({passed:true,actualSarasaFontVerified:true,measurements},null,2));
}finally{await browser.close();await server.close();}
