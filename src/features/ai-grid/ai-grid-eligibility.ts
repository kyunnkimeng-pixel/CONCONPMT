import type {
  CollectionSummary,
  IconSummary,
} from "@/features/collections/types";
import { isGifIcon } from "@/features/sheets/sheet-ui-model";

export function getAiGridEditDisabledReason(
  collection: CollectionSummary,
  icons: IconSummary[],
  selectedIconIds: string[],
) {
  if (selectedIconIds.length < 2) {
    return "AI 일괄 수정은 아이콘을 2개 이상 선택해야 합니다.";
  }
  if (selectedIconIds.length > 16) {
    return "AI 일괄 수정은 한 번에 최대 16개까지 지원합니다.";
  }

  const selected = new Set(selectedIconIds);
  const targets = icons.filter((icon) => selected.has(icon.id));
  if (targets.length !== selected.size) {
    return "선택한 아이콘 중 현재 모음에서 찾을 수 없는 항목이 있습니다.";
  }
  if (targets.some((icon) => icon.iconKind !== "image")) {
    return "빈 디시콘은 AI 일괄 수정 대상에 넣을 수 없습니다.";
  }
  if (targets.some((icon) => icon.shape !== "single")) {
    return "AI 일괄 수정은 현재 단일 아이콘만 지원합니다.";
  }
  if (targets.some(isGifIcon)) {
    return "GIF는 프레임 작업시트 왕복 기능을 사용해 주세요.";
  }
  if (
    targets.some((icon) => {
      const width = icon.cellWidthOverride ?? collection.defaultCellWidth;
      const height = icon.cellHeightOverride ?? collection.defaultCellHeight;
      return width !== height;
    })
  ) {
    return "AI 일괄 수정은 현재 정사각형 셀 아이콘만 지원합니다.";
  }
  return null;
}
