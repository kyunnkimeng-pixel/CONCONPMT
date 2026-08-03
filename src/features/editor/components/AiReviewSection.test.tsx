import { renderToString } from "react-dom/server";
import { describe, expect, it } from "vitest";

import {
  AiCandidateActionButtons,
  AiCandidateCompareStage,
  AiCreatedIconRefreshWarning,
  AiEditorRefreshWarning,
  AiImportResultPanel,
  AiMutationOutcomeDialog,
  AiNormalizationControls,
  AiNormalizationPreviewComparison,
  AiReviewSection,
  AiSourceSummary,
  AiUnavailableImagePlaceholder,
  AiVersionRecipeDetails,
  AiWorkspaceDialog,
  deriveAiSummaryRestoreLockReason,
} from "@/features/editor/components/AiReviewSection";
import type {
  AiCandidate,
  AiNormalizationPreview,
  AiVersion,
  EffectiveVisualSource,
  IconEditorState,
  SourceFileSummary,
} from "@/features/editor/types";
import type {
  CollectionSummary,
  IconSummary,
} from "@/features/collections/types";

const source: SourceFileSummary = {
  id: "source_original",
  originalFilename: "original.png",
  originalImageUrl: "asset://original.png",
  originalExtension: "png",
  mimeType: "image/png",
  sha256: "a".repeat(64),
  hasAlpha: true,
  width: 200,
  height: 200,
  byteSize: 12_345,
  isAnimated: false,
  frameCount: null,
  originalLoopMode: "preserve",
  originalLoopCount: null,
};

const visualSource: EffectiveVisualSource = {
  originalSource: source,
  effectiveRenderSource: source,
  originalLineageId: "lineage_1",
  originalLineageGeneration: 0,
  activeVersionId: null,
  activeCandidateId: null,
  activationRevision: 0,
  normalizationRecipeHash: null,
};

const collection = {
  id: "collection_1",
  name: "테스트 모음",
  defaultCellWidth: 200,
  defaultCellHeight: 200,
} as CollectionSummary;

const workspaceIcon = {
  id: "icon_1",
  collectionId: collection.id,
  displayName: "테스트 아이콘",
} as IconSummary;

const createdIcon = {
  id: "icon_new",
  collectionId: "collection_1",
  displayName: "AI 후보 아이콘",
} as IconSummary;

const editorState = {
  icon: createdIcon,
  source,
  visualSource,
} as IconEditorState;

const candidate: AiCandidate = {
  id: "candidate_1",
  requestId: "request_1",
  candidateIndex: 0,
  serviceSurface: "gemini_web",
  source,
  createdAt: "2026-07-27T10:00:00Z",
  isMaterialized: false,

  isStale: false,
  staleReason: null,
  isAvailable: true,
  unavailableReason: null,
  createdIconUsage: {
    createdIconCount: 0,
    latestCreatedIcon: null,
  },
};
const allowedCompatibility = {
  allowed: true,
  reasonCode: null,
  reason: null,
};

const blockedCurrentCompatibility = {
  allowed: false,
  reasonCode: "candidate_stale",
  reason: "현재 편집 상태와 맞지 않습니다.",
};

const normalizationPreview: AiNormalizationPreview = {
  candidateId: candidate.id,
  rawSource: {
    ...source,
    id: "source_raw",
    originalFilename: "result.png",
    originalImageUrl: "asset://result.png",
    width: 1024,
    height: 768,
    hasAlpha: false,
  },
  normalizedPreviewPath: "asset://normalized.png",
  finalPreviewPath: "asset://final.png",
  targetCanvasWidth: 640,
  targetCanvasHeight: 640,
  finalRenderWidth: 1280,
  finalRenderHeight: 640,
  pieceWidth: 640,
  pieceHeight: 640,
  normalizationRecipeHash: "recipe_hash",
  previewSignature: "preview_signature",
  nativeRecipeSignature: "native_recipe_signature",
  geometry: {
    kind: "contain_pad",
    resizedWidth: 640,
    resizedHeight: 480,
    cropX: 0,
    cropY: 0,
    pasteX: 0,
    pasteY: 80,
  },
  normalizedHasAlpha: true,
  currentIconCompatibility: allowedCompatibility,
  newIconCompatibility: allowedCompatibility,
  warnings: [],
  existingVersionId: null,
  isCurrentRecipe: false,
};

describe("AiReviewSection compact entry", () => {
  it("keeps the workspace closed and exposes no heavy or credential controls", () => {
    const html = renderToString(
      <AiReviewSection
        collection={collection}
        hasUnsavedChanges={false}
        icon={workspaceIcon}
        visualSource={visualSource}
        onBusyChange={() => {}}
        onCreatedIconCommitted={async () => {}}
        onEditorStateCommitted={async () => {}}
        onRevealIcon={() => true}
      />,
    );

    expect(html).toContain('data-testid="ai-source-summary"');
    expect(html).toContain("원본 사용 중");
    expect(html).toContain("AI로 수정");
    expect(html).not.toContain('data-testid="ai-workspace-dialog"');
    expect(html).not.toContain('data-testid="ai-workspace-tabs"');
    expect(html).not.toContain('data-testid="ai-candidate-rail"');
    expect(html).not.toContain('data-testid="ai-candidate-file"');
    expect(html).not.toContain('type="password"');
    expect(html).not.toContain("API 키");
    expect(html).not.toContain("토큰");
    expect(html).not.toContain("AI 이미지 생성");
  });

  it("shows active AI source state and a lock-aware original restore action", () => {
    const activeVisualSource: EffectiveVisualSource = {
      ...visualSource,
      effectiveRenderSource: {
        ...source,
        id: "source_ai",
        originalFilename: "ai-result.png",
      },
      activeVersionId: "version_1",
      activeCandidateId: candidate.id,
      activationRevision: 1,
      normalizationRecipeHash: "recipe_hash",
    };
    const html = renderToString(
      <AiSourceSummary
        busy={false}
        iconName="테스트 아이콘"
        isLoading={false}
        mutationLockReason="크롭·변형 또는 고급 편집 변경을 먼저 적용하거나 되돌려 주세요."
        visualSource={activeVisualSource}
        onOpen={() => {}}
        onRestoreOriginal={() => {}}
      />,
    );

    expect(html).toContain("AI 소스 사용 중");
    expect(html).toContain("AI 작업공간 열기");
    expect(html).toContain("원본은 보존되어 있습니다.");
    expect(html).toContain("원본으로 돌아가기");
    expect(html).toContain(
      "크롭·변형 또는 고급 편집 변경을 먼저 적용하거나 되돌려 주세요.",
    );
    expect(openingButton(html, "ai-summary-restore-original")).toContain(
      ' disabled=""',
    );
    expect(openingButton(html, "ai-summary-restore-original")).toContain(
      'aria-describedby="ai-summary-restore-reason"',
    );
    expect(html).toContain('id="ai-summary-restore-reason"');
    expect(html).not.toContain('data-testid="ai-workspace-body"');
  });
});

describe("deriveAiSummaryRestoreLockReason", () => {
  it("fails closed with the load error after review state loading ends", () => {
    const reason = deriveAiSummaryRestoreLockReason({
      actionLockReason: null,
      errorMessage: "AI 소스 이력을 읽지 못했습니다.",
      hasReviewState: false,
      isLoading: false,
    });
    const activeVisualSource: EffectiveVisualSource = {
      ...visualSource,
      activeVersionId: "version_1",
      activeCandidateId: candidate.id,
      activationRevision: 1,
    };
    const html = renderToString(
      <AiSourceSummary
        busy={false}
        iconName="테스트 아이콘"
        isLoading={false}
        mutationLockReason={reason}
        visualSource={activeVisualSource}
        onOpen={() => {}}
        onRestoreOriginal={() => {}}
      />,
    );

    expect(reason).toBe("AI 소스 이력을 읽지 못했습니다.");
    expect(openingButton(html, "ai-summary-restore-original")).toContain(
      ' disabled=""',
    );
    expect(openingButton(html, "ai-summary-restore-original")).toContain(
      'aria-describedby="ai-summary-restore-reason"',
    );
    expect(html).toContain('id="ai-summary-restore-reason"');
  });

  it("prioritizes draft locks and leaves loading or healthy state unlocked", () => {
    expect(
      deriveAiSummaryRestoreLockReason({
        actionLockReason: "먼저 편집 변경을 적용해 주세요.",
        errorMessage: "이력 오류",
        hasReviewState: false,
        isLoading: false,
      }),
    ).toBe("먼저 편집 변경을 적용해 주세요.");
    expect(
      deriveAiSummaryRestoreLockReason({
        actionLockReason: null,
        errorMessage: null,
        hasReviewState: false,
        isLoading: true,
      }),
    ).toBeNull();
    expect(
      deriveAiSummaryRestoreLockReason({
        actionLockReason: null,
        errorMessage: "후속 비동기 오류",
        hasReviewState: true,
        isLoading: false,
      }),
    ).toBeNull();
  });
});
describe("AiWorkspaceDialog", () => {
  const renderWorkspace = (
    layoutMode: "wide" | "narrow",
    activeView: "import" | "review" | "history" = "review",
  ) =>
    renderToString(
      <AiWorkspaceDialog
        activeSourceLabel="원본 사용 중"
        activeView={activeView}
        announcement={<p>후보를 검토할 수 있습니다.</p>}
        announcementTone="status"
        busy={false}
        footer={<p>고정 작업 영역</p>}
        iconName="테스트 아이콘"
        layoutMode={layoutMode}
        onClose={() => {}}
        onViewChange={() => {}}
      >
        <div data-testid="workspace-test-content">본문</div>
      </AiWorkspaceDialog>,
    );

  it("renders an accessible open dialog with exactly three connected tabs and five fixed regions", () => {
    const html = renderWorkspace("wide");

    expect(html).toContain('role="dialog"');
    expect(html).toContain('aria-modal="true"');
    expect(html).toContain('aria-labelledby="ai-workspace-title"');
    expect(html).toContain('aria-describedby="ai-workspace-description"');
    expect(html.match(/role="tab"/g)).toHaveLength(3);
    expect(html.match(/aria-controls="ai-workspace-panel"/g)).toHaveLength(3);
    expect(html).toContain('role="tabpanel"');
    expect(html).toContain('id="ai-workspace-panel"');
    expect(html).toContain('aria-labelledby="ai-workspace-tab-review"');
    expect(html.match(/role="(?:status|alert)"/g)).toHaveLength(1);
    expect(html).toContain('role="status"');
    expect(html).toContain("AI 수정·가져오기");
    expect(html).toContain("후보 검토");
    expect(html).toContain("소스 이력");

    for (const region of [
      "header",
      "tabs",
      "body",
      "announcement",
      "footer",
    ]) {
      expect(html).toContain(`data-testid="ai-workspace-${region}"`);
    }
  });

  it("exposes wide and narrow layout hooks without provider generation or secret controls", () => {
    const wideHtml = renderWorkspace("wide", "import");
    const narrowHtml = renderWorkspace("narrow", "history");

    expect(wideHtml).toContain('data-layout="wide"');
    expect(narrowHtml).toContain('data-layout="narrow"');
    expect(wideHtml).toContain('aria-labelledby="ai-workspace-tab-import"');
    expect(narrowHtml).toContain(
      'aria-labelledby="ai-workspace-tab-history"',
    );
    for (const html of [wideHtml, narrowHtml]) {
      expect(html).not.toContain('type="password"');
      expect(html).not.toContain("API 키");
      expect(html).not.toContain("토큰");
      expect(html).not.toContain("AI 이미지 생성");
      expect(html).not.toContain("프롬프트 입력");
    }
  });
});

describe("AiCandidateCompareStage", () => {
  it("offers all five comparison views, fit/100% zoom, and checkerboard controls", () => {
    const html = renderToString(
      <AiCandidateCompareStage
        checkerboardEnabled
        compareView="overlay"
        compareZoom="actual"
        normalizationPreview={normalizationPreview}
        selectedCandidate={candidate}
        visualSource={visualSource}
        warnings={[]}
        onCheckerboardChange={() => {}}
        onCompareViewChange={() => {}}
        onCompareZoomChange={() => {}}
      />,
    );

    for (const view of [
      "original",
      "raw",
      "normalized",
      "final",
      "overlay",
    ]) {
      expect(html).toContain(`data-testid="ai-compare-view-${view}"`);
    }
    expect(html.match(/data-testid="ai-compare-view-/g)).toHaveLength(5);
    expect(html).toContain('data-testid="ai-compare-zoom-fit"');
    expect(html).toContain('data-testid="ai-compare-zoom-actual"');
    expect(html).toContain('data-testid="ai-compare-checkerboard"');
    expect(html).toContain("화면 맞춤");
    expect(html).toContain("100%");
    expect(html).toContain("체커보드 배경");
    expect(html).toContain('data-testid="ai-compare-overlay"');
    expect(html).toContain("background-image:");
  });
});

function openingButton(html: string, testId: string) {
  const match = html.match(
    new RegExp(`<button[^>]*data-testid="${testId}"[^>]*>`),
  );
  expect(match).not.toBeNull();
  return match?.[0] ?? "";
}

describe("AiCandidateActionButtons", () => {
  it("makes new icon creation the ready candidate's primary action", () => {
    const html = renderToString(
      <AiCandidateActionButtons
        actionLockReason={null}
        candidate={candidate}
        currentCompatibility={allowedCompatibility}
        disabled={false}
        isCurrentRecipe={false}
        isActivating={false}
        isCreating={false}
        newIconCompatibility={allowedCompatibility}
        previewReady
        onActivate={() => {}}
        onCreate={() => {}}
      />,
    );

    expect(html).toContain("새 아이콘으로 추가 · 권장");
   expect(html).toContain("현재 아이콘에 사용");
    expect(openingButton(html, "ai-create-icon-candidate_1")).not.toContain(
      ' disabled=""',
    );
    expect(openingButton(html, "ai-activate-current-candidate_1")).not.toContain(
      ' disabled=""',
    );
  });

  it("allows a stale candidate to become a new icon but blocks current-icon apply", () => {
    const html = renderToString(
      <AiCandidateActionButtons
        actionLockReason={null}
        candidate={{
          ...candidate,
          isStale: true,
          staleReason: "현재 편집 상태와 맞지 않습니다.",
        }}
        disabled={false}
        currentCompatibility={blockedCurrentCompatibility}
        isCurrentRecipe={false}
        isActivating={false}
        isCreating={false}
        onActivate={() => {}}
        newIconCompatibility={allowedCompatibility}
        previewReady
        onCreate={() => {}}
      />,
    );

    expect(openingButton(html, "ai-create-icon-candidate_1")).not.toContain(
      ' disabled=""',
    );
    expect(openingButton(html, "ai-activate-current-candidate_1")).toContain(
      ' disabled=""',
    );
    expect(html).toContain("현재 편집 상태와 맞지 않습니다.");
  });

  it("shows create progress and locks both actions while one mutation is busy", () => {
    const html = renderToString(
      <AiCandidateActionButtons
        actionLockReason={null}
        candidate={candidate}
        disabled
        isCurrentRecipe={false}
        currentCompatibility={allowedCompatibility}
        isActivating={false}
        isCreating
        onActivate={() => {}}
        onCreate={() => {}}
        newIconCompatibility={allowedCompatibility}
        previewReady
      />,
    );

    const createButton = openingButton(html, "ai-create-icon-candidate_1");
    expect(createButton).toContain('aria-busy="true"');
    expect(createButton).toContain(' disabled=""');
    expect(openingButton(html, "ai-activate-current-candidate_1")).toContain(
      ' disabled=""',
    );
    expect(html).toContain("새 아이콘으로 추가하는 중");
  });
  it("allows another recipe for a previously materialized candidate", () => {
    const html = renderToString(
      <AiCandidateActionButtons
        actionLockReason={null}
        candidate={{ ...candidate, isMaterialized: true }}
        currentCompatibility={allowedCompatibility}
        disabled={false}
        isCurrentRecipe={false}
        isActivating={false}
        isCreating={false}
        newIconCompatibility={allowedCompatibility}
        previewReady
        onActivate={() => {}}
        onCreate={() => {}}
      />,
    );

    expect(openingButton(html, "ai-activate-current-candidate_1")).not.toContain(
      ' disabled=""',
    );
    expect(html).toContain("현재 아이콘에 사용");
  });

  it("keeps both mutations disabled until a matching preview exists", () => {

    const html = renderToString(
      <AiCandidateActionButtons
        actionLockReason={null}
        candidate={candidate}
        currentCompatibility={null}
        disabled={false}
        isCurrentRecipe={false}
        isActivating={false}
        isCreating={false}
        newIconCompatibility={null}
        previewReady={false}
        onActivate={() => {}}
        onCreate={() => {}}
      />,
    );

    expect(openingButton(html, "ai-create-icon-candidate_1")).toContain(
      ' disabled=""',
    );
    expect(openingButton(html, "ai-activate-current-candidate_1")).toContain(
      ' disabled=""',
    );
    expect(html).toContain("규격화 미리보기를 만들어 확인해 주세요");
  });

  it("uses each backend compatibility result independently", () => {
    const html = renderToString(
      <AiCandidateActionButtons
        actionLockReason={null}
        candidate={candidate}
        currentCompatibility={allowedCompatibility}
        disabled={false}
        isCurrentRecipe={false}
        isActivating={false}
        isCreating={false}
        newIconCompatibility={{
          allowed: false,
          reasonCode: "new_icon_blocked",
          reason: "이 후보는 새 아이콘으로 복제할 수 없습니다.",
        }}
        previewReady
        onActivate={() => {}}
        onCreate={() => {}}
      />,
    );

    expect(openingButton(html, "ai-create-icon-candidate_1")).toContain(
      ' disabled=""',
    );
    expect(openingButton(html, "ai-activate-current-candidate_1")).not.toContain(
      ' disabled=""',
    );
    expect(html).toContain("이 후보는 새 아이콘으로 복제할 수 없습니다.");
  });

  it("shows usage counts for 0, 1, and N created icons", () => {
    const zeroHtml = renderToString(
      <AiCandidateActionButtons
        actionLockReason={null}
        candidate={candidate}
        currentCompatibility={allowedCompatibility}
        disabled={false}
        isCurrentRecipe={false}
        isActivating={false}
        isCreating={false}
        newIconCompatibility={allowedCompatibility}
        previewReady
        onActivate={() => {}}
        onCreate={() => {}}
        onRevealLatestCreatedIcon={() => true}
      />,
    );
    expect(zeroHtml).toContain("새 아이콘으로 추가 · 권장");
    expect(zeroHtml).not.toContain("이 후보로 만든 아이콘");

    for (const count of [1, 4]) {
      const html = renderToString(
        <AiCandidateActionButtons
          actionLockReason={null}
          candidate={{
            ...candidate,
            createdIconUsage: {
              createdIconCount: count,
              latestCreatedIcon: createdIcon,
            },
          }}
          currentCompatibility={allowedCompatibility}
          disabled={false}
          isCurrentRecipe={false}
          isActivating={false}
          isCreating={false}
          newIconCompatibility={allowedCompatibility}
          previewReady
          onActivate={() => {}}
          onCreate={() => {}}
          onRevealLatestCreatedIcon={() => true}
        />,
      );
      expect(html).toContain("이 후보로 하나 더 추가");
      expect(html).toContain(`이 후보로 만든 아이콘 ${count}개`);
      expect(html).toContain("최근 만든 아이콘 보기");
    }
  });

  it("locks the latest-created reveal while mutations are locked", () => {
    const html = renderToString(
      <AiCandidateActionButtons
        actionLockReason="저장하지 않은 편집 변경을 먼저 적용하거나 되돌리세요."
        candidate={{
          ...candidate,
          createdIconUsage: {
            createdIconCount: 1,
            latestCreatedIcon: createdIcon,
          },
        }}
        currentCompatibility={allowedCompatibility}
        disabled
        isCurrentRecipe={false}
        isActivating={false}
        isCreating={false}
        newIconCompatibility={allowedCompatibility}
        previewReady
        onActivate={() => {}}
        onCreate={() => {}}
        onRevealLatestCreatedIcon={() => true}
      />,
    );

    const revealButton = openingButton(
      html,
      "ai-reveal-latest-created-candidate_1",
    );
    expect(revealButton).toContain('disabled=""');
    expect(revealButton).toContain(
      'aria-describedby="ai-create-reason-candidate_1"',
    );
  });
});

describe("AiNormalizationControls", () => {
  it("shows Korean mode, 3x3 alignment, filter, target, and explicit preview controls", () => {
    const html = renderToString(
      <AiNormalizationControls
        disabled={false}
        isPreviewing={false}
        options={{
          mode: "contain_pad",
          alignment: "center",
          resizeFilter: "lanczos3",
          padRgba: [0, 0, 0, 0],
        }}
        status={{
          code: "needs_preview",
          tone: "neutral",
          label: "규격화 미리보기가 필요합니다",
          message: "설정을 확인해 주세요.",
          canCommit: false,
        }}
        targetCanvasHeight={640}
        targetCanvasWidth={640}
        onOptionsChange={() => {}}
        onPreview={() => {}}
      />,
    );

    expect(html).toContain("전체 보이기 · 권장");
    expect(html).toContain("빈틈 없이 채우기");
    expect(html.match(/data-testid="ai-normalization-align-/g)).toHaveLength(9);
    expect(html).toContain("부드럽게 · 일반 그림");
    expect(html).toContain("픽셀 유지 · 픽셀 아트");
    expect(html).toContain("대상 캔버스: 640×640px");
    expect(html).toContain("여백: 투명");
    expect(html).toContain("규격화 미리보기");
    expect(openingButton(html, "ai-preview-normalization")).toContain(
      'aria-describedby="ai-normalization-status"',
    );
    expect(html).toContain('id="ai-normalization-status"');
    expect(html).not.toContain('aria-live=');
  });
});

describe("AiNormalizationPreviewComparison", () => {
  it("shows raw, normalized, and final renders on checkerboards with metadata", () => {
    const html = renderToString(
      <AiNormalizationPreviewComparison
        preview={normalizationPreview}
        warnings={[
          {
            code: "contain_padding",
            severity: "info",
            message: "위·아래에 투명 여백이 생깁니다.",
          },
        ]}
      />,
    );

    expect(html).toContain("AI 원본 · raw");
    expect(html).toContain("규격화 캔버스");
    expect(html).toContain("최종 편집기 렌더");
    expect(html).toContain("1024×768px · 불투명");
    expect(html).toContain("640×640px · 투명 영역 있음");
    expect(html).toContain("1280×640px · 편집 효과 포함");
    expect(html).toContain(
      "최종 렌더: 1280×640px · 조각 규격: 640×640px · 2조각",
    );
    expect(html).toContain("리사이즈 640×480px · 배치 위치 0, 80");
    expect(html).toContain("위·아래에 투명 여백이 생깁니다.");
    expect(html.match(/background-image:/g)).toHaveLength(3);
  });
});


describe("AI mutation outcomes", () => {
  const sharedProps = {
    busy: false,
    onClose: () => {},
    onContinueComparing: () => {},
    onExternalHandoffComplete: () => {},
    onOpenCreatedIcon: () => true,
    onRestoreOriginal: () => {},
    onRetrySync: () => {},
    onReturnToEditor: () => {},
    onShowHistory: () => {},
  };

  it("renders the create outcome with three next actions and primary autofocus", () => {
    const html = renderToString(
      <AiMutationOutcomeDialog
        {...sharedProps}
        outcome={{
          kind: "create",
          candidateId: candidate.id,
          createdIcon,
          createdIconUsage: {
            createdIconCount: 1,
            latestCreatedIcon: createdIcon,
          },
          syncError: null,
        }}
      />,
    );

    expect(html).toContain("새 아이콘을 추가했습니다.");
    expect(html).toContain("작업 중 상태");
    expect(html).toContain("alt 값");
    expect(html).toContain("새 아이콘 열기");
    expect(html).toContain("목록에서 보기");
    expect(html).toContain("계속 후보 비교");
    expect(openingButton(html, "ai-outcome-open-created-icon")).toContain(
      'autofocus=""',
    );
    expect(html.match(/role="(?:status|alert)"/g)).toHaveLength(1);
    expect(html).toContain('role="status"');
  });

  it("renders the activation outcome with three reversible next actions and primary autofocus", () => {
    const html = renderToString(
      <AiMutationOutcomeDialog
        {...sharedProps}
        outcome={{ kind: "activate", editorState, syncError: null }}
      />,
    );

    expect(html).toContain("현재 아이콘이 AI 소스를 사용 중입니다.");
    expect(html).toContain("편집기로 돌아가기");
    expect(html).toContain("원본으로 돌아가기");
    expect(html).toContain("소스 이력 보기");
    expect(openingButton(html, "ai-outcome-return-editor")).toContain(
      'autofocus=""',
    );
    expect(html.match(/role="(?:status|alert)"/g)).toHaveLength(1);
  });

  it("keeps a completed create outcome visible when list sync fails", () => {
    const html = renderToString(
      <AiMutationOutcomeDialog
        {...sharedProps}
        outcome={{
          kind: "create",
          candidateId: candidate.id,
          createdIcon,
          createdIconUsage: {
            createdIconCount: 1,
            latestCreatedIcon: createdIcon,
          },
          syncError: "목록 반영 실패",
        }}
      />,
    );

    expect(html).toContain("저장은 완료됐지만 아이콘 목록에 표시하지 못했습니다.");
    expect(html).toContain("목록 새로고침");
    expect(html).toContain('role="alert"');
    expect(html.match(/role="(?:status|alert)"/g)).toHaveLength(1);
    expect(html).not.toContain("AI 저장 실패");
  });
});

describe("AiImportResultPanel accessibility", () => {
  it("connects labels, help, field errors, and the disabled import reason", () => {
    const html = renderToString(
      <AiImportResultPanel
        currentVisualSource={visualSource}
        disabled={false}
        fileErrorMessage="PNG 또는 JPG 파일을 선택해 주세요."
        fileInputRef={{ current: null }}
        isImporting={false}
        selectedFile={null}
        serviceSurface="gemini_web"
        onFileChange={() => {}}
        onImport={() => {}}
        onServiceSurfaceChange={() => {}}
      />,
    );

    expect(html).toContain('for="ai-service-surface"');
    expect(html).toContain('aria-describedby="ai-service-surface-help"');
    expect(html).toContain('for="ai-candidate-file"');
    expect(html).toContain('aria-invalid="true"');
    expect(html).toContain(
      'aria-describedby="ai-candidate-file-help ai-candidate-file-preservation ai-candidate-file-error"',
    );
    expect(html).toContain('id="ai-candidate-file-error"');
    expect(html).toContain("PNG 또는 JPG 파일을 선택해 주세요.");
    expect(html).toContain('id="ai-import-file-required"');
    expect(openingButton(html, "ai-import-candidate")).toContain(
      'aria-describedby="ai-import-file-required"',
    );
  });
});

describe("committed AI mutation display sync", () => {
  it("shows a cached-state retry without describing the save as failed", () => {
    const html = renderToString(
      <AiEditorRefreshWarning
        busy={false}
        detail="editor apply failed"
        disabled={false}
        onRetry={() => {}}
      />,
    );

    expect(html).toContain("저장은 완료됐지만 편집기 표시를 적용하지 못했습니다.");
    expect(html).toContain("서버를 다시 호출하지 않고");
    expect(html).toContain("편집기 표시 다시 적용");
    expect(html).not.toContain('role="alert"');
    expect(html).not.toContain("저장 실패");
  });

  it("keeps a cached created-icon retry after leaving the outcome", () => {
    const html = renderToString(
      <AiCreatedIconRefreshWarning
        busy={false}
        detail="list apply failed"
        disabled={false}
        onRetry={() => {}}
      />,
    );

    expect(html).toContain("새 아이콘은 이미 저장되어 있습니다.");
    expect(html).toContain("서버를 다시 호출하지 않고");
    expect(html).toContain("아이콘 목록 다시 반영");
    expect(html).not.toContain('role="alert"');
    expect(html).not.toContain("저장 실패");
  });
});

describe("AiVersionRecipeDetails", () => {
  const versionSource = {
    ...source,
    id: "source_version",
    originalFilename: "candidate-normalized.png",
    width: 640,
    height: 640,
  };
  const versionBase: AiVersion = {
    id: "version_1",
    candidateId: candidate.id,
    parentVersionId: null,
    source: versionSource,
    normalizationRecipeHash: "1234567890abcdef",
    normalizationSummary: {
      kind: "contain_pad",
      mode: "contain_pad",
      alignment: "center",
      resizeFilter: "lanczos3",
      targetCanvasWidth: 640,
      targetCanvasHeight: 640,
    },
    isActive: false,
    isAvailable: true,
    unavailableReason: null,
    createdAt: "2026-07-27T10:05:00Z",
  };
  const rawCandidateSource = {
    ...source,
    id: "source_raw",
    width: 1024,
    height: 768,
  };

  it("distinguishes contain and cover versions from the same candidate", () => {
    const containHtml = renderToString(
      <AiVersionRecipeDetails
        candidateSource={rawCandidateSource}
        version={versionBase}
      />,
    );
    const coverHtml = renderToString(
      <AiVersionRecipeDetails
        candidateSource={rawCandidateSource}
        version={{
          ...versionBase,
          id: "version_2",
          normalizationSummary: {
            ...versionBase.normalizationSummary!,
            kind: "cover_crop",
            mode: "cover_crop",
            alignment: "top_right",
            resizeFilter: "nearest",
          },
        }}
      />,
    );

    expect(containHtml).toContain("전체 보이기 · 권장 · 가운데");
    expect(containHtml).toContain("부드럽게 · 일반 그림");
    expect(coverHtml).toContain("빈틈 없이 채우기 · 오른쪽 위");
    expect(coverHtml).toContain("픽셀 유지 · 픽셀 아트");
    expect(containHtml).toContain("후보 원본 1024×768px → 캔버스 640×640px");
  });

  it("uses a short recipe hash only when the structured summary is unavailable", () => {
    const html = renderToString(
      <AiVersionRecipeDetails
        candidateSource={rawCandidateSource}
        version={{ ...versionBase, normalizationSummary: null }}
      />,
    );

    expect(html).toContain("규격화 설정 미상 · 레시피 #12345678");
    expect(html).toContain("후보 원본 1024×768px → 캔버스 640×640px");
  });
});

describe("unavailable AI artifacts", () => {
  it("keeps an unavailable candidate visible but disables both commit actions with its reason", () => {
    const html = renderToString(
      <AiCandidateActionButtons
        actionLockReason={null}
        candidate={{
          ...candidate,
          isAvailable: false,
          unavailableReason: "후보 파일이 없습니다.",
        }}
        currentCompatibility={allowedCompatibility}
        disabled={false}
        isCurrentRecipe={false}
        isActivating={false}
        isCreating={false}
        newIconCompatibility={allowedCompatibility}
        previewReady
        onActivate={() => {}}
        onCreate={() => {}}
      />,
    );

    expect(openingButton(html, "ai-create-icon-candidate_1")).toContain(
      ' disabled=""',
    );
    expect(openingButton(html, "ai-activate-current-candidate_1")).toContain(
      ' disabled=""',
    );
    expect(html).toContain("후보 파일이 없습니다.");
  });

  it("renders a deliberate placeholder instead of a broken image element", () => {
    const html = renderToString(
      <AiUnavailableImagePlaceholder label="missing.png" size="candidate" />,
    );

    expect(html).toContain("missing.png 미리보기를 사용할 수 없음");
    expect(html).toContain("ai-unavailable-image-candidate");
    expect(html).not.toContain("<img");
  });
});
