// Real migration UI against isolated API fixtures; never invoke the native migration.
import assert from 'node:assert/strict';
import {mkdir,writeFile} from 'node:fs/promises';
import {pathToFileURL} from 'node:url';
import {createServer} from 'vite';
const {chromium}=await import(process.env.INLINE_PLAYWRIGHT_MODULE?pathToFileURL(process.env.INLINE_PLAYWRIGHT_MODULE).href:'playwright');
const mock=`
window.__test={attempts:[],fail:true,status:{ready:false,migrationRequired:true,canMigrate:true,legacyFileIdentity:'synthetic-file-identity',legacyRoot:'D:/synthetic/AppData/security',legacyPhysicalFile:'D:/synthetic/AppData/security/authorization.dpapi',root:'D:/synthetic/profile/.in-line/mcp-security',policy:'profile_private_v1',legacyRedirected:false,errorCode:null}};
export const api={mcpStorageState:async()=>structuredClone(window.__test.status),mcpMigrateStorage:async(id,confirmed)=>{window.__test.attempts.push({id,confirmed});if(window.__test.fail)throw Error('synthetic source changed');return {sourcePreserved:true,clients:3,copiedFiles:8};}};`;
const entry=`import React from 'react';import{createRoot}from'react-dom/client';import Gate from'/src/components/McpStorageGate.tsx';import'/src/styles.css';function App(){const[key,setKey]=React.useState(0);window.__reload=()=>setKey(k=>k+1);return <Gate key={key}><button>连接 AI</button></Gate>;}createRoot(document.getElementById('root')).render(<App/>);`;
const server=await createServer({optimizeDeps:{entries:[],noDiscovery:true,include:['react','react-dom/client','react/jsx-runtime']},server:{port:0,watch:{ignored:['**/src-tauri/**','**/release/**']}},plugins:[{name:'msix-storage-isolation',enforce:'pre',resolveId(id){if(id==='/__storage-entry.tsx')return id;},load(id){if(id==='/__storage-entry.tsx')return entry;if(id.replaceAll('\\','/').endsWith('/src/api.ts'))return mock;},configureServer(s){s.middlewares.use(async(req,res,next)=>{if(req.url!=='/__storage.html')return next();res.setHeader('Content-Type','text/html');res.end(await s.transformIndexHtml(req.url,'<!doctype html><html><body><div id="root"></div><script type="module" src="/__storage-entry.tsx"></script></body></html>'));});}}]});
await server.listen();const browser=await chromium.launch({headless:true,executablePath:process.env.INLINE_BROWSER_PATH});
try{
 const page=await browser.newPage({viewport:{width:900,height:700}});const errors=[];page.on('pageerror',e=>errors.push(e.message));
 await page.goto(server.resolvedUrls.local[0]+'__storage.html');
 const migrate=page.getByRole('button',{name:'迁移并重启软件',exact:true});await migrate.waitFor();
 assert.ok(await migrate.isDisabled());assert.equal(await page.getByRole('button',{name:'连接 AI',exact:true}).count(),0);
 const consent=page.getByLabel('我已保存当前工作，确认迁移并重新启动 In Line',{exact:true});
 await consent.check();await migrate.click();await page.getByRole('alert').filter({hasText:'synthetic source changed'}).waitFor();
 assert.ok(await migrate.isDisabled());assert.equal(await consent.isChecked(),false);
 await page.waitForTimeout(350);assert.equal(await page.evaluate(()=>__test.attempts.length),1);
 await page.evaluate(()=>__test.fail=false);await consent.check();await migrate.click();
 await page.getByRole('status').filter({hasText:'迁移完成，原文件已保留'}).waitFor();
 assert.deepEqual(await page.evaluate(()=>__test.attempts),[{id:'synthetic-file-identity',confirmed:true},{id:'synthetic-file-identity',confirmed:true}]);
 await page.evaluate(()=>{__test.status.canMigrate=false;__test.status.legacyRedirected=true;__test.status.errorCode='storage_redirected';__reload();});
 await page.getByRole('alert').filter({hasText:'读取的是隔离副本'}).waitFor();assert.equal(await migrate.count(),0);
 assert.equal(await page.getByRole('button',{name:'连接 AI',exact:true}).count(),0);
 await page.evaluate(()=>{__test.status.ready=true;__reload();});await page.getByRole('button',{name:'连接 AI',exact:true}).waitFor();
 assert.equal(await page.getByText('AI 接入存储升级',{exact:true}).count(),0);
 assert.deepEqual(errors,[]);
 await mkdir('release/ui',{recursive:true});
 const result={passed:true,productionDataTouched:false,nativeMigrationExecuted:false,checks:['explicit consent before migration','connection controls gated while pending','source-change failure clears confirmation without retry','successful receipt remains separate from session verification','redirected legacy source cannot migrate','ready shared root ignores legacy shadow']};
 await writeFile('release/MSIX-Storage-ui-check.json',JSON.stringify(result,null,2));console.log(JSON.stringify(result));
}finally{await browser.close();await server.close();}
