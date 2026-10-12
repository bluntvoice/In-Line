export type MaintenancePreview={nonce:string;content:string;source:string;action:string;format:string;expiresAt:number;suggestedName:string;prompt:string};
export type FaultEpisode={id:string;number:number;started:number;lastSeen:number;evidenceCount:number;firstErrorId:string;lastErrorId:string;state:string;mutedUntil:number|null;archived:boolean;build:unknown};
export type Fault={id:string;client:string|null;tool:string;module:string;code:string;severity:string;attention:string;episodes:FaultEpisode[]};
export type FaultState={items:Fault[];storageBytes?:number;capacityBytes?:number;storageError:boolean;unavailable?:boolean;latestClients:{client:string|null;at:number;tool:string;code:string;requestId:string}[]};
export function operationNotice(counts:Record<string,number>):string{
  const labels:Record<string,string>={write:"已执行",approval:"待审阅",completed:"后台完成",failed:"后台失败",cancelled:"后台取消",attention:"需要排查"};
  return "AI 操作："+Object.entries(counts).filter(([,n])=>n>0).map(([k,n])=>`${labels[k]??"操作"} ${n}`).join(" · ");
}
