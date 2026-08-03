import { renderToString } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";

import {
  AiWebHandoffPanel,
  type AiWebHandoffPanelProps,
} from "@/features/editor/components/AiWebHandoffPanel";

function props(
  overrides: Partial<AiWebHandoffPanelProps> = {},
): AiWebHandoffPanelProps {
  return {
    disabled: false,
    hasUnsavedChanges: false,
    onBusyStart: () => true,
    onBusyEnd: () => {},
    onAnnouncement: () => {},
    onPrepare: vi.fn(),
    onRestoreLatest: vi.fn().mockResolvedValue(null),
    onOpenSite: vi.fn(),
    onRevealUpload: vi.fn(),
    onStartNativeDrag: vi.fn(),
    onExtendRetention: vi.fn(),
    onDeleteSession: vi.fn(),
    onCommitResult: vi.fn(),
    onCommitted: vi.fn(),
    ...overrides,
  };
}

function openingTag(html: string, testId: string) {
  const marker = `data-testid="${testId}"`;
  const markerIndex = html.indexOf(marker);
  if (markerIndex < 0) return "";
  const start = html.lastIndexOf("<", markerIndex);
  const end = html.indexOf(">", markerIndex);
  return html.slice(start, end + 1);
}

describe("AiWebHandoffPanel initial workflow", () => {
  it("keeps the fast web flow focused on provider, desired edit, and one primary action", () => {
    const html = renderToString(<AiWebHandoffPanel {...props()} />);

    expect(html).toContain("사용할 웹사이트");
    expect(html).toContain("Gemini 웹");
    expect(html).toContain("NovelAI 웹");
    expect(html).toContain("원하는 수정");
    expect(html).toContain("웹 AI로 바로 준비");
    expect(html).not.toContain("Manifest");
    expect(html).not.toContain("JSON");
    expect(openingTag(html, "ai-web-handoff-prepare")).toContain(
      'disabled=""',
    );
  });

  it("blocks preparation while unsaved edits could make the visible source differ", () => {
    const html = renderToString(
      <AiWebHandoffPanel {...props({ hasUnsavedChanges: true })} />,
    );

    expect(html).toContain("저장하지 않은 편집");
    expect(html).toContain("먼저");
    expect(openingTag(html, "ai-web-handoff-prepare")).toContain(
      'disabled=""',
    );
  });

  it("provides an accessible action-only web-error helper without invented prompt controls", () => {
    const html = renderToString(<AiWebHandoffPanel {...props()} />);

    expect(html).toContain("웹사이트에서 오류가 표시됐나요?");
    expect(html).toContain('for="ai-web-handoff-web-error"');
    expect(html).toContain("웹 오류 문구 붙여넣기");
    expect(html).not.toContain("자동 재시도");
  });
});
