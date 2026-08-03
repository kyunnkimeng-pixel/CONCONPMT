export type AiWebHandoffPayloadState =
  | "available"
  | "cleanup_pending"
  | "deleted"
  | "expired"
  | "closed";

export interface AiWebHandoffHistoryItem {
  requestId: string;
  requestScope: string;
  handoffKind: string;
  collectionId: string | null;
  iconId: string | null;
  collectionName: string | null;
  iconName: string | null;
  serviceSurface: string;
  requestStatus: string;
  payloadState: AiWebHandoffPayloadState;
  hasResult: boolean;
  createdAt: string;
  expiresAt: string;
  resultReceivedAt: string | null;
  cleanupRequestedAt: string | null;
  payloadDeletedAt: string | null;
}

export interface AiWebHandoffStorageStatus {
  quotaBytes: number;
  usedBytes: number;
  availableBytes: number;
  retainedHistoryCount: number;
  livePayloadCount: number;
  cleanupPendingCount: number;
  quotaReached: boolean;
}

export interface AiWebHandoffMaintenanceReport {
  removedCount: number;
  deferredCount: number;
  storage: AiWebHandoffStorageStatus;
}