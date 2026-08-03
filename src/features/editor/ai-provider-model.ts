import type {
  AiCandidate,
  AiImageEditConsent,
  AiImageEditInput,
  AiOfficialResource,
  AiProvider,
} from "@/features/editor/types";

export const AI_PROVIDER_CHOICES = [
  {
    value: "novelai",
    label: "NovelAI API",
    description: "세션 PAT로 현재 이미지를 한 번에 1장 수정합니다.",
  },
  {
    value: "gemini",
    label: "Gemini API (실험실)",
    description: "자격 조건을 확인한 비공개 파일럿용 유료 API입니다.",
  },
  {
    value: "web",
    label: "웹 전달",
    description: "공식 웹사이트에서 직접 작업한 뒤 결과를 가져옵니다.",
  },
] as const;

export type AiProviderChoice = (typeof AI_PROVIDER_CHOICES)[number]["value"];

export const GEMINI_IMAGE_MODELS = [
  "gemini-2.5-flash-image",
  "gemini-3.1-flash-image",
] as const;

export type GeminiImageModel = (typeof GEMINI_IMAGE_MODELS)[number];

export const AI_OFFICIAL_RESOURCES = [
  "user_manual",
  "novelai_app",
  "novelai_pat",
  "novelai_docs",
  "novelai_terms",
  "gemini_ai_studio",
  "gemini_image_docs",
  "gemini_pricing",
  "gemini_terms",
] as const satisfies readonly AiOfficialResource[];

export const NOVELAI_LIMITS = {
  width: { min: 64, max: 4096, step: 64 },
  height: { min: 64, max: 4096, step: 64 },
  steps: { min: 1, max: 50, step: 1 },
  scale: { min: 0, max: 20, step: 0.1 },
  strength: { min: 0, max: 1, step: 0.01 },
  noise: { min: 0, max: 1, step: 0.01 },
} as const;

export interface NovelAiEditDraft {
  prompt: string;
  negativePrompt: string;
  model: string;
  action: string;
  width: number;
  height: number;
  steps: number;
  scale: number;
  strength: number;
  noise: number;
  humanActionConfirmed: boolean;
  rightsConfirmed: boolean;
  costConfirmed: boolean;
  requestContentConfirmed: boolean;
  contractOverrideConfirmed: boolean;
}

export interface GeminiEditDraft {
  prompt: string;
  model: GeminiImageModel;
  humanActionConfirmed: boolean;
  rightsConfirmed: boolean;
  costConfirmed: boolean;
  requestContentConfirmed: boolean;
  adultConfirmed: boolean;
  under18AudienceExcludedConfirmed: boolean;
  professionalBusinessConfirmed: boolean;
  supportedRegionConfirmed: boolean;
  paidServiceConfirmed: boolean;
}

export function createDefaultNovelAiEditDraft(): NovelAiEditDraft {
  return {
    prompt: "",
    negativePrompt: "",
    model: "",
    action: "",
    width: 1024,
    height: 1024,
    steps: 28,
    scale: 5,
    strength: 0.7,
    noise: 0,
    humanActionConfirmed: false,
    rightsConfirmed: false,
    costConfirmed: false,
    requestContentConfirmed: false,
    contractOverrideConfirmed: false,
  };
}

export function createDefaultGeminiEditDraft(): GeminiEditDraft {
  return {
    prompt: "",
    model: "gemini-2.5-flash-image",
    humanActionConfirmed: false,
    rightsConfirmed: false,
    costConfirmed: false,
    requestContentConfirmed: false,
    adultConfirmed: false,
    under18AudienceExcludedConfirmed: false,
    professionalBusinessConfirmed: false,
    supportedRegionConfirmed: false,
    paidServiceConfirmed: false,
  };
}

function finiteWithin(value: number, min: number, max: number) {
  return Number.isFinite(value) && value >= min && value <= max;
}

export function novelAiDraftErrors(draft: NovelAiEditDraft): string[] {
  const errors: string[] = [];
  if (!draft.prompt.trim()) errors.push("수정 프롬프트를 입력해 주세요.");
  if (!draft.model.trim()) errors.push("NovelAI 모델 ID를 입력해 주세요.");
  if (!draft.action.trim()) errors.push("NovelAI action 값을 입력해 주세요.");
  if (
    !finiteWithin(
      draft.width,
      NOVELAI_LIMITS.width.min,
      NOVELAI_LIMITS.width.max,
    ) ||
    draft.width % NOVELAI_LIMITS.width.step !== 0
  ) {
    errors.push("너비는 64~4096 사이의 64 배수여야 합니다.");
  }
  if (
    !finiteWithin(
      draft.height,
      NOVELAI_LIMITS.height.min,
      NOVELAI_LIMITS.height.max,
    ) ||
    draft.height % NOVELAI_LIMITS.height.step !== 0
  ) {
    errors.push("높이는 64~4096 사이의 64 배수여야 합니다.");
  }
  if (
    !Number.isInteger(draft.steps) ||
    !finiteWithin(
      draft.steps,
      NOVELAI_LIMITS.steps.min,
      NOVELAI_LIMITS.steps.max,
    )
  ) {
    errors.push("스텝은 1~50 사이의 정수여야 합니다.");
  }
  if (
    !finiteWithin(
      draft.scale,
      NOVELAI_LIMITS.scale.min,
      NOVELAI_LIMITS.scale.max,
    )
  ) {
    errors.push("프롬프트 강도(scale)는 0~20 사이여야 합니다.");
  }
  if (
    !finiteWithin(
      draft.strength,
      NOVELAI_LIMITS.strength.min,
      NOVELAI_LIMITS.strength.max,
    )
  ) {
    errors.push("원본 변화 강도(strength)는 0~1 사이여야 합니다.");
  }
  if (
    !finiteWithin(
      draft.noise,
      NOVELAI_LIMITS.noise.min,
      NOVELAI_LIMITS.noise.max,
    )
  ) {
    errors.push("노이즈(noise)는 0~1 사이여야 합니다.");
  }
  if (!draft.humanActionConfirmed) {
    errors.push("사람이 직접 시작하는 1회 요청임을 확인해 주세요.");
  }
  if (!draft.rightsConfirmed) {
    errors.push("원본과 프롬프트를 사용할 권리를 확인해 주세요.");
  }
  if (!draft.costConfirmed) {
    errors.push("Image Anlas 또는 구독 사용량이 들 수 있음을 확인해 주세요.");
  }
  if (!draft.requestContentConfirmed) {
    errors.push("현재 이미지와 프롬프트가 NovelAI로 전송됨을 확인해 주세요.");
  }
  if (!draft.contractOverrideConfirmed) {
    errors.push("모델 ID와 action의 실험적 계약을 직접 확인해 주세요.");
  }
  return errors;
}

export function geminiDraftErrors(draft: GeminiEditDraft): string[] {
  const errors: string[] = [];
  if (!draft.prompt.trim()) errors.push("수정 프롬프트를 입력해 주세요.");
  if (!GEMINI_IMAGE_MODELS.includes(draft.model)) {
    errors.push("앱이 허용한 Gemini 이미지 모델을 선택해 주세요.");
  }
  if (!draft.humanActionConfirmed) {
    errors.push("사람이 직접 시작하는 1회 요청임을 확인해 주세요.");
  }
  if (!draft.rightsConfirmed) {
    errors.push("원본과 프롬프트를 사용할 권리를 확인해 주세요.");
  }
  if (!draft.costConfirmed) {
    errors.push("모델별 유료 호출 비용을 확인해 주세요.");
  }
  if (!draft.requestContentConfirmed) {
    errors.push("현재 이미지와 프롬프트가 Google로 전송됨을 확인해 주세요.");
  }
  if (!draft.adultConfirmed) {
    errors.push("사용자가 만 18세 이상임을 확인해 주세요.");
  }
  if (!draft.under18AudienceExcludedConfirmed) {
    errors.push("API 클라이언트가 미성년자를 대상으로 하지 않음을 확인해 주세요.");
  }
  if (!draft.professionalBusinessConfirmed) {
    errors.push("전문적 또는 사업 목적의 개발 사용임을 확인해 주세요.");
  }
  if (!draft.supportedRegionConfirmed) {
    errors.push("현재 사용·배포 지역이 지원 대상임을 확인해 주세요.");
  }
  if (!draft.paidServiceConfirmed) {
    errors.push("Paid Services 및 관련 데이터 조건을 확인해 주세요.");
  }
  return errors;
}

function commonConsent(
  draft: Pick<
    NovelAiEditDraft | GeminiEditDraft,
    | "humanActionConfirmed"
    | "rightsConfirmed"
    | "costConfirmed"
    | "requestContentConfirmed"
  >,
): AiImageEditConsent {
  return {
    humanActionConfirmed: draft.humanActionConfirmed,
    rightsConfirmed: draft.rightsConfirmed,
    costConfirmed: draft.costConfirmed,
    requestContentConfirmed: draft.requestContentConfirmed,
    contractOverrideConfirmed: false,
    adultConfirmed: false,
    under18AudienceExcludedConfirmed: false,
    professionalBusinessConfirmed: false,
    supportedRegionConfirmed: false,
    paidServiceConfirmed: false,
  };
}

export function buildNovelAiEditInput(
  iconId: string,
  draft: NovelAiEditDraft,
): AiImageEditInput {
  return {
    iconId,
    provider: "novelai",
    prompt: draft.prompt.trim(),
    model: draft.model.trim(),
    options: {
      negativePrompt: draft.negativePrompt.trim() || undefined,
      action: draft.action.trim(),
      width: draft.width,
      height: draft.height,
      steps: draft.steps,
      scale: draft.scale,
      strength: draft.strength,
      noise: draft.noise,
    },
    consent: {
      ...commonConsent(draft),
      contractOverrideConfirmed: draft.contractOverrideConfirmed,
    },
  };
}

export function buildGeminiEditInput(
  iconId: string,
  draft: GeminiEditDraft,
): AiImageEditInput {
  return {
    iconId,
    provider: "gemini",
    prompt: draft.prompt.trim(),
    model: draft.model,
    options: {},
    consent: {
      ...commonConsent(draft),
      adultConfirmed: draft.adultConfirmed,
      under18AudienceExcludedConfirmed:
        draft.under18AudienceExcludedConfirmed,
      professionalBusinessConfirmed: draft.professionalBusinessConfirmed,
      supportedRegionConfirmed: draft.supportedRegionConfirmed,
      paidServiceConfirmed: draft.paidServiceConfirmed,
    },
  };
}

export function newestGeneratedCandidateId(
  previousCandidates: ReadonlyArray<Pick<AiCandidate, "id">>,
  nextCandidates: ReadonlyArray<
    Pick<AiCandidate, "id" | "createdAt" | "candidateIndex">
  >,
) {
  const previousIds = new Set(previousCandidates.map(({ id }) => id));
  const added = nextCandidates.filter(({ id }) => !previousIds.has(id));
  const pool = added.length > 0 ? added : nextCandidates;
  return (
    pool.reduce<(typeof pool)[number] | null>((newest, candidate) => {
      if (!newest) return candidate;
      const createdOrder = candidate.createdAt.localeCompare(newest.createdAt);
      if (createdOrder !== 0) return createdOrder > 0 ? candidate : newest;
      return candidate.candidateIndex >= newest.candidateIndex
        ? candidate
        : newest;
    }, null)?.id ?? null
  );
}
export type SecretInputLike = { value: string };

export function consumeSessionCredential(input: SecretInputLike | null) {
  if (!input) return "";
  const credential = input.value.trim();
  input.value = "";
  return credential;
}

export async function copyAiHandoffPrompt(
  prompt: string,
  writers: {
    clipboardWriteText?: (value: string) => Promise<void>;
    fallbackCopy: () => boolean;
  },
) {
  const value = prompt.trim();
  if (!value) return "empty" as const;
  if (writers.clipboardWriteText) {
    try {
      await writers.clipboardWriteText(value);
      return "clipboard" as const;
    } catch {
      // The local selection fallback below keeps copy useful without broad access.
    }
  }
  return writers.fallbackCopy() ? ("fallback" as const) : ("failed" as const);
}

export function isOfficialAiResource(
  value: string,
): value is AiOfficialResource {
  return (AI_OFFICIAL_RESOURCES as readonly string[]).includes(value);
}

export function formatAiProviderExecutionError(
  provider: AiProvider,
  message: string,
) {
  const label = provider === "novelai" ? "NovelAI" : "Gemini";
  const detail = message.trim() || "요청을 처리할 수 없습니다.";
  const retryNotice = detail.includes("자동 재시도하지 않았습니다")
    ? ""
    : " 자동 재시도하지 않았습니다.";
  return `${label} 요청 실패: ${detail}${retryNotice} 원본은 바뀌지 않았습니다.`;
}

export function providerConfigured(
  provider: AiProvider,
  status: { novelAiConfigured: boolean; geminiConfigured: boolean },
) {
  return provider === "novelai"
    ? status.novelAiConfigured
    : status.geminiConfigured;
}
