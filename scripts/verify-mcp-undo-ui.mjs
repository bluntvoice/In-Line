// Isolated synthetic UI: no Tauri program, business database or authorization access.
import {createServer} from 'vite';
import {mkdir,writeFile} from 'node:fs/promises';
import path from 'node:path';
import {pathToFileURL} from 'node:url';
const {chromium}=await import(pathToFileURL(process.env.INLINE_PLAYWRIGHT_MODULE).href);
const fixture={audits:[{id:1,clientId:'synthetic',action:'patch',taskId:11,reason:'合成测试：用户明确修改名称',intent:'名称更新',createdAt:'2026-10-10',before:{task:{title:'原名称',status:'pending'}},after:{task:{title:'新名称',status:'pending'}},undoOf:null}],pending:[{id:1,auditId:1,reason:'合成撤销申请',createdAt:'2026-10-10'}]};
await mkdir(path.resolve('release/ui/mcp-p3'),{recursive:true});
await writeFile(path.resolve('release/ui/mcp-p3/audit-entry.jsx'),`import React from 'react';import{createRoot}from 'react-dom/client';import Component from '../../../src/components/McpAuditSetting.tsx';import '../../../src/styles.css';createRoot(document.getElementById('root')).render(React.createElement(Component));`);
const apiCode=`export const api={mcpAuditState:async()=>structuredClone(window.__auditFixture),mcpResolveUndo:async(id,approve)=>{window.__auditCalls.push({id,approve});await new Promise(r=>setTimeout(r,100));if(window.__auditConflict)throw new Error('conflict：事项已变化，未撤销');window.__auditFixture.pending=[];return {status:approve?'approved':'rejected'};}};`;
const server=await createServer({configFile:false,esbuild:{jsx:"automatic"},cacheDir:'release/ui/mcp-p3/vite-cache',optimizeDeps:{entries:['release/ui/mcp-p3/audit-entry.jsx'],include:['react','react-dom/client']},server:{watch:{ignored:['**/src-tauri/**','**/docs/**']},strictPort:false,host:'127.0.0.1',port:0},plugins:[{name:'synthetic-audit-fixture',enforce:'pre',resolveId(id){if(id==='@tauri-apps/api/event')return '\0audit-event';},load(id){if(id==='\0audit-event')return 'export async function listen(){return()=>{};}';},transform(code,id){if(id.split('?')[0].replaceAll('\\','/').endsWith('/src/api.ts'))return apiCode;},configureServer(server){server.middlewares.use((req,res,next)=>{if(req.url==='/__audit.html'){res.setHeader('Content-Type','text/html');res.end(`<html lang="zh-CN"><meta charset="utf-8"><div id="root"></div><script type="module" src="/release/ui/mcp-p3/audit-entry.jsx"></script></html>`);}else next();});}}]});
let browser;
try{
 await server.listen();const port=server.httpServer.address().port;
 browser=await chromium.launch({headless:true,executablePath:process.env.INLINE_BROWSER_PATH});
 let page;let pageErrors=[];
 const open=async(conflict=false)=>{await page?.close();page=await browser.newPage({viewport:{width:680,height:900}});page.on('pageerror',e=>{pageErrors.push(e.message);console.error(e.message);});await page.addInitScript(({fixture,conflict})=>{window.__auditFixture=fixture;window.__auditCalls=[];window.__auditConflict=conflict;},{fixture,conflict});await page.goto(`http://127.0.0.1:${port}/__audit.html`,{waitUntil:"domcontentloaded"});await page.getByText('AI 操作记录与撤销申请').click();await page.getByRole('button',{name:'审阅并撤销',exact:true}).waitFor();};
 await open();await page.getByText('查看原操作前后变化').click();const review=await page.locator('.mcp-issued').innerText();if(!review.includes('"原名称"')||!review.includes('"新名称"'))throw new Error('Review must show both exact field values: '+review);
 await page.getByRole('button',{name:'审阅并撤销',exact:true}).click();
 if(await page.evaluate(()=>window.__auditCalls.length)!==0)throw new Error('Review must not execute undo');
 await mkdir(path.resolve('release/ui/mcp-p3'),{recursive:true});await page.screenshot({path:path.resolve('release/ui/mcp-p3/undo-review.png'),fullPage:true});
 await page.getByRole('button',{name:'确认撤销',exact:true}).click();await page.waitForFunction(()=>window.__auditFixture.pending.length===0);
 const calls=await page.evaluate(()=>window.__auditCalls);if(calls.length!==1||calls[0].approve!==true)throw new Error('Explicit approval must issue one request');
 await open(true);await page.getByRole('button',{name:'审阅并撤销',exact:true}).click();await page.getByRole('button',{name:'确认撤销',exact:true}).click();await page.getByRole('alert').filter({hasText:'未撤销'}).waitFor();
 if(await page.evaluate(()=>window.__auditFixture.pending.length)!==1)throw new Error('Conflict must retain pending request');
 await page.getByRole('button',{name:'拒绝',exact:true}).count().then(async n=>{if(n===0)await page.getByRole('button',{name:'取消',exact:true}).click();});
 await page.evaluate(()=>window.__auditConflict=false);await page.getByRole('button',{name:'拒绝',exact:true}).click();await page.waitForFunction(()=>window.__auditFixture.pending.length===0);
 if(pageErrors.length)throw new Error(pageErrors.join('\n'));
 console.log('PASS: synthetic browser review shows exact before/after; reviewing alone never submits; explicit approval sends one call; conflict remains pending; rejection is independent. Production program/data/config not accessed.');
}finally{await browser?.close();await server.close();}
