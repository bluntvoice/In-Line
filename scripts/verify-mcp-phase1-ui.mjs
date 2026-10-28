// Real connection component with synthetic APIs only. No native config/grants.
import assert from 'node:assert/strict';
import {mkdir,writeFile} from 'node:fs/promises';
import {pathToFileURL} from 'node:url';
import {createServer} from 'vite';
const {chromium}=await import(process.env.INLINE_PLAYWRIGHT_MODULE?pathToFileURL(process.env.INLINE_PLAYWRIGHT_MODULE).href:'playwright');
const mock=`
const permissions={regularRead:true,fullRead:true,write:false};
const make=id=>({id,name:'Codex',kind:'codex',revoked:false,permissions:{...permissions},scope:{departments:[id+'部门'],taskTypes:['合同']}});
const state={paused:false,groups:{regularRead:true,fullRead:true,write:true},revision:20,sensitiveNoticeAcknowledged:true,clients:[make('A'),make('B'),{...make('revoked'),revoked:true},{...make('CLI'),kind:'cli'}],onboardingClients:[{id:'codex',label:'Codex',available:false,authorizationAvailable:true,status:'自动接入验证中'},{id:'codex-cli',label:'Codex CLI',available:false,authorizationAvailable:true,status:'独立授权'}]};
window.__test={state,reuses:[],rotations:[],grants:[],writes:[],copies:[],copyFailures:1,failReuse:false};
const t=window.__test;const realNow=Date.now;Date.now=()=>t.now??realNow();
let sequence=0;const receipt=id=>({packageId:'synthetic-'+(++sequence),clientId:id,expiresAt:Math.floor(Date.now()/1000)+600,prompt:'合成一次性提示词 '+sequence+'（不含凭证）'});
export const api={
 mcpStorageState:async()=>({ready:true}),mcpSecurityState:async()=>structuredClone(state),
 mcpReuseOnboarding:async id=>{t.reuses.push(id);if(state.paused)throw Error('paused：暂停保持');if(t.failReuse)throw Error('onboarding_credentials_unavailable：未新建授权、未自动轮换');return receipt(id);},
 mcpRotateOnboarding:async id=>{t.rotations.push(id);return receipt(id);},
 mcpPrepareOnboarding:async args=>{t.grants.push(structuredClone(args));const c=make('new-client');c.permissions=args.permissions;c.scope=args.scope;state.clients.push(c);return receipt(c.id);},
 copyText:async text=>{t.copies.push(text);if(t.copyFailures-->0)throw Error('synthetic clipboard failure');},
 mcpSetGroups:async()=>{t.writes.push('groups');throw Error('unexpected group write');},mcpSetPaused:async()=>{t.writes.push('pause');throw Error('unexpected pause write');},mcpAcknowledgeSensitiveNotice:async()=>{t.writes.push('notice');throw Error('unexpected notice write');},
 mcpBatchState:async()=>({pending:[],batchLimit:20}),mcpFaultState:async()=>({items:[],latestClients:[],unavailable:false}),mcpJobsState:async()=>({items:[],storageError:false}),mcpAuditState:async()=>({audits:[],pending:[]})
};`;
const entry=`import React from 'react';import{createRoot}from'react-dom/client';import Mcp from'/src/components/McpSecuritySetting.tsx';import'/src/styles.css';createRoot(document.getElementById('root')).render(<div className="settings-page"><Mcp notify={()=>{}}/></div>);`;
const server=await createServer({optimizeDeps:{entries:[],noDiscovery:true,include:['react','react-dom/client','react/jsx-runtime','lucide-react']},server:{port:0,watch:{ignored:['**/src-tauri/**','**/release/**']}},plugins:[{name:'phase1-isolated',enforce:'pre',resolveId(id){if(id==='/__phase1.tsx'||id==='@tauri-apps/api/event')return id;},load(id){if(id==='/__phase1.tsx')return entry;if(id==='@tauri-apps/api/event')return 'export const listen=async()=>()=>{};';if(id.replaceAll('\\','/').endsWith('/src/api.ts'))return mock;},configureServer(s){s.middlewares.use(async(req,res,next)=>{if(req.url!=='/__phase1.html')return next();res.setHeader('Content-Type','text/html');res.end(await s.transformIndexHtml(req.url,'<!doctype html><html><body><div id="root"></div><script type="module" src="/__phase1.tsx"></script></body></html>'));});}}]});
await server.listen();
const browser=await chromium.launch({headless:true,executablePath:process.env.INLINE_BROWSER_PATH});
const out='release/MCP-Phase1-20261028';
try{
 const page=await browser.newPage({viewport:{width:980,height:900}});page.setDefaultTimeout(10000);const errors=[];page.on('pageerror',e=>errors.push(e.message));
 await page.goto(server.resolvedUrls.local[0]+'__phase1.html');
 await page.getByRole('button',{name:'连接 AI',exact:true}).click();
 const dialog=page.getByRole('dialog',{name:'连接 AI · MCP / CLI'});const identity=dialog.getByLabel('授权身份',{exact:true});
 const reuse=()=>dialog.getByRole('button',{name:'复用授权并复制接入提示词',exact:true});
 assert.equal(await identity.inputValue(),'');assert.ok(await reuse().isDisabled());
 assert.equal(await identity.locator('option').count(),4); // placeholder, A, B, explicit new
 await identity.selectOption('A');assert.ok(await dialog.getByLabel('完整读取',{exact:true}).isDisabled());
 await dialog.getByText('限制部门或事项类型（可选）',{exact:true}).click();
 assert.equal(await dialog.getByLabel('部门范围',{exact:true}).inputValue(),'A部门');assert.ok(await dialog.getByLabel('部门范围',{exact:true}).isDisabled());
 await reuse().click();await dialog.getByRole('alert').filter({hasText:'复制失败'}).waitFor();
 await dialog.getByRole('button',{name:'再次复制',exact:true}).click();
 assert.deepEqual(await page.evaluate(()=>__test.reuses),['A']);assert.equal(await page.evaluate(()=>__test.grants.length),0);
 await page.keyboard.press('Escape');await page.getByRole('button',{name:'连接 AI',exact:true}).click();assert.equal(await identity.inputValue(),'A');
 assert.match(await dialog.getByRole('status').last().innerText(),/当前会话可用.*自动恢复仍未验证/);
 await page.evaluate(()=>__test.now=Date.now()+601000);await reuse().waitFor();await reuse().click();
 assert.deepEqual(await page.evaluate(()=>__test.reuses),['A','A']);
 await dialog.getByRole('button',{name:'发起新的接入',exact:true}).click();assert.equal(await identity.inputValue(),'A');
 await page.evaluate(()=>__test.failReuse=true);await reuse().click();await dialog.getByRole('alert').filter({hasText:'onboarding_credentials_unavailable'}).waitFor();
 assert.equal(await page.evaluate(()=>__test.rotations.length),0);assert.equal(await page.evaluate(()=>__test.grants.length),0);
 await dialog.getByRole('button',{name:'轮换并接入（旧凭证立即失效）',exact:true}).click();assert.deepEqual(await page.evaluate(()=>__test.rotations),['A']);
 await dialog.getByRole('button',{name:'发起新的接入',exact:true}).click();await identity.selectOption('new');await page.evaluate(()=>__test.failReuse=false);
 await dialog.getByRole('button',{name:'授权并复制接入提示词',exact:true}).click();
 assert.equal(await page.evaluate(()=>__test.grants.length),1);assert.equal(await identity.inputValue(),'new-client');
 await page.evaluate(()=>__test.now=Date.now()+601000);await reuse().waitFor();await reuse().click();
 assert.equal(await page.evaluate(()=>__test.grants.length),1);assert.equal(await page.evaluate(()=>__test.reuses.at(-1)),'new-client');
 await dialog.getByRole('button',{name:'发起新的接入',exact:true}).click();await page.evaluate(()=>__test.state.paused=true);await reuse().click();
 await dialog.getByRole('alert').filter({hasText:'paused'}).waitFor();assert.equal(await page.evaluate(()=>__test.state.paused),true);
 assert.deepEqual(await page.evaluate(()=>__test.writes),[]);
 const samples=[];await mkdir(out,{recursive:true});
 for(const width of [1280,980,560])for(const scale of [1,1.5]){
   await page.setViewportSize({width:Math.floor(width/scale),height:Math.floor(1000/scale)});
   await page.locator('.settings-page').evaluate(e=>e.style.setProperty('--settings-control-font-size','21px'));
   const issues=await dialog.evaluate(root=>{const bad=[];if(root.scrollWidth>root.clientWidth+1)bad.push('dialog overflow');const buttons=[...root.querySelectorAll('button')].filter(e=>e.checkVisibility());for(const b of buttons){if(b.scrollWidth>b.clientWidth+1||b.scrollHeight>b.clientHeight+1)bad.push('button text overflow');}for(let i=0;i<buttons.length;i++)for(let j=i+1;j<buttons.length;j++){const a=buttons[i].getBoundingClientRect(),b=buttons[j].getBoundingClientRect();if(Math.min(a.right,b.right)>Math.max(a.left,b.left)+1&&Math.min(a.bottom,b.bottom)>Math.max(a.top,b.top)+1)bad.push('overlap');}return bad;});
   samples.push({width,scale,textPercent:150,issues});assert.deepEqual(issues,[],JSON.stringify(samples.at(-1)));
 }
 await page.screenshot({path:out+'/reuse-dialog-narrow.png'});
 await page.evaluate(()=>__test.state.paused=false);await dialog.getByLabel('客户端',{exact:true}).selectOption('codex-cli');await page.keyboard.press('Escape');
 await page.getByText('已授权客户端与高级管理',{exact:true}).click();
 await page.locator('.mcp-client-list > li').first().getByRole('button',{name:'轮换并接入',exact:true}).click();
 assert.equal(await dialog.getByLabel('客户端',{exact:true}).inputValue(),'codex');assert.equal(await identity.inputValue(),'A');
 assert.deepEqual(await page.evaluate(()=>__test.rotations),['A','A']);assert.deepEqual(await page.evaluate(()=>__test.writes),[]);assert.deepEqual(errors,[]);
 const result={passed:true,productionDataTouched:false,nativeAcceptance:false,checks:['multiple identities require choice','CLI/revoked identities excluded','selected grant and scope unchanged','clipboard retry keeps receipt','reopen keeps identity','expiry reissues same identity','missing credentials never auto-grant/rotate','rotation only on explicit same-ID action','new grant then expiry reuses its identity','global pause preserved','advanced rotation selects target identity after CLI selection','configuration and session/recovery remain distinct'],samples};
 await writeFile(out+'/ui-check.json',JSON.stringify(result,null,2));console.log(JSON.stringify(result));
}finally{await browser.close();await server.close();}
