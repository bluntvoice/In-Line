import {describe,it,expect} from "vitest";
import {assertBuildSet,verifyDirectory,type PackageRecord} from "../scripts/verify-cli-package.mjs";
import {mkdtemp,writeFile,rm} from "node:fs/promises";
import {tmpdir} from "node:os";
import {join} from "node:path";

describe("three executable package gate",()=>{
  const sha="a".repeat(40);
  const records=():PackageRecord[]=>["in-line","in-line-mcp","in-line-cli"].map(name=>({name,cliPhase:5,build:{softwareVersion:"0.5.0",mcpVersion:1,databaseSchemaVersion:14,baselineCommit:sha,exactCommit:sha,sourceDirty:false,commitVerification:"clean_checkout_readback"}}));
  it("accepts one clean coherent build and refuses a different requested commit",()=>{
    expect(assertBuildSet(records(),sha).exactCommit).toBe(sha);
    expect(()=>assertBuildSet(records(),"b".repeat(40))).toThrow();
  });
  it("rejects absent and mixed binaries, versions, schemas and commits",()=>{
    expect(()=>assertBuildSet(records().slice(0,2),sha)).toThrow();
    const changes:Array<[string,unknown]>=[["softwareVersion","0.4.0"],["databaseSchemaVersion",13],["mcpVersion",0],["baselineCommit","b".repeat(40)],["exactCommit",null]];
    for(const [key,value] of changes){
      const rows=records();rows[1].build[key]=value;expect(()=>assertBuildSet(rows,sha)).toThrow();
    }
    const rows=records();rows[2].name="in-line-mcp";expect(()=>assertBuildSet(rows,sha)).toThrow();
  });
  it("does not relabel dirty local builds as the release commit",()=>{
    const rows=records();for(const row of rows){row.build.exactCommit=null;row.build.sourceDirty=true;row.build.commitVerification="unverified_local_source";}
    expect(assertBuildSet(rows).exactCommit).toBeNull();
    expect(()=>assertBuildSet(rows,sha)).toThrow();
  });
  it("rejects a legacy GUI before executing an unsupported metadata flag",async()=>{
    const root=await mkdtemp(join(tmpdir(),"inline-package-old-"));
    try{
      const bytes=Buffer.alloc(2048);bytes.write("MZ");
      await writeFile(join(root,"in-line.exe"),bytes);
      await expect(verifyDirectory(root)).rejects.toThrow("legacy binary not launched");
    }finally{await rm(root,{recursive:true,force:true});}
  });
});
