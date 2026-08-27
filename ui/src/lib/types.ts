export interface Volume {
  path: string;
  label: string;
  fileSystem: string;
  totalBytes: number;
  freeBytes: number;
  removable: boolean;
  identity: string;
}
export interface Evidence {
  source: string;
  detail: string;
}
export interface Assessment {
  category: string;
  owner: string | null;
  confidence: string;
  risk: string;
  purpose: string;
  consequence: string;
  recovery: string;
  recommendation: string;
  ruleId: string | null;
  protectedReason: string | null;
  evidence: Evidence[];
}
export interface FileRecord {
  id: number;
  path: string;
  parent: string;
  name: string;
  isDir: boolean;
  logicalBytes: number;
  allocatedBytes: number | null;
  modified: number;
  latestChange: number;
  accessed: number;
  created: number;
  identity: string | null;
  attributes: number;
  links: number;
  fileCount: number;
  issue: string | null;
  complete: boolean;
  hasBlockedChildren: boolean;
  assessment: Assessment;
}
export interface Scan {
  id: string;
  root: string;
  started: number;
  finished: number | null;
  status: string;
  files: number;
  directories: number;
  logicalBytes: number;
  allocatedBytes: number;
  issues: number;
  mode: string;
  message: string;
}
export interface LlmSettings {
  enabled: boolean;
  automatic: boolean;
  metadataConsent: boolean;
  baseUrl: string;
  model: string;
  format: string;
  tokenParameter: string;
  minimumBytes: number;
  maxRequests: number;
  concurrency: number;
  timeoutSeconds: number;
}
export interface Settings {
  scanRetention: number;
  enhancedScan: boolean;
  communityEnabled: boolean;
  protectedPaths: string[];
  ignoredPaths: string[];
  excludedLlmPaths: string[];
  labels: Record<string, string>;
  llm: LlmSettings;
}
export interface Progress {
  scanId?: string | null;
  active: boolean;
  queued: number;
  finished: number;
  requests: number;
  maxRequests: number;
  message: string;
}
export interface Bootstrap {
  scanLocations?: ScanLocation[];
  volumes: Volume[];
  scans: Scan[];
  settings: Settings;
  hasKey: boolean;
  accessPolicy: string;
  analysisProgress: Progress;
}
export interface RuntimeStatus {
  scan: Scan;
  scans: Scan[];
  analysisProgress: Progress;
}
export interface EntryPage {
  items: FileRecord[];
  total: number;
}
export interface ScanLocation {
  id: string;
  name: string;
  path: string;
  description: string;
}
export interface SuggestionGroup {
  id: string;
  name: string;
  purpose: string;
  consequence: string;
  count: number;
  occupiedBytes: number;
  estimated: boolean;
  recognized: boolean;
}
export interface SuggestionPage {
  groups: SuggestionGroup[];
  items: FileRecord[];
  total: number;
}
export interface UnitComponent {
  entryId: number;
  path: string;
  role: string;
  evidence: string;
  logicalBytes: number;
  occupiedBytes: number;
  fileCount: number;
  protected: boolean;
}
export interface ApplicationUnit {
  id: string;
  name: string;
  kind: string;
  confidence: string;
  logicalBytes: number;
  occupiedBytes: number;
  fileCount: number;
  estimated: boolean;
  complete: boolean;
  components: UnitComponent[];
  children: ApplicationUnit[];
}
export interface ApplicationUnitPage {
  items: ApplicationUnit[];
  total: number;
  applications: number;
  uncertain: number;
  occupiedBytes: number;
  estimated: boolean;
}
export interface Group {
  name: string;
  bytes: number;
  count: number;
}
export interface CleanupItem {
  entryId: number;
  path: string;
  bytes: number;
  fingerprint: string;
  risk: string;
  allowed: boolean;
  reason: string;
}
export interface CleanupPreview {
  id: string;
  scanId: string;
  created: number;
  items: CleanupItem[];
  policyFingerprint: string;
  pendingBytes: number;
  requiresExtraConfirmation: boolean;
}
export interface HistoryItem {
  id: string;
  batchId: string;
  path: string;
  bytes: number;
  time: number;
  status: string;
  message: string;
  freeSpaceDelta: number;
}
export interface ContextFile {
  entryId: number;
  name: string;
  bytes: number;
  isDir: boolean;
  modified: number;
  sampleAllowed: boolean;
}
export interface AnalysisContext {
  scanId: string;
  entryId: number;
  fingerprint: string;
  path: string;
  logicalBytes: number;
  fileCount: number;
  modified: number;
  accessed: number;
  evidence: Evidence[];
  files: ContextFile[];
  truncated: boolean;
  note: string;
}
export interface ModelAssessment {
  purpose: string;
  source: string;
  consequences: string;
  recovery: string;
  recommendation: string;
  confidence: string;
  uncertainties: string[];
  evidence: string[];
  questions: string[];
}
export interface AnalysisResult {
  id: string;
  entryId: number;
  status: string;
  message: string;
  assessment: ModelAssessment | null;
  promptTokens: number | null;
  completionTokens: number | null;
  includedContent: boolean;
  created: number;
}
export interface Rule {
  id: string;
  name: string;
  root: string;
  category: string;
  owner: string;
  ageDays: number;
  purpose: string;
  consequence: string;
  recovery: string;
  warning: string;
  community: boolean;
}
