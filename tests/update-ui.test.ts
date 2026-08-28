import {describe,expect,it} from "vitest";
import {UPDATE_BUTTONS,updateProgressPresentation} from "../src/lib/update-ui";
import type {UpdateProgress} from "../src/types";

const progress=(value:Partial<UpdateProgress>):UpdateProgress=>({
  phase:"downloading",version:"0.2.3",downloadedBytes:0,totalBytes:null,percent:null,message:null,...value
});

describe("update UI presentation",()=>{
  it("disables the button only while checking and exposes the latest state",()=>{
    expect(UPDATE_BUTTONS.checking).toEqual({text:"正在检查…",disabled:true});
    expect(UPDATE_BUTTONS.latest.text).toBe("已是最新版本");
    expect(UPDATE_BUTTONS.idle).toEqual({text:"检查更新",disabled:false});
  });

  it("shows real progress when total size exists and indeterminate progress otherwise",()=>{
    expect(updateProgressPresentation(progress({downloadedBytes:5*1024*1024,totalBytes:10*1024*1024,percent:50}))).toMatchObject({detail:"5.00 MB / 10.0 MB · 50%",indeterminate:false});
    expect(updateProgressPresentation(progress({downloadedBytes:3*1024*1024}))).toMatchObject({detail:"已下载 3.00 MB",indeterminate:true});
  });

  it("presents verification, installer launch and download failures distinctly",()=>{
    expect(updateProgressPresentation(progress({phase:"verifying"})).title).toBe("正在校验更新包…");
    expect(updateProgressPresentation(progress({phase:"launching"})).detail).toBe("正在启动安装程序…");
    expect(updateProgressPresentation(progress({phase:"failed",message:"更新包校验失败，请重新下载"}))).toMatchObject({title:"更新下载失败",detail:"更新包校验失败，请重新下载",showProgress:false});
  });
});
