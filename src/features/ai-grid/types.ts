import type { IconSummary } from "@/features/collections/types";
import type { SheetGridAnalysis, SheetGridSettings } from "@/features/sheets/types";

export type AiGridRequestScope = "grid_edit" | "single_generate" | "grid_generate";
export type AiGridWorkspaceStatus =
  | "draft"
  | "prepared"
  | "awaiting_result"
  | "layout_review_pending"
  | "completed"
  | "failed"
  | "cancelled"
  | "expired";

export interface AiGridRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface AiGridLayout {
  canvasWidth: number;
  canvasHeight: number;
  rows: number;
  columns: number;
  cellSize: number;
  gapX: number;
  gapY: number;
  borderLeft: number;
  borderTop: number;
  borderRight: number;
  borderBottom: number;
}

export interface AiGridArtifact {
  role: "input_sheet" | "output_sheet";
  sourceFileId: string;
  originalFilename: string;
  filePath: string;
  previewUrl: string;
  extension: string;
  mimeType: string;
  width: number;
  height: number;
  byteSize: number;
  sha256: string;
  hasAlpha: boolean;
  manifestJson: string;
  createdAt: string;
}

export interface AiGridWorkspaceItem {
  id: string;
  itemIndex: number;
  originIconId: string | null;
  originIconIdSnapshot: string | null;
  targetNameSnapshot: string;
  shape: "single";
  rowIndex: number;
  columnIndex: number;
  inputRect: AiGridRect;
  reviewStatus: "pending" | "included" | "excluded" | "candidate_created" | "icon_created";
  outputCandidateId: string | null;
  createdIconId: string | null;
}

export interface AiGridWorkspace {
  requestId: string;
  collectionId: string;
  requestScope: AiGridRequestScope;
  status: AiGridWorkspaceStatus;
  retryOfRequestId: string | null;
  layout: AiGridLayout;
  itemCount: number;
  candidateCount: number;
  createdIconCount: number;
  inputArtifact: AiGridArtifact | null;
  outputArtifact: AiGridArtifact | null;
  items: AiGridWorkspaceItem[];
  createdAt: string;
  updatedAt: string;
}

export interface ReviewedAiGridDecision {
  resultCellIndex: number;
  targetItemIndex: number;
  include: boolean;
  crop: AiGridRect | null;
}

export interface AiGridReviewCommitResult {
  commit: {
    requestId: string;
    candidateIds: string[];
    rejectedItemIndexes: number[];
    reviewSignature: string;
  };
  workspace: AiGridWorkspace;
}

export interface AiGeneratedIconsCommitResult {
  commit: {
    requestId: string;
    createdIcons: IconSummary[];
  };
  workspace: AiGridWorkspace;
}

export interface FinalizeGeneratedIconInput {
  itemIndex: number;
  displayName: string;
  altText: string;
}

export interface AiGridInputDragResult {
  started: boolean;
  nativeDragSupported: boolean;
  message: string;
}

export interface AiGridOutputReview {
  settings: SheetGridSettings;
  analysis: SheetGridAnalysis;
}