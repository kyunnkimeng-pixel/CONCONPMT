import { renderToString } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { PreviewComposer } from "@/features/preview/components/PreviewComposer";

describe("PreviewComposer", () => {
  it("names the clear action after the content it affects", () => {
    const html = renderToString(
      <PreviewComposer
        commentText="테스트 댓글"
        defaultCellHeight={200}
        defaultCellWidth={200}
        gifRefreshKey={0}
        insertedItems={[]}
        onClear={() => {}}
        onRemoveLast={() => {}}
        onTextChange={() => {}}
      />,
    );

    expect(html).toContain("미리보기 비우기");
    expect(html).toContain("입력한 댓글과 미리보기 아이콘만 비웁니다.");
    expect(html).not.toContain(">초기화<");
  });
});
