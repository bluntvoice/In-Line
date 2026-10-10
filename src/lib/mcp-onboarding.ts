export type McpPermissions={regularRead:boolean;fullRead:boolean;write:boolean};
export type McpScope={departments:string[]|null;taskTypes:string[]|null};
export type McpClient={id:string;name:string;kind:string|null;revoked:boolean;permissions:McpPermissions;scope:McpScope};
export type McpClientPreset={id:string;label:string;available:boolean;status:string};
export type McpSecurityState={paused:boolean;groups:McpPermissions;revision:number;clients:McpClient[];onboardingClients:McpClientPreset[]};
export type McpReceipt={packageId:string;clientId:string;expiresAt:number;prompt:string};
export const defaultPermissions=():McpPermissions=>({regularRead:true,fullRead:false,write:false});
export const parseScope=(value:string):string[]|null=>value.trim()?Array.from(new Set(value.split(/[,，\n]/).map(x=>x.trim()).filter(Boolean))):null;
export const receiptExpired=(receipt:McpReceipt,now=Date.now()):boolean=>now>=receipt.expiresAt*1000;
export const canPrepare=(preset:McpClientPreset|undefined,busy:boolean):boolean=>Boolean(preset?.available)&&!busy;
// Store the receipt before touching the clipboard: a failed copy must reuse this grant.
export async function prepareAndCopy(prepare:()=>Promise<McpReceipt>,remember:(receipt:McpReceipt)=>void,copy:(text:string)=>Promise<void>):Promise<void>{
  const receipt=await prepare();remember(receipt);await copy(receipt.prompt);
}
