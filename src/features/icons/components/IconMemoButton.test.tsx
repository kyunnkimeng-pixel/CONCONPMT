import { renderToString } from "react-dom/server";
import type { ReactElement } from "react";
import { describe, expect, it, vi } from "vitest";

import { IconMemoButton } from "@/features/icons/components/IconTile";

describe("IconMemoButton", () => {
  it("keeps an add entry visible when an icon has no memo", () => {
    const html = renderToString(
      <IconMemoButton displayName="웃음" note={null} onEdit={() => {}} />,
    );

    expect(html).toContain("웃음 메모 추가");
    expect(html).toContain('data-testid="icon-memo-add"');
  });

  it("exposes existing memo content and an edit action", () => {
    const html = renderToString(
      <IconMemoButton displayName="웃음" note="표정 수정 필요" onEdit={() => {}} />,
    );

    expect(html).toContain("웃음 메모 수정");
    expect(html).toContain("표정 수정 필요");
    expect(html).toContain('data-testid="icon-memo-indicator"');
  });

  it("opens the memo editor without bubbling tile selection or drag events", () => {
    const onEdit = vi.fn();
    const button = IconMemoButton({
      displayName: "웃음",
      note: null,
      onEdit,
    }) as ReactElement<{
      onClick: (event: { stopPropagation: () => void }) => void;
      onDoubleClick: (event: { stopPropagation: () => void }) => void;
      onPointerDown: (event: { stopPropagation: () => void }) => void;
    }>;
    const stopPropagation = vi.fn();

    button.props.onClick({ stopPropagation });
    button.props.onDoubleClick({ stopPropagation });
    button.props.onPointerDown({ stopPropagation });

    expect(onEdit).toHaveBeenCalledTimes(1);
    expect(stopPropagation).toHaveBeenCalledTimes(3);
  });
});
