import {describe,expect,it,vi} from "vitest";
import {createElement} from "react";
import {renderToStaticMarkup} from "react-dom/server";
import {ClientChoice,ClientPermissions} from "../components/McpSecuritySetting";
import {canPrepare,defaultPermissions,parseScope,prepareAndCopy,receiptExpired} from "./mcp-onboarding";
import type {McpReceipt} from "./mcp-onboarding";
describe("MCP safe onboarding",()=>{
  it("offers separately authorized CLI clients without claiming a verified MCP session",()=>{
    const cli={id:"codex-cli",label:"Codex（CLI，独立授权）",available:false,authorizationAvailable:true,status:"独立授权，导入后单独验证"};
    const absent={id:"powershell-cli",label:"PowerShell（CLI，独立授权）",available:false,authorizationAvailable:false,status:"未安装 CLI 程序"};
    expect(canPrepare(cli,false)).toBe(true);expect(canPrepare(absent,false)).toBe(false);
    const html=renderToStaticMarkup(createElement(ClientChoice,{presets:[cli,absent],value:"codex-cli",busy:false,onChange:()=>{}}));
    expect(html).toContain("独立授权");expect(html).toContain("导入后单独验证");expect(html).not.toContain("Token");
  });
  it("disables only CLI writes while allowing independently chosen read permissions",()=>{
    const render=(readOnly:boolean)=>renderToStaticMarkup(createElement(ClientPermissions,{value:defaultPermissions(),disabled:false,readOnly,onChange:()=>{}}));
    const cli=render(true),mcp=render(false);
    expect(cli.match(/disabled=""/g)).toHaveLength(1);expect(cli).toMatch(/disabled=""[^>]*\/>日常写入/);
    expect(mcp).not.toContain('disabled=""');expect(cli).toContain("完整读取");
  });
  it("keeps full read and write off and collapses blank scope to unrestricted",()=>{
    expect(defaultPermissions()).toEqual({regularRead:true,fullRead:false,write:false});
    expect(parseScope("  ")).toBeNull();
    expect(parseScope("A，B\n A,, ")).toEqual(["A","B"]);
  });
  it("does not open an unverified client or a busy action",()=>{
    const preset={id:"codex",label:"Codex",available:false,status:"自动接入验证中"};
    expect(canPrepare(preset,false)).toBe(false);expect(canPrepare(undefined,false)).toBe(false);
    expect(canPrepare({...preset,available:true},true)).toBe(false);
    expect(canPrepare({...preset,available:true},false)).toBe(true);
    const html=renderToStaticMarkup(createElement(ClientChoice,{presets:[preset],value:"codex",busy:false,onChange:()=>{}}));
    expect(html).toContain("Codex");expect(html).toContain("自动接入验证中");expect(html).not.toContain("Token");expect(html).not.toContain("input");
  });
  it("expires exactly at the boundary",()=>{
    const receipt={packageId:"package",clientId:"client",expiresAt:600,prompt:"safe"};
    expect(receiptExpired(receipt,599999)).toBe(false);expect(receiptExpired(receipt,600000)).toBe(true);
  });
  it("remembers one grant before clipboard failure and retries only copying",async()=>{
    const receipt={packageId:"package",clientId:"client",expiresAt:600,prompt:"safe command"};
    const prepare=vi.fn(async()=>receipt),copy=vi.fn(async()=>{throw new Error("clipboard unavailable");});
    let saved:McpReceipt|undefined;
    await expect(prepareAndCopy(prepare,value=>{saved=value;},copy)).rejects.toThrow("clipboard unavailable");
    expect(saved).toEqual(receipt);expect(prepare).toHaveBeenCalledTimes(1);
    const retry=vi.fn(async(_text:string)=>{});await retry(saved!.prompt);
    expect(prepare).toHaveBeenCalledTimes(1);expect(retry).toHaveBeenCalledWith("safe command");
  });
});
