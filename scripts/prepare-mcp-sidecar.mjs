import { copyFileSync, existsSync, mkdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root=resolve(dirname(fileURLToPath(import.meta.url)),"..");
const manifest=join(root,"src-tauri","Cargo.toml");
const triple=execFileSync("rustc",["--print","host-tuple"],{encoding:"utf8"}).trim();

execFileSync("cargo",["build","--locked","--manifest-path",manifest,"--release","--bin","in-line-mcp"],{
  cwd:root,
  stdio:"inherit"
});

const metadata=JSON.parse(execFileSync("cargo",["metadata","--no-deps","--format-version","1","--manifest-path",manifest],{
  cwd:root,
  encoding:"utf8"
}));
const extension=process.platform==="win32"?".exe":"";
const source=join(metadata.target_directory,"release",`in-line-mcp${extension}`);
const target=join(root,"src-tauri","binaries",`in-line-mcp-${triple}${extension}`);
if(!existsSync(source))throw new Error(`未找到 MCP 构建产物：${source}`);
mkdirSync(dirname(target),{recursive:true});
copyFileSync(source,target);
console.log(`MCP sidecar ready: ${target}`);
