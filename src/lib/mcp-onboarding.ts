export type McpPermissions={regularRead:boolean;fullRead:boolean;write:boolean};
export type McpScope={departments:string[]|null;taskTypes:string[]|null};
export type McpClient={id:string;name:string;kind:string|null;revoked:boolean;permissions:McpPermissions;scope:McpScope};
export type McpClientPreset={id:string;label:string;available:boolean;authorizationAvailable?:boolean;status:string};
export type McpSecurityState={paused:boolean;groups:McpPermissions;revision:number;sensitiveNoticeAcknowledged?:boolean;clients:McpClient[];onboardingClients:McpClientPreset[]};
export type McpReceipt={packageId:string;clientId:string;expiresAt:number;prompt:string};
export type McpStorageState={root:string;policy:string;ready:boolean;migrationRequired:boolean;canMigrate:boolean;legacyRoot:string|null;legacyPhysicalFile:string|null;legacyFileIdentity:string|null;legacyRedirected:boolean;errorCode:string|null};
type DiagnosticStore={revision:number;clientCount:number;selectedClient:{found:boolean;kind:string|null;revoked:boolean|null}|null};
type DiagnosticSnapshot={diagnosticOnly:true;configurationVerified:false;currentSessionVerified:false;processId:number;executable:string|null;build:unknown;selectedClientFingerprint:string|null;storage:{root:string;resolvedRoot:string|null;physicalFile:string|null;redirected:boolean|null;authorizationFileExists:boolean;fileIdentity:string|null;fileBytes:number|null;modifiedUnixMs:number|null};storageStableDuringRead:boolean;memory:DiagnosticStore|null;disk:DiagnosticStore|null;memoryMatchesDisk:boolean|null;diskErrorCode:string|null};
export type McpAuthorizationDiagnostic={diagnosticOnly:true;configurationVerified:false;currentSessionVerified:false;gui:DiagnosticSnapshot;importer:DiagnosticSnapshot|null;importerErrorCode:string|null;sameStorageFile:boolean|null;sameRevision:boolean|null;sameSelectedClient:boolean|null;sameBuild:boolean|null};
export const defaultPermissions=():McpPermissions=>({regularRead:true,fullRead:false,write:false});
export const parseScope=(value:string):string[]|null=>value.trim()?Array.from(new Set(value.split(/[,，\n]/).map(x=>x.trim()).filter(Boolean))):null;
export const receiptExpired=(receipt:McpReceipt,now=Date.now()):boolean=>now>=receipt.expiresAt*1000;
export const canPrepare=(preset:McpClientPreset|undefined,busy:boolean):boolean=>Boolean(preset?.authorizationAvailable??preset?.available)&&!busy;
// Store the receipt before touching the clipboard: a failed copy must reuse this grant.
export async function prepareAndCopy(prepare:()=>Promise<McpReceipt>,remember:(receipt:McpReceipt)=>void,copy:(text:string)=>Promise<void>):Promise<void>{
  const receipt=await prepare();remember(receipt);await copy(receipt.prompt);
}
