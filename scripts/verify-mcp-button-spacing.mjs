// Actual MCP components and SettingsPanel; all API responses are synthetic.
// WebView set_zoom reduces the effective CSS viewport. Emulate that layout and
// DPR without CSS zoom, then separately stress enlarged control text.
import assert from 'node:assert/strict';
import {mkdir,readFile,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
import {pathToFileURL} from 'node:url';
import {createServer} from 'vite';
const {chromium}=await import(process.env.INLINE_PLAYWRIGHT_MODULE?pathToFileURL(process.env.INLINE_PLAYWRIGHT_MODULE).href:'playwright');
const fontFiles=new Map();
for(const asset of JSON.parse(await readFile('docs/fonts/source.json','utf8')).files){
  const bytes=await readFile(`release/font-cache-fixture/${asset.file}`);
  assert.equal(bytes.length,asset.bytes);assert.equal(createHash('sha256').update(bytes).digest('hex'),asset.sha256);fontFiles.set(asset.file,bytes);
}
const mock=`
window.__TAURI_INTERNALS__={convertFileSrc:file=>'/__font/'+file};
window.__test={mode:'ready',calls:[],remount:null};const t=window.__test;
const permissions={regularRead:true,fullRead:false,write:false},scope={departments:['合成部门名称较长的布局测试甲','合成部门乙'],taskTypes:['合成合同审查']};
const security={paused:false,groups:{regularRead:true,fullRead:true,write:true},revision:1,sensitiveNoticeAcknowledged:true,clients:[{id:'synthetic-client',name:'合成客户端名称较长的布局测试 Codex MCP',kind:'codex',permissions,scope,revoked:false}],onboardingClients:[{id:'codex',label:'Codex（MCP）',available:false,authorizationAvailable:true,status:'自动接入验证中'}]};
const audit={id:1,action:'patch',taskId:1,reason:'合成原因',intent:'合成编辑',createdAt:'2026-10-26',before:{title:'旧'},after:{title:'新'}};
export const api={
 launchAtLogin:async()=>false,listBackups:async()=>[],onDataChanged:()=>()=>{},
 getUIFontSelection:async()=>({requested:'in-line:sarasa-ui-sc',effective:'in-line:sarasa-ui-sc',missing:false}),
 mcpStorageState:async()=>({ready:t.mode==='ready',canMigrate:true,migrationRequired:true,legacyFileIdentity:'synthetic-identity',legacyRoot:'D:/synthetic/legacy',root:'D:/synthetic/profile'}),
 mcpSecurityState:async()=>structuredClone(security),
 mcpPrepareOnboarding:async args=>{t.calls.push(['grant',args]);return {clientId:'synthetic-client',packageId:'synthetic-receipt',expiresAt:Math.floor(Date.now()/1000)+600,prompt:'合成接入提示词，无凭证'};},
 copyText:async text=>{t.calls.push(['copy',text]);},
 mcpConnectionGuide:async()=> '合成通用接入说明，无凭证',
 mcpAuthorizationDiagnostic:async()=>({diagnosticOnly:true,configurationVerified:false,currentSessionVerified:false}),
 mcpReuseOnboarding:async id=>{t.calls.push(['reuse',id]);return {packageId:'synthetic-package',clientId:id,expiresAt:Math.floor(Date.now()/1000)+600,prompt:'合成安全接入提示词（不含凭证）'};},
 mcpBatchState:async()=>({batchLimit:20,pending:[{id:1,request:{reason:'合成整批调整',operations:[{}]},preview:{items:[]}}]}),
 mcpAuditState:async()=>({audits:[audit],pending:[{id:1,auditId:1,reason:'合成撤销申请'}]}),
 mcpJobsState:async()=>({items:[{jobId:'synthetic-job',kind:'query_tasks',state:'running',progress:{processed:1,total:2},rounds:[{}]}]}),
 mcpCacheState:async()=>({cache:{entries:1,bytes:1024,ttlSeconds:60,sources:[],capacityBypasses:0},health:{healthy:false,repairable:true}}),
 mcpFaultState:async()=>({items:['open','resolved'].map((state,i)=>({id:'synthetic-fault-'+i,severity:'P2',code:'synthetic',tool:'get_capabilities',episodes:[{id:'synthetic-episode-'+i,number:1,state,evidenceCount:1,lastErrorId:'synthetic-error'}]})),latestClients:[]}),
 mcpMaintenancePreview:async(source,action,format)=>{t.calls.push(['preview',source,action]);return {nonce:'synthetic-preview',action,format,expiresAt:Math.floor(Date.now()/1000)+600,content:'合成预览',prompt:'合成手动转交提示词'};},
 mcpMaintenanceCancel:async()=>{},
};`;
const entry=`import React from 'react';import{createRoot}from'react-dom/client';import Font from'/src/components/UIFontProvider.tsx';import Settings from'/src/components/SettingsPanel.tsx';import'/src/styles.css';
function App(){const[key,setKey]=React.useState(0);window.__test.remount=()=>setKey(k=>k+1);return <Font><Settings key={key} backups={[]} settings={{}} isDatabaseEmpty={false} onChanged={()=>{}} onOpenTask={()=>{}} onNavigateBackupResult={()=>{}} notify={()=>{}}/></Font>;}createRoot(document.getElementById('root')).render(<App/>);`;
const unrelated=['FontSetting','UIScaleSetting','TicketColorSetting','FloatingWindowSetting'];
const baselineFiles=['src/styles.css','src/components/McpSecuritySetting.tsx','src/components/McpStorageGate.tsx','src/components/SettingsPanel.tsx'];
const baseline=process.argv.includes('--baseline')?new Map(baselineFiles.map(path=>[path,execFileSync('git',['show',`HEAD:${path}`],{encoding:'utf8',windowsHide:true})])):new Map();
const server=await createServer({optimizeDeps:{entries:[],noDiscovery:true,include:['react','react-dom/client','react/jsx-runtime','lucide-react']},server:{host:'127.0.0.1',port:0,watch:{ignored:['**/src-tauri/**','**/release/**']}},plugins:[{
  name:'mcp-spacing-isolation',enforce:'pre',
  resolveId(id){if(id==='/__spacing-entry.tsx'||id==='@tauri-apps/api/event')return id;},
  load(id){if(id==='/__spacing-entry.tsx')return entry;if(id==='@tauri-apps/api/event')return 'export const listen=async()=>()=>{};';if(id.replaceAll('\\','/').endsWith('/src/api.ts'))return mock;if(unrelated.some(name=>id.replaceAll('\\','/').endsWith('/src/components/'+name+'.tsx')))return 'export default function UnrelatedSetting(){return null;}';for(const[path,source]of baseline)if(id.replaceAll('\\','/').endsWith('/'+path))return source;},
  configureServer(s){s.middlewares.use(async(req,res,next)=>{if(req.url?.startsWith('/__font/')){const bytes=fontFiles.get(req.url.slice(8));if(!bytes){res.statusCode=404;return res.end();}res.setHeader('Content-Type','font/woff2');return res.end(bytes);}if(req.url!=='/__spacing.html')return next();res.setHeader('Content-Type','text/html');res.end(await s.transformIndexHtml(req.url,'<!doctype html><html><body><div id="root"></div><script type="module" src="/__spacing-entry.tsx"></script></body></html>'));});}
}]});
await server.listen();const browser=await chromium.launch({headless:true,executablePath:process.env.INLINE_BROWSER_PATH});
const samples=[],errors=[];
try{
 const page=await browser.newPage();page.setDefaultTimeout(12000);page.on('pageerror',e=>errors.push(e.message));
 const cdp=await page.context().newCDPSession(page);
 async function viewport(width,scale,textScale){
   await page.setViewportSize({width:Math.floor(width/scale),height:Math.floor(900/scale)});
   await cdp.send('Emulation.setDeviceMetricsOverride',{width:Math.floor(width/scale),height:Math.floor(900/scale),deviceScaleFactor:scale,mobile:false});
   await page.locator('.settings-page').evaluate((e,textScale)=>e.style.setProperty('--settings-control-font-size',`${14*textScale}px`),textScale);
 }
 async function measure(scene,width,scale,textScale){
   await page.evaluate(()=>document.fonts.ready);await page.waitForTimeout(30);
   const sample=await page.evaluate(()=>{
     const modal=document.querySelector('.mcp-connect-dialog[open],.mcp-content-dialog');
     const roots=modal?[modal]:[...document.querySelectorAll('.mcp-security-setting,.mcp-storage-gate,.mcp-connection-row')];
     const visible=e=>e.checkVisibility()&&e.getBoundingClientRect().width>0;
     const buttons=[...new Set(roots.flatMap(r=>[...r.querySelectorAll('button')]))].filter(visible);
     const issues=[];let minGap=Infinity;
     for(const b of buttons){const r=b.getBoundingClientRect(),parent=b.parentElement.getBoundingClientRect();const range=document.createRange();range.selectNodeContents(b);const text=range.getBoundingClientRect();
       if(b.scrollWidth>b.clientWidth+1||b.scrollHeight>b.clientHeight+1||text.top<r.top-1||text.bottom>r.bottom+1)issues.push('text overflow: '+b.textContent);
       if(r.left<parent.left-1||r.right>parent.right+1||r.left<0||r.right>innerWidth+1)issues.push('button overflow: '+b.textContent);
     }
     for(let i=0;i<buttons.length;i++)for(let j=i+1;j<buttons.length;j++){
       const a=buttons[i].getBoundingClientRect(),b=buttons[j].getBoundingClientRect();const x=Math.max(a.left-b.right,b.left-a.right),y=Math.max(a.top-b.bottom,b.top-a.bottom);
       if(x<-0.5&&y<-0.5)issues.push('overlap: '+buttons[i].textContent+' / '+buttons[j].textContent);
       // Adjacent controls in the same action group must retain at least 8 CSS px.
       if(buttons[i].parentElement===buttons[j].parentElement){const gap=x>=0&&y<0?x:y>=0&&x<0?y:Math.hypot(Math.max(0,x),Math.max(0,y));minGap=Math.min(minGap,gap);if(gap<7.5)issues.push('gap '+gap+': '+buttons[i].textContent+' / '+buttons[j].textContent);}
     }
     for(const r of roots.filter(visible)){if(r.scrollWidth>r.clientWidth+1)issues.push('container overflow: '+r.className);}
     return {buttons:buttons.length,minGap:Number.isFinite(minGap)?minGap:null,issues,font:document.documentElement.dataset.uiFont,cssWidth:innerWidth};
   });
   samples.push({scene,width,scale,textScale,...sample});assert.ok(sample.buttons>0);assert.equal(sample.cssWidth,Math.floor(width/scale));assert.equal(sample.font,'In Line Sarasa UI SC');assert.deepEqual(sample.issues,[],JSON.stringify(samples.at(-1)));
 }
 async function matrix(scene){for(const width of [1280,980,560])for(const scale of [1,1.1,1.5])for(const textScale of [1,1.5]){await viewport(width,scale,textScale);await measure(scene,width,scale,textScale);}}
 await page.goto(server.resolvedUrls.local[0]+'__spacing.html');await page.waitForFunction(()=>document.documentElement.dataset.uiFont==='In Line Sarasa UI SC');await page.evaluate(()=>document.fonts.ready);
 assert.deepEqual(await page.evaluate(()=>Array.from(document.fonts).map(f=>f.weight).sort()),['400','600','700']);
 await page.evaluate(()=>document.querySelectorAll('.mcp-security-setting').forEach(d=>d.open=true));
 await page.getByRole('button',{name:'清理报告缓存',exact:true}).waitFor();
 await matrix('advanced management');
 await page.getByRole('button',{name:'审阅并确认',exact:true}).click();await page.getByRole('button',{name:'审阅并撤销',exact:true}).click();await page.getByRole('button',{name:'取消任务',exact:true}).click();
 await page.evaluate(()=>document.querySelectorAll('.mcp-security-setting details').forEach(d=>d.open=true));
 await page.getByRole('button',{name:'清理已恢复',exact:true}).click();
 await page.getByRole('button',{name:'预览清理',exact:true}).click();
 await matrix('batch undo job fault and maintenance confirmations');
 await page.getByRole('button',{name:'连接 AI',exact:true}).click();const dialog=page.getByRole('dialog',{name:'连接 AI · MCP / CLI'});await dialog.waitFor();
 await matrix('connection permissions');
 await dialog.getByRole('button',{name:'复用授权并复制接入提示词',exact:true}).click();await dialog.getByRole('button',{name:'再次复制',exact:true}).waitFor();
 await dialog.getByRole('button',{name:'再次复制',exact:true}).click();assert.equal(await page.evaluate(()=>__test.calls.filter(c=>c[0]==='reuse').length),1);assert.equal(await page.evaluate(()=>__test.calls.filter(c=>c[0]==='grant').length),0);
 await matrix('connection receipt actions');await mkdir('release/ui',{recursive:true});await dialog.locator('.mcp-button-actions').scrollIntoViewIfNeeded();await page.screenshot({path:'release/ui/import-spacing-connection.png'});
 await page.keyboard.press('Escape');
 await page.getByRole('button',{name:'通用接入',exact:true}).click();await page.getByRole('dialog',{name:'通用 MCP 接入',exact:true}).waitFor();await matrix('generic guide footer');await page.keyboard.press('Escape');
 await page.getByRole('button',{name:'权限',exact:true}).click();await page.getByRole('dialog',{name:'修改客户端权限'}).waitFor();await matrix('edit permission actions');await page.keyboard.press('Escape');
 await page.evaluate(()=>{__test.mode='migration';__test.remount();});await page.getByRole('button',{name:'迁移并重启软件',exact:true}).waitFor();await matrix('storage migration actions');await page.screenshot({path:'release/ui/import-spacing-storage.png'});
 assert.equal(await page.getByRole('button',{name:'迁移并重启软件',exact:true}).isDisabled(),true);assert.deepEqual(errors,[]);
 const report={passed:true,productionDataTouched:false,nativeMigrationExecuted:false,scaleMethod:'effective CSS viewport and DPR equivalent to native WebView zoom; not desktop acceptance',font:'hash-verified Sarasa UI SC 400/600/700',samples};
 await writeFile('release/Import-Spacing-ui-check.json',JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify({passed:true,scenarios:samples.length,minGap:Math.min(...samples.map(s=>s.minGap??Infinity)),productionDataTouched:false}));
}finally{await browser.close();await server.close();}
