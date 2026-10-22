import { copyFileSync, existsSync, mkdirSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root=resolve(dirname(fileURLToPath(import.meta.url)),"..");
const manifest=join(root,"src-tauri","Cargo.toml");
const triple=execFileSync("rustc",["--print","host-tuple"],{encoding:"utf8"}).trim();
const args=process.argv.slice(2);
if(args.length>1 || (args.length===1 && args[0]!=="--stage-only"))throw new Error("Unknown companion preparation arguments");
const stageOnly=args[0]==="--stage-only";

// Match the dependency feature enabled by `tauri build`; otherwise its final
// all-binary build overwrites these outputs with a different release variant.
if(!stageOnly)execFileSync("cargo",["build","--locked","--manifest-path",manifest,"--release","--features","tauri/custom-protocol","--bin","in-line-mcp","--bin","in-line-cli"],{
  cwd:root,
  stdio:"inherit"
});

const metadata=JSON.parse(execFileSync("cargo",["metadata","--no-deps","--format-version","1","--manifest-path",manifest],{
  cwd:root,
  encoding:"utf8"
}));
const extension=process.platform==="win32"?".exe":"";
for(const name of ["in-line-mcp","in-line-cli"]){
  const source=join(metadata.target_directory,"release",`${name}${extension}`);
  const target=join(root,"src-tauri","binaries",`${name}-${triple}${extension}`);
  if(!existsSync(source))throw new Error(`未找到构建产物：${source}`);
  const bytes=readFileSync(source);
  if(bytes.length<1024 || (process.platform==="win32" && bytes.subarray(0,2).toString("ascii")!=="MZ"))throw new Error(`构建产物为空或不是Windows PE：${name}`);
  mkdirSync(dirname(target),{recursive:true});
  copyFileSync(source,target);
  console.log(`Bundled companion ready: ${target}`);
}
