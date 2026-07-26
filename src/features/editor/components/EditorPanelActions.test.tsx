import { renderToString } from "react-dom/server";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { CollectionSummary } from "@/features/collections/types";
import { EditorPanel } from "@/features/editor/components/EditorPanel";

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
      />,
    );

    expect(html).toContain("크롭 기본값");
    expect(html).toContain("저장값으로 되돌리기");
    expect(html).toContain("다른 편집값은 유지됩니다.");
    expect(html).not.toContain(">초기화<");
  });
});
