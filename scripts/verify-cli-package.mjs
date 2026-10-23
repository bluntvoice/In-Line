// Offline package verification. Never launches the GUI or authenticates a profile.
import {execFile} from "node:child_process";
import {promisify} from "node:util";
import {readFile,writeFile,mkdir} from "node:fs/promises";
import {createHash} from "node:crypto";
import {resolve,join,dirname} from "node:path";
import {pathToFileURL} from "node:url";

export function assertBuildSet(records,expectedCommit){
  const names=["in-line","in-line-mcp","in-line-cli"];
  if(records.length!==3 || names.some(name=>records.filter(r=>r.name===name).length!==1))throw new Error("Three distinct package binaries required");
  const identity=records[0].build;
  const fields=["softwareVersion","mcpVersion","databaseSchemaVersion","baselineCommit","exactCommit","sourceDirty","commitVerification"];
  if(typeof identity?.softwareVersion!=="string" || !Number.isInteger(identity.mcpVersion) || !Number.isInteger(identity.databaseSchemaVersion) || !/^[0-9a-f]{40}$/i.test(identity.baselineCommit??"") || typeof identity.sourceDirty!=="boolean" || !["clean_checkout_readback","unverified_local_source"].includes(identity.commitVerification) || (identity.exactCommit!==null && !/^[0-9a-f]{40}$/i.test(identity.exactCommit??"")))throw new Error("Missing build identity");
  for(const record of records){
    if(fields.some(key=>record.build?.[key]!==identity[key]))throw new Error(`Mixed package builds: ${record.name}`);
    if(record.name==="in-line-cli" && record.cliPhase!==5)throw new Error("Unsupported CLI capability phase");
    if(expectedCommit && (record.build.exactCommit!==expectedCommit || record.build.baselineCommit!==expectedCommit || record.build.sourceDirty!==false || record.build.commitVerification!=="clean_checkout_readback"))throw new Error("Package is not the requested clean commit");
  }
  return identity;
}

// Tauri 2.11.4 patches this one marker before NSIS, then restores the source
// executable. Reconstruct the exact expected payload; never mask byte ranges.
export function assertNsisPayload(source,payload,name){
  if(!["in-line","in-line-mcp","in-line-cli"].includes(name))throw new Error("Unknown package binary");
  const expected=Buffer.from(source);
  if(name==="in-line"){
    const from=Buffer.from("__TAURI_BUNDLE_TYPE_VAR_UNK"),to=Buffer.from("__TAURI_BUNDLE_TYPE_VAR_NSS");
    const offset=expected.indexOf(from);
    if(offset<0 || expected.indexOf(from,offset+1)!==-1)throw new Error("Expected one Tauri bundle type marker");
    to.copy(expected,offset);
  }
  if(!expected.equals(payload))throw new Error(`Installer payload differs from expected NSIS binary: ${name}`);
}

export async function verifyDirectory(directory,{expectedCommit,companions,target,nsisSource,output}={}){
  const env=Object.fromEntries(Object.entries(process.env).filter(([key])=>!/^IN_LINE_MCP_/i.test(key)));
  const records=[];
  for(const name of ["in-line","in-line-mcp","in-line-cli"]){
    const path=join(resolve(directory),`${name}.exe`),bytes=await readFile(path);
    if(bytes.length<1024 || bytes.subarray(0,2).toString("ascii")!=="MZ")throw new Error(`Empty or invalid Windows binary: ${name}`);
    // An older GUI may ignore unknown flags and open its normal data store.
    // Refuse it before execution; every new offline probe embeds this marker.
    if(name!=="in-line-cli" && !bytes.includes(Buffer.from("offline-build-info","utf8")))throw new Error(`Offline metadata probe unsupported; legacy binary not launched: ${name}`);
    // Verify extracted bytes before executing even an offline metadata probe.
    const source=nsisSource?await readFile(join(resolve(nsisSource),`${name}.exe`)):null;
    if(source)assertNsisPayload(source,bytes,name);
    const args=name==="in-line-cli"?["--version","--format","json"]:["--build-info"];
    const {stdout,stderr}=await promisify(execFile)(path,args,{env,windowsHide:true,timeout:10000,maxBuffer:2*1024*1024,encoding:"utf8"});
    const result=JSON.parse(stdout);
    if(stderr || result.status!=="ok" || result.error!==null || result.apiVersion!==1 || result.data?.name!==name)throw new Error(`Invalid offline metadata: ${name}`);
    const record={name,bytes:bytes.length,sha256:createHash("sha256").update(bytes).digest("hex"),build:result.data.build};
    if(name==="in-line-cli")record.cliPhase=result.data.cliPhase;
    else if(result.data.offline!==true)throw new Error(`Offline flag missing: ${name}`);
    records.push(record);
    if(source){
      record.sourceSha256=createHash("sha256").update(source).digest("hex");
      record.sourceTransformation=name==="in-line"?"tauri_nsis_bundle_type_marker":"none";
    }
    if(companions && name!=="in-line"){
      if(!target)throw new Error("Companion target required");
      const staged=await readFile(join(resolve(companions),`${name}-${target}.exe`));
      if(!staged.equals(bytes))throw new Error(`Staged companion differs: ${name}`);
    }
  }
  const build=assertBuildSet(records,expectedCommit);
  const packageJson=JSON.parse(await readFile(new URL("../package.json",import.meta.url),"utf8"));
  const lock=JSON.parse(await readFile(new URL("../package-lock.json",import.meta.url),"utf8"));
  const config=JSON.parse(await readFile(new URL("../src-tauri/tauri.conf.json",import.meta.url),"utf8"));
  const cargo=await readFile(new URL("../src-tauri/Cargo.toml",import.meta.url),"utf8");
  const cargoLock=await readFile(new URL("../src-tauri/Cargo.lock",import.meta.url),"utf8");
  const cargoVersion=/^version\s*=\s*"([^"]+)"/m.exec(cargo)?.[1];
  const lockVersion=/\[\[package\]\]\s*\r?\nname = "in-line"\s*\r?\nversion = "([^"]+)"/.exec(cargoLock)?.[1];
  if([packageJson.version,lock.version,lock.packages?.[""]?.version,config.version,cargoVersion,lockVersion].some(v=>v!==build.softwareVersion))throw new Error("Version files differ from binaries");
  if(!["binaries/in-line-mcp","binaries/in-line-cli"].every(s=>config.bundle.externalBin.includes(s)))throw new Error("Missing packaged companion configuration");
  const report={offline:true,build,records,companionsMatched:!!companions,nsisPayloadMatched:!!nsisSource,pathModified:false,installed:false,currentMcpSessionVerified:false};
  if(output){await mkdir(dirname(resolve(output)),{recursive:true});await writeFile(output,JSON.stringify(report,null,2)+"\n");}
  return report;
}

if(process.argv[1] && import.meta.url===pathToFileURL(resolve(process.argv[1])).href){
  const args=process.argv.slice(2),directory=args.shift();if(!directory)throw new Error("Binary directory required");
  const options={};
  while(args.length){const key=args.shift(),value=args.shift();if(!value || options[key])throw new Error("Invalid or duplicate verifier option");options[key]=value;}
  if(Object.keys(options).some(key=>!["--expected-commit","--companions","--target","--nsis-source","--output"].includes(key)))throw new Error("Unknown verifier option");
  const report=await verifyDirectory(directory,{expectedCommit:options["--expected-commit"],companions:options["--companions"],target:options["--target"],nsisSource:options["--nsis-source"],output:options["--output"]});
  console.log(`PASS: three offline package identities/version files${report.companionsMatched?" and staged companions":""}${report.nsisPayloadMatched?" and exact expected NSIS payload":""} match; no store/IPC/PATH/installation operations.`);
}
