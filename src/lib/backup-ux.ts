import type { BackupInfo } from "../types";

export type BackupFailureStage="import"|"restore";

export interface BackupFailureGuidance{
  reason:string;
  recommendation:string;
}

export function prioritizeBackups(values:BackupInfo[],pinnedPath:string|null){
  const unique=[...new Map(values.map(value=>[value.path,value])).values()];
  if(!pinnedPath)return unique;
  return unique.sort((left,right)=>Number(right.path===pinnedPath)-Number(left.path===pinnedPath));
}

export function backupFailureGuidance(error:unknown,stage:BackupFailureStage):BackupFailureGuidance{
  const reason=(error instanceof Error?error.message:String(error||"未知错误")).replace(/^Error:\s*/i,"").trim()||"未知错误";
  const normalizedReason=reason.toLowerCase();
  const common=stage==="restore"?"备份文件已保留在列表中，当前数据没有被覆盖。":"当前数据没有被改变。";
  if(reason.includes("更高版本"))return{reason,recommendation:`${common} 请先将 In Line 升级到与备份相同或更高的正式版本，再重新导入。`};
  if(reason.includes("版本过旧"))return{reason,recommendation:`${common} 请先用生成该备份的旧版 In Line 打开并完成数据库升级，再重新备份。`};
  if(reason.includes("校验失败")||normalizedReason.includes("integrity")||normalizedReason.includes("database disk image is malformed"))return{reason:"备份数据库已损坏或完整性校验未通过",recommendation:`${common} 请改用同一台旧电脑上的另一份手动或自动备份；不要继续删除现有数据。`};
  if(reason.includes("不是有效")||reason.includes(".db 格式")||normalizedReason.includes("not a database"))return{reason:"所选文件不是有效的 In Line 数据库备份",recommendation:`${common} 请选择由 In Line 生成的完整 .db 备份，不要选择 -wal、-shm 或改名后的其他文件。`};
  if(reason.includes("找不到")||reason.includes("无法打开"))return{reason,recommendation:`${common} 请先把备份复制到本机普通文件夹，确认文件未被微信、网盘或其他程序占用后重试。`};
  if(reason.includes("正忙")||reason.includes("locked")||reason.includes("busy"))return{reason,recommendation:`${common} 请关闭其他 In Line 窗口及占用数据库的程序，等待几秒后点击“重试恢复”。`};
  if(reason.includes("只能恢复"))return{reason,recommendation:`${common} 请重新使用“导入并恢复”选择外部备份，不要直接移动列表中的文件。`};
  return{reason,recommendation:`${common} 建议先保留原备份并重新打开软件再试；如仍失败，请记录此原因并提供备份文件进行检查。`};
}
