import { renderToString } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { AltInlineEditor } from "@/features/icons/components/AltInlineEditor";

function renderEditor(suppressLiveRegion: boolean) {
  return renderToString(
    <AltInlineEditor
      ariaLabel="테스트 alt 수정"
      suppressLiveRegion={suppressLiveRegion}
      validationMessage="중복된 alt 값입니다."
      validateDraft={() => null}
      value="중복"
      onCommit={() => true}
    />,
  );
}

describe("AltInlineEditor live-region suppression", () => {
  it("announces visible validation when no modal covers the grid", () => {
    const html = renderEditor(false);

    expect(html).toContain('role="alert"');
    expect(html).not.toContain('aria-hidden="true"');
  });

  it("keeps the visual message but removes background alert semantics", () => {
    const html = renderEditor(true);

    expect(html).toContain("중복된 alt 값입니다.");
    expect(html).toContain('aria-hidden="true"');
    expect(html).not.toContain('role="alert"');
  });
});
