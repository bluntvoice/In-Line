export interface PackageRecord {
  name: string;
  cliPhase?: number;
  build: Record<string, unknown>;
}
export function assertBuildSet(records: PackageRecord[], expectedCommit?: string): Record<string, unknown>;
export function verifyDirectory(directory: string, options?: {
  expectedCommit?: string;
  companions?: string;
  target?: string;
  output?: string;
}): Promise<{
  offline: boolean;
  build: Record<string, unknown>;
  records: PackageRecord[];
  companionsMatched: boolean;
  pathModified: boolean;
  installed: boolean;
  currentMcpSessionVerified: boolean;
}>;
