// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  AiWebHandoffPanel,
  type AiWebHandoffPanelProps,
} from "@/features/editor/components/AiWebHandoffPanel";
import type {
  AiWebHandoffDeleteResult,
  AiWebHandoffResultInspection,
  AiWebHandoffSession,
} from "@/features/editor/types";
import { CommandError } from "@/lib/tauri";

const actEnvironment = globalThis as typeof globalThis & {
  IS_REACT_ACT_ENVIRONMENT?: boolean;
};

function session(
  overrides: Partial<AiWebHandoffSession> = {},
): AiWebHandoffSession {
  return {
    requestId: "ai_request_test",
    kind: "static_icon_sheet",
    layoutMode: "single",
    operation: "edit",
    serviceSurface: "gemini_web",
    finalPrompt: "STRUCTURE\n사용자 편집 요청:\n웃게",
    uploadFileName: "upload.png",
    uploadPreviewPath: "asset://upload.png",
    expectedWidth: 200,
    expectedHeight: 200,
    expectedHasAlpha: true,
    createdAt: "2026-07-28T00:00:00.000Z",
    expiresAt: "2026-08-04T00:00:00.000Z",
    canExtend: true,
    nativeDragSupported: false,
    warnings: [],
    ...overrides,
  };
}

function acceptedInspection(
  overrides: Partial<AiWebHandoffResultInspection> = {},
): AiWebHandoffResultInspection {
  return {
    accepted: true,
    issues: [
      {
        code: "ai_handoff_result_manual_review",
        severity: "manual_review",
        message: "직접 확인",
        expected: null,
        actual: null,
      },
    ],
    validationSignature: "sig",
    expectedWidth: 200,
    expectedHeight: 200,
    expectedHasAlpha: true,
    actualWidth: 200,
    actualHeight: 200,
    actualHasAlpha: true,
    reviewState: null,
    ...overrides,
  };
}

function blockingInspection(): AiWebHandoffResultInspection {
  return acceptedInspection({
    accepted: false,
    validationSignature: null,
    actualWidth: 128,
    issues: [
      {
        code: "ai_handoff_result_dimensions",
        severity: "blocking",
        message: "크기 불일치",
        expected: "200×200px",
        actual: "128×200px",
        suggestedPrompt: "Keep the canvas exactly 200×200px.",
        localAction: "다시 요청",
      },
    ],
  });
}

function props(
  overrides: Partial<AiWebHandoffPanelProps> = {},
): AiWebHandoffPanelProps {
  const deleted: AiWebHandoffDeleteResult = {
    sessionClosed: true,
    payloadDeleted: true,
    cleanupDeferred: false,
  };
  return {
    disabled: false,
    hasUnsavedChanges: false,
    onBusyStart: vi.fn(() => true),
    onBusyEnd: vi.fn(),
    onAnnouncement: vi.fn(),
    onPrepare: vi.fn().mockResolvedValue(session()),
    onRestoreLatest: vi.fn().mockResolvedValue(null),
    onOpenSite: vi.fn().mockResolvedValue(undefined),
    onRevealUpload: vi.fn().mockResolvedValue(undefined),
    onStartNativeDrag: vi.fn(),
    onExtendRetention: vi.fn(),
    onDeleteSession: vi.fn().mockResolvedValue(deleted),
    onCommitResult: vi.fn(),
    onCommitted: vi.fn(),
    ...overrides,
  };
}

function testId<T extends HTMLElement>(container: HTMLElement, value: string) {
  const element = container.querySelector<T>(`[data-testid="${value}"]`);
  if (!element) throw new Error(`Missing element with data-testid=${value}`);
  return element;
}

function setControlValue(
  element: HTMLTextAreaElement | HTMLSelectElement,
  value: string,
) {
  const descriptor = Object.getOwnPropertyDescriptor(
    Object.getPrototypeOf(element),
    "value",
  );
  descriptor?.set?.call(element, value);
  element.dispatchEvent(
    new Event(element instanceof HTMLSelectElement ? "change" : "input", {
      bubbles: true,
    }),
  );
}

function setInputFiles(input: HTMLInputElement, files: File[]) {
  Object.defineProperty(input, "files", {
    configurable: true,
    value: files,
  });
  input.dispatchEvent(new Event("change", { bubbles: true }));
}

function dropFiles(target: HTMLElement, files: File[]) {
  const event = new Event("drop", { bubbles: true, cancelable: true });
  Object.defineProperty(event, "dataTransfer", {
    configurable: true,
    value: { files },
  });
  target.dispatchEvent(event);
}

async function flushEffects() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
  });
}

describe("AiWebHandoffPanel lifecycle", () => {
  let container: HTMLDivElement;
  let root: Root;
  let clipboardDescriptor: PropertyDescriptor | undefined;

  beforeEach(() => {
    actEnvironment.IS_REACT_ACT_ENVIRONMENT = true;
    container = document.createElement("div");
    document.body.append(container);
    root = createRoot(container);
    clipboardDescriptor = Object.getOwnPropertyDescriptor(
      navigator,
      "clipboard",
    );
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: vi.fn().mockResolvedValue(undefined),
      },
    });
  });

  afterEach(async () => {
    await act(async () => {
      root.unmount();
    });
    container.remove();
    if (clipboardDescriptor) {
      Object.defineProperty(navigator, "clipboard", clipboardDescriptor);
    } else {
      Reflect.deleteProperty(navigator, "clipboard");
    }
    actEnvironment.IS_REACT_ACT_ENVIRONMENT = false;
    vi.restoreAllMocks();
  });

  async function renderPanel(panelProps: AiWebHandoffPanelProps) {
    await act(async () => {
      root.render(<AiWebHandoffPanel {...panelProps} />);
    });
    await flushEffects();
  }

  it("restores the latest live handoff and its service", async () => {
    const restored = session({ serviceSurface: "novelai_web" });
    const onRestoreLatest = vi.fn().mockResolvedValue(restored);

    await renderPanel(props({ onRestoreLatest }));

    expect(onRestoreLatest).toHaveBeenCalledOnce();
    expect(
      container.querySelector('[data-testid="ai-web-handoff-ready"]'),
    ).not.toBeNull();
    expect(
      testId<HTMLSelectElement>(container, "ai-web-handoff-service").value,
    ).toBe("novelai_web");
  });

  it("leaves no active card when there is no latest handoff", async () => {
    const onRestoreLatest = vi.fn().mockResolvedValue(null);

    await renderPanel(props({ onRestoreLatest }));

    expect(onRestoreLatest).toHaveBeenCalledOnce();
    expect(
      container.querySelector('[data-testid="ai-web-handoff-ready"]'),
    ).toBeNull();
  });

  it("clears a stale local card when a later restore reports no live handoff", async () => {
    const initialProps = props({
      onRestoreLatest: vi.fn().mockResolvedValue(session()),
    });
    await renderPanel(initialProps);
    expect(
      container.querySelector('[data-testid="ai-web-handoff-ready"]'),
    ).not.toBeNull();

    const onRestoreLatest = vi.fn().mockResolvedValue(null);
    await renderPanel({ ...initialProps, onRestoreLatest });

    expect(onRestoreLatest).toHaveBeenCalledOnce();
    expect(
      container.querySelector('[data-testid="ai-web-handoff-ready"]'),
    ).toBeNull();
  });

  it("marks a restored package as mismatched when the current draft is unknown", async () => {
    await renderPanel(
      props({ onRestoreLatest: vi.fn().mockResolvedValue(session()) }),
    );

    expect(
      container.querySelector(
        '[data-testid="ai-web-handoff-draft-changed"]',
      ),
    ).not.toBeNull();

    await act(async () => {
      setControlValue(
        testId<HTMLTextAreaElement>(container, "ai-web-handoff-request"),
        "눈물을 추가해 줘",
      );
    });

    expect(
      container.querySelector(
        '[data-testid="ai-web-handoff-draft-changed"]',
      ),
    ).not.toBeNull();
  });

  it("replaces the active card when prepare supersedes the previous package", async () => {
    const oldSession = session({ finalPrompt: "OLD FINAL PROMPT" });
    const replacement = session({
      requestId: "ai_request_replacement",
      finalPrompt: "NEW FINAL PROMPT",
    });
    const onPrepare = vi.fn().mockResolvedValue(replacement);
    const onDeleteSession = vi.fn();
    const panelProps = props({
      onRestoreLatest: vi.fn().mockResolvedValue(oldSession),
      onPrepare,
      onDeleteSession,
    });
    await renderPanel(panelProps);

    await act(async () => {
      setControlValue(
        testId<HTMLTextAreaElement>(container, "ai-web-handoff-request"),
        "새 표정",
      );
    });
    await act(async () => {
      testId<HTMLButtonElement>(container, "ai-web-handoff-prepare").click();
      await Promise.resolve();
    });
    await flushEffects();

    expect(onPrepare).toHaveBeenCalledWith("gemini_web", "새 표정");
    expect(onDeleteSession).not.toHaveBeenCalled();
    expect(
      container.querySelector<HTMLTextAreaElement>(
        "#ai-web-handoff-final-prompt",
      )?.value,
    ).toBe("NEW FINAL PROMPT");
    expect(
      container.querySelector(
        '[data-testid="ai-web-handoff-draft-changed"]',
      ),
    ).toBeNull();
  });

  it("closes the card and announces typed deferred payload cleanup", async () => {
    const deferred: AiWebHandoffDeleteResult = {
      sessionClosed: true,
      payloadDeleted: false,
      cleanupDeferred: true,
    };
    const onDeleteSession = vi.fn().mockResolvedValue(deferred);
    const onAnnouncement = vi.fn();
    await renderPanel(
      props({
        onRestoreLatest: vi.fn().mockResolvedValue(session()),
        onDeleteSession,
        onAnnouncement,
      }),
    );

    await act(async () => {
      testId<HTMLButtonElement>(container, "ai-web-handoff-delete").click();
      await Promise.resolve();
    });
    await flushEffects();

    expect(onDeleteSession).toHaveBeenCalledWith("ai_request_test");
    expect(
      container.querySelector('[data-testid="ai-web-handoff-ready"]'),
    ).toBeNull();
    expect(onAnnouncement).toHaveBeenCalledWith(
      expect.stringContaining("다음 앱 정리 때 다시 삭제"),
      "status",
    );
  });

  it("commits an accepted selected file, clears the card, and notifies the editor", async () => {
    const accepted = acceptedInspection();
    const onCommitResult = vi.fn().mockResolvedValue(accepted);
    const onCommitted = vi.fn();
    await renderPanel(
      props({
        onRestoreLatest: vi.fn().mockResolvedValue(session()),
        onCommitResult,
        onCommitted,
      }),
    );
    const file = new File([new Uint8Array([137, 80, 78, 71])], "result.png", {
      type: "image/png",
    });

    await act(async () => {
      setInputFiles(
        testId<HTMLInputElement>(container, "ai-web-handoff-result-input"),
        [file],
      );
      await Promise.resolve();
    });
    await flushEffects();

    expect(onCommitResult).toHaveBeenCalledWith("ai_request_test", file);
    expect(onCommitted).toHaveBeenCalledOnce();
    expect(onCommitted).toHaveBeenCalledWith(accepted);
    expect(
      container.querySelector('[data-testid="ai-web-handoff-ready"]'),
    ).toBeNull();
    expect(
      container.querySelector('[data-testid="ai-web-handoff-completed"]'),
    ).not.toBeNull();
  });

  it("clears a restored card when the restore endpoint reports a terminal session", async () => {
    const initialProps = props({
      onRestoreLatest: vi.fn().mockResolvedValue(session()),
    });
    await renderPanel(initialProps);

    const terminalError = new CommandError(
      "ai_handoff_stale",
      "이 웹 전달은 현재 아이콘과 달라 닫혔습니다.",
    );
    const onAnnouncement = vi.fn();
    await renderPanel({
      ...initialProps,
      onRestoreLatest: vi.fn().mockRejectedValue(terminalError),
      onAnnouncement,
    });

    expect(
      container.querySelector('[data-testid="ai-web-handoff-ready"]'),
    ).toBeNull();
    expect(onAnnouncement).toHaveBeenCalledWith(
      expect.stringContaining(terminalError.message),
      "error",
    );
  });

  it("clears the card when result commit reports a closed session", async () => {
    const terminalError = new CommandError(
      "ai_handoff_closed",
      "이 웹 전달은 이미 닫혔습니다.",
    );
    const onCommitResult = vi.fn().mockRejectedValue(terminalError);
    const onAnnouncement = vi.fn();
    await renderPanel(
      props({
        onRestoreLatest: vi.fn().mockResolvedValue(session()),
        onCommitResult,
        onAnnouncement,
      }),
    );
    const file = new File([new Uint8Array([137, 80, 78, 71])], "result.png", {
      type: "image/png",
    });

    await act(async () => {
      setInputFiles(
        testId<HTMLInputElement>(container, "ai-web-handoff-result-input"),
        [file],
      );
      await Promise.resolve();
    });
    await flushEffects();

    expect(onCommitResult).toHaveBeenCalledWith("ai_request_test", file);
    expect(
      container.querySelector('[data-testid="ai-web-handoff-ready"]'),
    ).toBeNull();
    expect(onAnnouncement).toHaveBeenCalledWith(terminalError.message, "error");
  });
  it("keeps the card and correction guidance after a blocking dropped file", async () => {
    const blocking = blockingInspection();
    const onCommitResult = vi.fn().mockResolvedValue(blocking);
    const onCommitted = vi.fn();
    await renderPanel(
      props({
        onRestoreLatest: vi.fn().mockResolvedValue(session()),
        onCommitResult,
        onCommitted,
      }),
    );
    const file = new File([new Uint8Array([137, 80, 78, 71])], "result.png", {
      type: "image/png",
    });

    await act(async () => {
      dropFiles(
        testId<HTMLDivElement>(container, "ai-web-handoff-result-drop"),
        [file],
      );
      await Promise.resolve();
    });
    await flushEffects();

    expect(onCommitResult).toHaveBeenCalledWith("ai_request_test", file);
    expect(onCommitted).not.toHaveBeenCalled();
    expect(
      container.querySelector('[data-testid="ai-web-handoff-ready"]'),
    ).not.toBeNull();
    expect(
      container.querySelector('[data-testid="ai-web-handoff-issues"]'),
    ).not.toBeNull();
    expect(
      container.querySelector<HTMLTextAreaElement>(
        "#ai-web-handoff-correction-prompt",
      )?.value,
    ).toContain("Keep the canvas exactly 200×200px.");
  });
});
