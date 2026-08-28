import type {UpdateProgress} from "../types";

export const UPDATE_BUTTONS={
  idle:{text:"检查更新",disabled:false},
  checking:{text:"正在检查…",disabled:true},
  latest:{text:"已是最新版本",disabled:false}
} as const;

const size=(bytes:number)=>`${(bytes/1024/1024).toFixed(bytes>=10*1024*1024?1:2)} MB`;

export function updateProgressPresentation(progress:UpdateProgress){
  const title=progress.phase==="failed"?"更新下载失败":progress.phase==="launching"?"下载完成":progress.phase==="verifying"?"正在校验更新包…":"In Line 正在更新";
  const detail=progress.phase==="failed"?(progress.message||"请稍后重新下载"):progress.phase==="launching"?"正在启动安装程序…":progress.phase==="verifying"?"正在核对 SHA-256 完整性":progress.totalBytes?`${size(progress.downloadedBytes)} / ${size(progress.totalBytes)}${progress.percent!==null?` · ${progress.percent}%`:""}`:`已下载 ${size(progress.downloadedBytes)}`;
  return {title,detail,showProgress:progress.phase!=="failed",indeterminate:progress.percent===null};
}
