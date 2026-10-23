export interface PackageRecord {
  name: string;
  cliPhase?: number;
  build: Record<string, unknown>;
  sha256?: string;
  sourceSha256?: string;
  sourceTransformation?: string;
}
export function assertBuildSet(records: PackageRecord[], expectedCommit?: string): Record<string, unknown>;
export function assertNsisPayload(source: Uint8Array, payload: Uint8Array, name: string): void;
export function verifyDirectory(directory: string, options?: {
  expectedCommit?: string;
  companions?: string;
  target?: string;
  nsisSource?: string;
  output?: string;
}): Promise<{
  offline: boolean;
  build: Record<string, unknown>;
  records: PackageRecord[];
  companionsMatched: boolean;
  nsisPayloadMatched: boolean;
  pathModified: boolean;
  installed: boolean;
  currentMcpSessionVerified: boolean;
}>;
