// Real React components in an isolated browser. No user data, grants or config.
import assert from 'node:assert/strict';
import {mkdir,readFile,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {pathToFileURL} from 'node:url';
import {createServer} from 'vite';
const {chromium}=await import(process.env.INLINE_PLAYWRIGHT_MODULE?pathToFileURL(process.env.INLINE_PLAYWRIGHT_MODULE).href:'playwright');
const fontFiles=new Map();
for(const asset of JSON.parse(await readFile('docs/fonts/source.json','utf8')).files){const bytes=await readFile(`release/font-cache-fixture/${asset.file}`);assert.equal(bytes.length,asset.bytes);assert.equal(createHash('sha256').update(bytes).digest('hex'),asset.sha256);fontFiles.set(asset.file,bytes);}
const mock=`
const callbacks=new Set();
window.__test={font:'Arial',scale:'130',fontFailures:1,scaleFailures:1,calls:[],copyFailures:1,grantCount:0};
window.__TAURI_INTERNALS__={convertFileSrc:file=>'/__font/'+file};
const t=window.__test;
const state={paused:true,groups:{regularRead:false,fullRead:false,write:false},revision:1,sensitiveNoticeAcknowledged:false,clients:[],onboardingClients:[{id:'codex',label:'Codex（MCP）',available:false,authorizationAvailable:true,status:'自动接入验证中'},{id:'codex-cli',label:'Codex（CLI）',available:false,authorizationAvailable:true,status:'独立授权'},{id:'powershell-cli',label:'PowerShell（CLI）',available:false,authorizationAvailable:false,status:'未安装'}]};
t.state=state;t.emit=()=>callbacks.forEach(cb=>cb());
const api={
 getUIFontSelection:async()=>{t.calls.push('fontRead');if(t.fontFailures-->0)throw Error('startup font unavailable');const value={requested:t.font,effective:t.font,missing:false};if(t.holdFont){t.holdFont=false;return new Promise(resolve=>t.releaseFont=()=>resolve(value));}return value;},
 getUIScale:async()=>{t.calls.push('scaleRead');if(t.scaleFailures-->0)throw Error('startup zoom unavailable');return t.scale;},
 setSetting:async(key,value)=>{t.calls.push(['settingWrite',key,value]);if(key==='ui_scale')t.scale=value;},onDataChanged:cb=>{callbacks.add(cb);return()=>callbacks.delete(cb);},
 mcpSecurityState:async()=>structuredClone(state),
 mcpSetGroups:async p=>{t.calls.push(['groups',p]);state.groups=p;},mcpSetPaused:async v=>{t.calls.push(['pause',v]);state.paused=v;},mcpAcknowledgeSensitiveNotice:async()=>{t.calls.push('notice');state.sensitiveNoticeAcknowledged=true;},
 mcpPrepareOnboarding:async args=>{t.grantCount++;t.calls.push(['grant',args]);await new Promise(r=>setTimeout(r,80));const receipt={packageId:'fixture-package',clientId:'fixture-client',expiresAt:Math.floor(Date.now()/1000)+600,prompt:'本地安全导入指令（隔离测试，无凭证）'};state.clients.push({id:receipt.clientId,name:args.client,kind:args.client==='codex'?'codex':'cli',permissions:args.permissions,scope:args.scope,revoked:false});return receipt;},
 copyText:async text=>{t.calls.push(['copy',text]);if(t.copyFailures-->0)throw Error('clipboard unavailable');},
 mcpBatchState:async()=>({pending:[],batchLimit:20}),mcpFaultState:async()=>({items:[],latestClients:[],unavailable:false}),mcpJobsState:async()=>({items:[],storageError:false}),mcpAuditState:async()=>({audits:[],pending:[]})
};export {api};`;
const entry=`import React from 'react';import{createRoot}from'react-dom/client';import Font,{useUIFont}from'/src/components/UIFontProvider.tsx';import Scale,{useUIScale}from'/src/components/UIScaleProvider.tsx';import Mcp from'/src/components/McpSecuritySetting.tsx';import'/src/styles.css';
function Probe(){const f=useUIFont(),s=useUIScale();return <><output id="font">{f.selection.effective}|{String(f.unavailable)}</output><output id="scale">{s.scale}|{String(s.unavailable)}</output><button onClick={()=>s.save(150)}>save scale</button><Mcp notify={()=>{}}/></>};
const root=createRoot(document.getElementById('root'));window.__unmount=()=>root.unmount();root.render(<Font><Scale><Probe/></Scale></Font>);`;
const server=await createServer({optimizeDeps:{entries:[],noDiscovery:true,include:['react','react-dom/client','react/jsx-runtime','lucide-react']},server:{port:0,watch:{ignored:['**/src-tauri/**','**/release/**']}},plugins:[{name:'post-phase6-isolation',enforce:'pre',resolveId(id){if(id==='/__regression-entry.tsx'||id==='@tauri-apps/api/event')return id;},load(id){if(id==='/__regression-entry.tsx')return entry;if(id==='@tauri-apps/api/event')return 'export const listen=async()=>()=>{};';if(id.replaceAll('\\','/').endsWith('/src/api.ts'))return mock;},configureServer(s){s.middlewares.use(async(req,res,next)=>{if(req.url?.startsWith('/__font/')){const bytes=fontFiles.get(req.url.slice(8));if(!bytes){res.statusCode=404;return res.end();}res.setHeader('Content-Type','font/woff2');return res.end(bytes);}if(req.url!=='/__regression.html')return next();res.setHeader('Content-Type','text/html');res.end(await s.transformIndexHtml(req.url,'<!doctype html><html><body><div id="root"></div><script type="module" src="/__regression-entry.tsx"></script></body></html>'));});}}]});
await server.listen();const browser=await chromium.launch({headless:true,executablePath:process.env.INLINE_BROWSER_PATH});const errors=[];
try{
 const page=await browser.newPage({viewport:{width:1000,height:850}});page.setDefaultTimeout(12000);page.on('pageerror',e=>{errors.push(e.message);console.error('browser:',e.stack);});await page.goto(server.resolvedUrls.local[0]+'__regression.html',{waitUntil:'domcontentloaded'});
 await page.waitForFunction(()=>document.querySelector('#font')?.textContent==='Arial|false'&&document.querySelector('#scale')?.textContent==='130|false');
 assert.equal(await page.evaluate(()=>window.__test.calls.filter(c=>Array.isArray(c)&&c[0]==='settingWrite').length),0);
 await page.evaluate(()=>{__test.fontFailures=10;__test.scaleFailures=10;__test.emit();});
 await page.waitForFunction(()=>document.querySelector('#font').textContent==='Arial|true'&&document.querySelector('#scale').textContent==='130|true');
 await page.waitForTimeout(2350);assert.equal(await page.getAttribute('html','data-ui-font'),'Arial');
 await page.evaluate(()=>{__test.fontFailures=0;__test.scaleFailures=0;window.dispatchEvent(new Event('focus'));});
 await page.waitForFunction(()=>document.querySelector('#font').textContent==='Arial|false'&&document.querySelector('#scale').textContent==='130|false');
 await page.evaluate(()=>{__test.holdFont=true;__test.emit();});await page.waitForFunction(()=>Boolean(__test.releaseFont));
 await page.evaluate(()=>{__test.font='Microsoft YaHei UI';__test.emit();});await page.waitForFunction(()=>document.documentElement.dataset.uiFont==='Microsoft YaHei UI');
 await page.evaluate(()=>__test.releaseFont());assert.equal(await page.getAttribute('html','data-ui-font'),'Microsoft YaHei UI');
 await page.evaluate(()=>{const RealFontFace=window.FontFace;window.__fontLoadFailures=1;window.FontFace=class extends RealFontFace{load(){if(window.__fontLoadFailures-->0)return Promise.reject(Error('isolated font protocol startup failure'));return super.load();}};__test.font='in-line:sarasa-ui-sc';__test.emit();});
 await page.waitForFunction(()=>document.documentElement.dataset.uiFont==='In Line Sarasa UI SC');
 assert.deepEqual(await page.evaluate(()=>Array.from(document.fonts).filter(f=>f.family.replaceAll('\"','')==='In Line Sarasa UI SC').map(f=>f.weight).sort()),['400','600','700']);
 await page.getByRole('button',{name:'连接 AI',exact:true}).click();const dialog=page.getByRole('dialog',{name:'连接 AI · MCP / CLI'});await dialog.waitFor();
 assert.equal(await dialog.getByLabel('常规读取',{exact:true}).isChecked(),true);assert.equal(await dialog.getByLabel('完整读取',{exact:true}).isChecked(),false);assert.equal(await dialog.getByLabel('日常写入',{exact:true}).isChecked(),false);
 const grant=dialog.getByRole('button',{name:'授权并复制接入提示词',exact:true});assert.ok(await grant.isDisabled());
 await dialog.getByLabel('客户端',{exact:true}).selectOption('powershell-cli');assert.ok(await grant.isDisabled());await dialog.getByLabel('客户端',{exact:true}).selectOption('codex-cli');
 await dialog.getByLabel('完整读取',{exact:true}).check();await dialog.getByLabel('同时开启全局权限',{exact:false}).check();await dialog.getByLabel('解除全部外部接入暂停',{exact:false}).check();assert.ok(await grant.isDisabled());
 await dialog.getByLabel('我了解完整读取',{exact:false}).check();await dialog.getByText('限制部门或事项类型（可选）',{exact:true}).click();await dialog.getByLabel('部门范围',{exact:true}).fill('部门 A，部门 B');await dialog.getByLabel('事项类型范围',{exact:true}).fill('合同');
 await grant.click();await dialog.getByRole('alert').filter({hasText:'复制失败'}).waitFor();
 assert.equal(await page.evaluate(()=>__test.grantCount),1);await dialog.getByRole('button',{name:'再次复制',exact:true}).click();assert.equal(await page.evaluate(()=>__test.grantCount),1);
 const args=await page.evaluate(()=>__test.calls.find(c=>Array.isArray(c)&&c[0]==='grant')[1]);assert.deepEqual(args,{client:'codex-cli',permissions:{regularRead:true,fullRead:true,write:false},scope:{departments:['部门 A','部门 B'],taskTypes:['合同']}});
 assert.deepEqual(await page.evaluate(()=>__test.state.groups),{regularRead:true,fullRead:true,write:false});assert.equal(await page.evaluate(()=>__test.state.paused),false);
 assert.equal(await dialog.getByLabel('客户端',{exact:true}).isDisabled(),true);assert.equal(await dialog.getByLabel('接入提示词',{exact:true}).inputValue(),'本地安全导入指令（隔离测试，无凭证）');
 await page.keyboard.press('Escape');await dialog.waitFor({state:'hidden'});await page.getByRole('button',{name:'连接 AI',exact:true}).click();await dialog.waitFor();assert.equal(await dialog.getByLabel('客户端',{exact:true}).inputValue(),'codex-cli');assert.equal(await page.evaluate(()=>__test.grantCount),1);
 await mkdir('release/ui',{recursive:true});await page.setViewportSize({width:560,height:800});await page.screenshot({path:'release/ui/connect-dialog-isolated.png'});assert.ok(await dialog.evaluate(e=>e.scrollWidth<=e.clientWidth+1));
 await page.keyboard.press('Escape');await page.getByRole('button',{name:'save scale',exact:true}).click();await page.waitForFunction(()=>document.querySelector('#scale').textContent==='150|false');
 await page.evaluate(()=>{__test.fontFailures=100;__test.scaleFailures=100;__test.emit();});await page.waitForTimeout(30);await page.evaluate(()=>window.__unmount());const reads=await page.evaluate(()=>__test.calls.filter(c=>typeof c==='string'&&c.endsWith('Read')).length);await page.waitForTimeout(500);assert.equal(await page.evaluate(()=>__test.calls.filter(c=>typeof c==='string'&&c.endsWith('Read')).length),reads);
 assert.deepEqual(errors,[]);const result={passed:true,productionDataTouched:false,checks:['startup preference retry','preserve loaded font and scale on transient failure','focus recovery','stale response rejected','real verified Sarasa weights and font-load failure retry','unmount cancels retries','dialog defaults and unavailable clients','explicit global/resume/sensitive confirmations','CLI independent scope','clipboard retry without duplicate grant','receipt retained on reopen','narrow layout']};await writeFile('release/Post-Phase6-ui-check.json',JSON.stringify(result,null,2));console.log(JSON.stringify(result));
}finally{await browser.close();await server.close();}





