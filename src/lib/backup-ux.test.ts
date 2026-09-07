import {describe,expect,it} from "vitest";
import type {BackupInfo} from "../types";
import {backupFailureGuidance,prioritizeBackups} from "./backup-ux";

const backup=(path:string):BackupInfo=>({name:path,path,size:1,modifiedAt:"2026-09-07T00:00:00Z"});

describe("backup ux",()=>{
  it("pins a newly imported backup without duplicating it",()=>{
    const values=prioritizeBackups([backup("old"),backup("import"),backup("import")],"import");
    expect(values.map(value=>value.path)).toEqual(["import","old"]);
  });

  it("recommends upgrading for a newer backup schema",()=>{
    const guidance=backupFailureGuidance("该备份来自更高版本的 In Line，请先升级软件","import");
    expect(guidance.reason).toContain("更高版本");
    expect(guidance.recommendation).toContain("升级");
  });

  it("keeps an imported backup available after a restore failure",()=>{
    const guidance=backupFailureGuidance(new Error("数据库正忙，请稍后重试"),"restore");
    expect(guidance.reason).toBe("数据库正忙，请稍后重试");
    expect(guidance.recommendation).toContain("备份文件已保留在列表中");
    expect(guidance.recommendation).toContain("重试恢复");
  });

  it("translates sqlite invalid-database errors into actionable guidance",()=>{
    const guidance=backupFailureGuidance("file is not a database","import");
    expect(guidance.reason).toBe("所选文件不是有效的 In Line 数据库备份");
    expect(guidance.recommendation).toContain("完整 .db 备份");
    expect(guidance.recommendation).toContain("-wal");
  });
});
