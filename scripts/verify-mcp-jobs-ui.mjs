// Synthetic UI only; no installed app, authorization, credential or business database.
import {createServer} from 'vite';
import {mkdir,writeFile} from 'node:fs/promises';
import path from 'node:path';
import {pathToFileURL} from 'node:url';
const {chromium}=await import(pathToFileURL(process.env.INLINE_PLAYWRIGHT_MODULE).href);
const root=path.resolve('release/ui/mcp-p5');await mkdir(root,{recursive:true});
await writeFile(path.join(root,'entry.jsx'),`import React from 'react';import{createRoot}from'react-dom/client';import Component from '../../../src/components/McpJobsSetting.tsx';import '../../../src/styles.css';createRoot(document.getElementById('root')).render(React.createElement(Component));`);
const fixture={items:[{jobId:'synthetic',clientId:'fixture',kind:'query_tasks',state:'running',progress:{processed:100,total:205},rounds:[{number:1,outcome:null}],errorCode:null,complete:false}]};
const apiCode=`export const api={mcpJobsState:async()=>structuredClone(window.__jobs),mcpCancelJob:async(id)=>{window.__calls.push({cancel:id});await new Promise(r=>setTimeout(r,100));window.__jobs.items[0].state='cancelled';},mcpCleanupJobs:async()=>{window.__calls.push({cleanup:true});return{summariesRemoved:0,temporaryResultsCleared:0};}};`;
const server=await createServer({configFile:false,esbuild:{jsx:'automatic'},cacheDir:'release/ui/mcp-p5/vite-cache',optimizeDeps:{entries:['release/ui/mcp-p5/entry.jsx'],include:['react','react-dom/client']},server:{watch:{ignored:['**/src-tauri/**','**/docs/**']},host:'127.0.0.1',port:0},plugins:[{name:'synthetic-jobs',enforce:'pre',resolveId(id){if(id==='@tauri-apps/api/event')return '\0jobs-event';},load(id){if(id==='\0jobs-event')return 'export async function listen(){return()=>{};}';},transform(code,id){if(id.split('?')[0].replaceAll('\\','/').endsWith('/src/api.ts'))return apiCode;},configureServer(s){s.middlewares.use((req,res,next)=>{if(req.url==='/__jobs.html'){res.setHeader('Content-Type','text/html');res.end('<html lang="zh-CN"><meta charset="utf-8"><div id="root"></div><script type="module" src="/release/ui/mcp-p5/entry.jsx"></script></html>');}else next();});}}]});
let browser;const errors=[];
try {await server.listen();browser=await chromium.launch({headless:true,executablePath:process.env.INLINE_BROWSER_PATH});const page=await browser.newPage({viewport:{width:640,height:850}});page.on('pageerror',e=>errors.push(e.message));await page.addInitScript(fixture=>{window.__jobs=fixture;window.__calls=[];},fixture);await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/__jobs.html`);
await page.getByText('后台查询与导出 · 有任务进行中',{exact:true}).click();await page.getByText('100 / 205 条',{exact:true}).waitFor();
await page.getByRole('button',{name:'取消任务',exact:true}).click();if(await page.evaluate(()=>window.__calls.length)!==0)throw new Error('Opening review must not cancel');
await page.getByRole('button',{name:'继续任务',exact:true}).click();if(await page.evaluate(()=>window.__calls.length)!==0)throw new Error('Dismissing review must not cancel');
await page.getByRole('button',{name:'取消任务',exact:true}).click();await page.getByRole('button',{name:'确认取消',exact:true}).click();await page.getByText('事项查询 · 已取消',{exact:true}).waitFor();if(await page.evaluate(()=>window.__calls.filter(c=>c.cancel).length)!==1)throw new Error('Explicit task cancellation must occur once');
if(await page.getByRole('button',{name:'取消任务',exact:true}).count()!==0)throw new Error('Terminal task must not offer cancellation');
await page.getByRole('button',{name:'清理过期任务数据',exact:true}).click();await page.waitForFunction(()=>window.__calls.some(c=>c.cleanup));
await page.screenshot({path:path.join(root,'jobs.png'),fullPage:true});if(errors.length)throw new Error(errors.join('\n'));
console.log('PASS: real component progress, explicit whole-job review, dismiss without mutation, exactly one cancellation, terminal status, manual cleanup and retention explanation; synthetic data only.');
}finally{await browser?.close();await server.close();}
