import {describe,expect,it} from "vitest";
import {canPrepare} from "./mcp-onboarding";

describe("first client authorization",()=>{
  it("allows a supported authorization adapter before current-session verification",()=>{
    expect(canPrepare({id:"codex",label:"Codex",available:false,authorizationAvailable:true,status:"当前会话待验证"},false)).toBe(true);
  });
  it("keeps unavailable adapters and busy operations disabled",()=>{
    const preset={id:"codex",label:"Codex",available:false,authorizationAvailable:true,status:"当前会话待验证"};
    expect(canPrepare(preset,true)).toBe(false);
    expect(canPrepare({...preset,authorizationAvailable:false},false)).toBe(false);
    expect(canPrepare(undefined,false)).toBe(false);
  });
  it("does not infer authorization support from verified availability when explicitly denied",()=>{
    expect(canPrepare({id:"codex",label:"Codex",available:true,authorizationAvailable:false,status:"不可授权"},false)).toBe(false);
  });
  it("keeps older unsupported presets disabled",()=>{
    expect(canPrepare({id:"codex",label:"Codex",available:false,status:"待验证"},false)).toBe(false);
  });
});
