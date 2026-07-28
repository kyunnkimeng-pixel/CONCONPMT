import { renderToString } from "react-dom/server";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { CollectionSummary } from "@/features/collections/types";
import {
  AiSourceRepairNotice,
  EditorPanel,
} from "@/features/editor/components/EditorPanel";

describe("EditorPanel actions", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("names reset actions after their exact scope", () => {
    vi.stubGlobal("window", {
      localStorage: {
        getItem: () => null,
        setItem: () => undefined,
      },
    });

    const html = renderToString(
      <EditorPanel
        collection={
          {
            id: "collection_1",
            name: "테스트 모음",
          } as CollectionSummary
        }
        iconId="icon_1"
        onClose={() => {}}
        onIconUpdated={() => {}}
        onRevealIcon={() => true}
      />,
    );

    expect(html).toContain("크롭 기본값");
    expect(html).toContain("저장값으로 되돌리기");
    expect(html).toContain("크롭·변형 적용");
    expect(html).toContain("다른 편집값은 유지됩니다.");
    expect(html).not.toContain(">초기화<");
  });

  it("shows an accessible original-source repair action and progress label", () => {
    const readyHtml = renderToString(
      <AiSourceRepairNotice
        errorMessage="AI 소스 이력이 손상되었습니다."
        isRepairing={false}
        onRepair={() => {}}
      />,
    );
    const busyHtml = renderToString(
      <AiSourceRepairNotice
        errorMessage="AI 소스 이력이 손상되었습니다."
        isRepairing
        onRepair={() => {}}
      />,
    );

    expect(readyHtml).toContain("원본 소스로 복구");
    expect(readyHtml).toContain('aria-busy="false"');
    expect(readyHtml).toContain('data-testid="editor-ai-repair-original"');
    expect(busyHtml).toContain("원본 소스로 복구 중");
    expect(busyHtml).toContain('aria-busy="true"');
    expect(busyHtml).toContain("disabled");
  });
});
