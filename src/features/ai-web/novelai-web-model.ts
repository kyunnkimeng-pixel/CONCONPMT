export type NovelAiWebTask =
  | "single_edit"
  | "grid_edit"
  | "grid_generate"
  | "gif_frame_sheet";

export type NovelAiBackgroundPolicy =
  | "preserve_transparency"
  | "allow_opaque";

const SINGLE_UNDESIRED = [
  "text",
  "caption",
  "speech bubble",
  "watermark",
  "logo",
  "signature",
  "border",
  "frame",
  "cropped",
  "out of frame",
  "duplicate",
  "multiple views",
  "reference sheet",
  "checkerboard",
  "checkered background",
  "fake transparency",
  "opaque background",
].join(", ");

const SHEET_UNDESIRED = [
  "text",
  "caption",
  "speech bubble",
  "watermark",
  "logo",
  "signature",
  "grid lines",
  "cell labels",
  "merged cells",
  "missing cells",
  "checkerboard",
  "checkered background",
  "fake transparency",
  "opaque background",
  "cropped",
  "out of frame",
].join(", ");

function novelAiDownloadStep(backgroundPolicy: NovelAiBackgroundPolicy) {
  if (backgroundPolicy === "allow_opaque") {
    return "NovelAI 이미지 생성 화면의 메뉴(☰) → Account Settings를 연 뒤 Image Settings 탭 → Image Generation → Image Format for Generated Images에서 PNG/JPG/WebP 중 원하는 형식을 선택하세요. 배경 포함 결과에서는 배경 제거 도구가 필수가 아닙니다. 다만 체커무늬·가짜 투명 패턴은 실제 픽셀로 남으므로 사용하지 말고 Download Image로 저장하세요.";
  }
  return "NovelAI 이미지 생성 화면의 메뉴(☰) → Account Settings를 연 뒤 Image Settings 탭 → Image Generation → Image Format for Generated Images에서 PNG를 선택하세요. 투명 배경이 필요한 결과는 alpha를 확인하고, 배경이 채워졌다면 Director Tools의 Remove BG를 적용한 뒤 Download Image로 저장하세요.";
}

const NOVELAI_PROMPT_STEP =
  "PMTCONCON Studio에서 복사한 NovelAI Prompt를 NovelAI 화면의 Prompt 입력란에 먼저 붙여 넣으세요.";

const NOVELAI_UNDESIRED_STEP =
  "그다음 Undesired Content 입력란에 제외 태그를 붙여 넣으세요. 화면 레이아웃에 따라 Prompt와 같은 카드의 탭 또는 아래쪽 별도 필드로 보일 수 있습니다.";

export interface NovelAiGuideOptions {
  task: NovelAiWebTask;
  expectedCanvas: string;
  hasReference?: boolean;
  allowsProportionalNormalization?: boolean;
  backgroundPolicy?: NovelAiBackgroundPolicy;
}

export interface NovelAiGuideSpec {
  recommendedMode: string;
  modeReason: string;
  steps: string[];
  resolutionText: string;
  warningText: string | null;
}

export function novelAiUndesiredContentForTask(
  task: NovelAiWebTask,
  backgroundPolicy: NovelAiBackgroundPolicy = "preserve_transparency",
) {
  const content = task === "single_edit" ? SINGLE_UNDESIRED : SHEET_UNDESIRED;
  if (backgroundPolicy === "preserve_transparency") return content;
  return content
    .split(", ")
    .filter((tag) => tag !== "opaque background")
    .join(", ");
}

export function normalizeNovelAiPromptInput(value: string) {
  return value
    .trim()
    .toLocaleLowerCase("en-US")
    .replace(/[\r\n;]+/g, ", ")
    .replace(/\s*,\s*/g, ", ")
    .replace(/(?:,\s*){2,}/g, ", ")
    .replace(/\s{2,}/g, " ")
    .replace(/^,\s*|,\s*$/g, "");
}

export function needsNovelAiEnglishInputHint(value: string) {
  return /[^\x00-\x7f]/u.test(value.trim());
}

export function buildNovelAiGuideSpec({
  task,
  expectedCanvas,
  hasReference = false,
  allowsProportionalNormalization = false,
  backgroundPolicy = "preserve_transparency",
}: NovelAiGuideOptions): NovelAiGuideSpec {
  const downloadStep = novelAiDownloadStep(backgroundPolicy);
  if (task === "grid_generate") {
    return {
      recommendedMode: hasReference
        ? "Vibe Transfer 또는 Precise Reference"
        : "이미지 업로드 없이 생성",
      modeReason: hasReference
        ? "참고 시트는 출력 틀이 아닙니다. 색·질감은 Vibe Transfer, 캐릭터나 그림체 일관성은 V4.5의 Precise Reference를 사용하세요."
        : "참고 이미지가 없으므로 Prompt와 Undesired Content만 입력합니다.",
      steps: [
        hasReference
          ? "참고 시트를 Add a Base Img (Optional)에 올리세요. What do you want to do with this image? 선택 창이 표시되면 Vibe Transfer 또는 Precise Reference를 고르고, 참조 도구가 별도 패널로 표시되는 UI에서는 해당 패널의 Add Image를 사용하세요. 두 방식은 동시에 사용하지 않습니다."
          : "V4.5 모델과 정사각형 해상도를 선택하세요.",
        NOVELAI_PROMPT_STEP,
        NOVELAI_UNDESIRED_STEP,
        downloadStep,
        `출력 캔버스를 ${expectedCanvas}로 맞추고 셀 수·순서를 생성 후 반드시 확인하세요.`,
      ],
      resolutionText: `그리드 결과는 ${expectedCanvas}가 정확히 유지되어야 셀을 복원할 수 있습니다. 웹에서 크기가 바뀐 결과는 앱이 임의로 셀 분할하지 않습니다.`,
      warningText: hasReference
        ? "여러 Precise Character Reference는 서로 다른 캐릭터로 분리되지 않고 섞일 수 있습니다. 참조 기능은 구독 중에도 Anlas가 들 수 있으므로 생성 버튼의 비용을 확인하세요."
        : "생성형 AI는 정확한 셀 수와 순서를 보장하지 않습니다. 앱의 셀 검토에서 결과를 확인하세요.",
    };
  }

  const isSingle = task === "single_edit";
  const isGif = task === "gif_frame_sheet";
  return {
    recommendedMode: "Image2Image (권장)",
    modeReason: isSingle
      ? "업로드한 아이콘의 구도와 캐릭터를 바탕으로 수정하는 방식입니다. Strength와 Noise는 낮게 시작하고 미리보며 조절하세요."
      : isGif
        ? "프레임 위치·순서·셀 경계를 유지해야 하므로 clean PNG를 base image로 사용해야 합니다."
        : "기존 셀 배치와 아이콘 순서를 유지해야 하므로 입력 그리드를 base image로 사용해야 합니다.",
    steps: [
      isGif
        ? "프레임 시트 설정에서 NovelAI 웹 호환 GIF / 200x200 / 4x4 프리셋을 적용하세요. 각 clean PNG를 한 장씩 Add a Base Img (Optional)로 올리세요. What do you want to do with this image? 창이 표시되면 Image2Image를 선택하고, 바로 base image가 붙는 UI에서는 이어서 나타나는 Strength와 Noise를 낮게 시작하세요. 모든 페이지에 같은 Prompt와 같은 Strength/Noise/sampler 설정을 적용하되, 해상도는 앱이 각 페이지에 표시한 실제 캔버스로 바꾸세요."
        : "전달 PNG 한 장을 Add a Base Img (Optional)로 올리세요. What do you want to do with this image? 창이 표시되면 Image2Image를 선택하고, 바로 base image가 붙는 UI에서는 이어서 나타나는 Strength와 Noise를 낮게 시작하세요.",
      NOVELAI_PROMPT_STEP,
      NOVELAI_UNDESIRED_STEP,
      downloadStep,
      isSingle
        ? "정사각형 해상도를 선택해 생성한 뒤 Download Image로 받은 PNG/JPG/WebP를 앱으로 가져오세요. WebP는 내부 PNG로 안전하게 변환합니다."
        : isGif
          ? `각 clean PNG 결과를 내려받은 뒤 앱의 페이지별 슬롯에 연결하세요. 파일별 캔버스 크기는 다음과 정확히 같아야 합니다: ${expectedCanvas}.`
          : `출력 캔버스를 ${expectedCanvas}로 유지하고 PNG 또는 WebP로 내려받으세요. WebP는 내부 PNG로 안전하게 변환합니다.`,
    ],
    resolutionText: allowsProportionalNormalization
      ? `NovelAI에서는 200×200 입력이 192×192 등 가까운 지원 크기로 바뀔 수 있습니다. 같은 비율이고 필요한 투명도가 유지되면 PMTCONCON Studio가 후보를 보존하고 적용할 때 ${expectedCanvas}로 맞춥니다.`
      : isGif
        ? `각 clean PNG는 다음 파일별 크기 ${expectedCanvas}가 정확히 유지되어야 다시 가져올 수 있습니다. NovelAI가 어느 페이지든 캔버스를 바꾸면 해당 파일을 다시 생성해야 합니다.`
        : `이 작업은 ${expectedCanvas}가 정확히 유지되어야 다시 가져올 수 있습니다. NovelAI가 캔버스를 바꾸면 생성 설정을 조정한 뒤 다시 받아야 합니다.`,
    warningText: isGif
      ? "Vibe Transfer와 Precise Reference는 스타일·캐릭터 참고용이며 프레임 배치를 보장하지 않습니다. NovelAI가 다운로드명을 바꾸면 앱의 페이지별 PNG 연결에서 해당 페이지를 직접 지정하세요."
      : "Vibe Transfer는 그림체·색감 참고용이고 Precise Reference는 V4.5 캐릭터·스타일 참고용입니다. 정확한 배치 보존에는 대신 사용하지 마세요.",
  };
}
