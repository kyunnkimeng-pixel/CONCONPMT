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

function buttonWithText(container: HTMLElement, text: string) {
  const button = [...container.querySelectorAll<HTMLButtonElement>("button")].find(
    (candidate) => candidate.textContent?.includes(text),
  );
  if (!button) throw new Error(`Missing button containing text=${text}`);
  return button;
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

  async function prepareNovelAiHandoff(panelProps: AiWebHandoffPanelProps) {
    await renderPanel(panelProps);
    await act(async () => {
      setControlValue(
        testId<HTMLSelectElement>(container, "ai-web-handoff-service"),
        "novelai_web",
      );
      setControlValue(
        testId<HTMLTextAreaElement>(container, "ai-web-handoff-request"),
        "brighter smile",
      );
    });
    await act(async () => {
      testId<HTMLButtonElement>(container, "ai-web-handoff-prepare").click();
      await Promise.resolve();
    });
    await flushEffects();
    expect(
      testId<HTMLElement>(container, "novelai-copy-state-single_edit")
        .textContent,
    ).toContain("1/2 완료");
  }

  it("restores the latest live handoff and its service", async () => {
    const restored = session({
      serviceSurface: "novelai_web",
      finalPrompt: "single image, square canvas, icon, brighter smile",
    });
    const onRestoreLatest = vi.fn().mockResolvedValue(restored);

    await renderPanel(props({ onRestoreLatest }));

    expect(onRestoreLatest).toHaveBeenCalledOnce();
    expect(
      container.querySelector('[data-testid="ai-web-handoff-ready"]'),
    ).not.toBeNull();
    expect(
      testId<HTMLSelectElement>(container, "ai-web-handoff-service").value,
    ).toBe("novelai_web");
    expect(
      container.querySelector('[data-testid="novelai-web-guide-single_edit"]'),
    ).not.toBeNull();
    expect(container.textContent).toContain("Image2Image");
    expect(container.textContent).toContain("192×192");
    expect(container.textContent).toContain("NovelAI Prompt (태그)");
    expect(container.textContent).toContain("1/2 Prompt");
    expect(
      testId<HTMLButtonElement>(
        container,
        "novelai-copy-undesired-single_edit",
      ).disabled,
    ).toBe(true);
  });

  it("unlocks Undesired Content only after the NovelAI Prompt copy succeeds", async () => {
    const prepared = session({
      serviceSurface: "novelai_web",
      finalPrompt: "single image, square canvas, icon, brighter smile",
    });
    await renderPanel(
      props({
        onPrepare: vi.fn().mockResolvedValue(prepared),
        onOpenSite: vi.fn().mockResolvedValue(undefined),
      }),
    );

    await act(async () => {
      setControlValue(
        testId<HTMLSelectElement>(container, "ai-web-handoff-service"),
        "novelai_web",
      );
      setControlValue(
        testId<HTMLTextAreaElement>(container, "ai-web-handoff-request"),
        "brighter smile",
      );
    });
    expect(
      testId<HTMLButtonElement>(
        container,
        "novelai-copy-undesired-single_edit",
      ).disabled,
    ).toBe(true);

    await act(async () => {
      testId<HTMLButtonElement>(container, "ai-web-handoff-prepare").click();
      await Promise.resolve();
    });
    await flushEffects();

    expect(
      testId<HTMLElement>(container, "novelai-copy-state-single_edit")
        .textContent,
    ).toContain("1/2 완료");
    expect(
      testId<HTMLButtonElement>(
        container,
        "novelai-copy-undesired-single_edit",
      ).disabled,
    ).toBe(false);
  });

  it("keeps stale prepared prompt recopy disabled and NovelAI Undesired Content locked after the draft changes", async () => {
    const restored = session({
      serviceSurface: "gemini_web",
      finalPrompt: "GEMINI STRUCTURE\n사용자 편집 요청:\n웃게",
    });

    await renderPanel(
      props({ onRestoreLatest: vi.fn().mockResolvedValue(restored) }),
    );

    await act(async () => {
      setControlValue(
        testId<HTMLSelectElement>(container, "ai-web-handoff-service"),
        "novelai_web",
      );
      setControlValue(
        testId<HTMLTextAreaElement>(container, "ai-web-handoff-request"),
        "brighter smile",
      );
    });

    expect(buttonWithText(container, "프롬프트 다시 복사").disabled).toBe(
      true,
    );
    expect(
      testId<HTMLElement>(container, "novelai-copy-state-single_edit")
        .textContent,
    ).toContain("현재 1/2");
    expect(
      testId<HTMLButtonElement>(
        container,
        "novelai-copy-undesired-single_edit",
      ).disabled,
    ).toBe(true);
    expect(navigator.clipboard.writeText).not.toHaveBeenCalled();
  });

  it("ignores a delayed prompt copy completion after the current draft changes", async () => {
    const prepared = session({
      serviceSurface: "novelai_web",
      finalPrompt: "single image, square canvas, icon, brighter smile",
    });
    let resolveDelayedCopy: (() => void) | undefined;
    const delayedCopy = new Promise<void>((resolve) => {
      resolveDelayedCopy = resolve;
    });
    vi.mocked(navigator.clipboard.writeText)
      .mockResolvedValueOnce(undefined)
      .mockImplementationOnce(() => delayedCopy);

    await renderPanel(
      props({
        onPrepare: vi.fn().mockResolvedValue(prepared),
        onOpenSite: vi.fn().mockResolvedValue(undefined),
      }),
    );

    await act(async () => {
      setControlValue(
        testId<HTMLSelectElement>(container, "ai-web-handoff-service"),
        "novelai_web",
      );
      setControlValue(
        testId<HTMLTextAreaElement>(container, "ai-web-handoff-request"),
        "brighter smile",
      );
      testId<HTMLButtonElement>(container, "ai-web-handoff-prepare").click();
      await Promise.resolve();
    });
    await flushEffects();

    expect(
      testId<HTMLElement>(container, "novelai-copy-state-single_edit")
        .textContent,
    ).toContain("1/2 완료");

    await act(async () => {
      buttonWithText(container, "프롬프트 다시 복사").click();
      await Promise.resolve();
    });
    expect(navigator.clipboard.writeText).toHaveBeenCalledTimes(2);

    await act(async () => {
      setControlValue(
        testId<HTMLTextAreaElement>(container, "ai-web-handoff-request"),
        "sad expression",
      );
    });
    await act(async () => {
      resolveDelayedCopy?.();
      await delayedCopy;
    });
    await flushEffects();

    expect(
      testId<HTMLElement>(container, "novelai-copy-state-single_edit")
        .textContent,
    ).toContain("현재 1/2");
    expect(
      testId<HTMLButtonElement>(
        container,
        "novelai-copy-undesired-single_edit",
      ).disabled,
    ).toBe(true);
  });

  it("keeps the prepared card and does not open the site when prompt copy fails", async () => {
    const onOpenSite = vi.fn().mockResolvedValue(undefined);
    const onAnnouncement = vi.fn();
    const execCommandDescriptor = Object.getOwnPropertyDescriptor(
      document,
      "execCommand",
    );
    Object.defineProperty(document, "execCommand", {
      configurable: true,
      value: vi.fn(() => false),
    });
    vi.mocked(navigator.clipboard.writeText).mockRejectedValueOnce(
      new Error("clipboard denied"),
    );

    try {
      await renderPanel(props({ onOpenSite, onAnnouncement }));
      await act(async () => {
        setControlValue(
          testId<HTMLTextAreaElement>(container, "ai-web-handoff-request"),
          "더 밝게",
        );
      });
      await act(async () => {
        testId<HTMLButtonElement>(container, "ai-web-handoff-prepare").click();
        await Promise.resolve();
      });
      await flushEffects();

      expect(onOpenSite).not.toHaveBeenCalled();
      expect(
        testId<HTMLElement>(container, "ai-web-handoff-copy-status")
          .textContent,
      ).toContain("직접 복사 필요");
      expect(onAnnouncement).toHaveBeenCalledWith(
        expect.stringContaining("자동 복사에 실패"),
        "error",
      );
      expect(onAnnouncement).not.toHaveBeenCalledWith(
        expect.stringContaining("공식 웹사이트를 열었습니다"),
        "status",
      );
    } finally {
      if (execCommandDescriptor) {
        Object.defineProperty(document, "execCommand", execCommandDescriptor);
      } else {
        Reflect.deleteProperty(document, "execCommand");
      }
    }
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

  it("resumes an untouched restored NovelAI copy flow and locks it after an edit", async () => {
    const restored = session({
      serviceSurface: "novelai_web",
      finalPrompt: "single image, square canvas, icon, brighter smile",
    });
    await renderPanel(
      props({ onRestoreLatest: vi.fn().mockResolvedValue(restored) }),
    );

    expect(
      container.querySelector(
        '[data-testid="ai-web-handoff-draft-changed"]',
      ),
    ).toBeNull();
    const recopy = buttonWithText(container, "프롬프트 다시 복사");
    expect(recopy.disabled).toBe(false);

    await act(async () => {
      recopy.click();
      await Promise.resolve();
    });
    await flushEffects();
    expect(navigator.clipboard.writeText).toHaveBeenCalledWith(
      restored.finalPrompt,
    );
    expect(
      testId<HTMLElement>(container, "novelai-copy-state-single_edit")
        .textContent,
    ).toContain("1/2 완료");
    expect(
      testId<HTMLButtonElement>(
        container,
        "novelai-copy-undesired-single_edit",
      ).disabled,
    ).toBe(false);

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
    expect(buttonWithText(container, "프롬프트 다시 복사").disabled).toBe(true);
    expect(
      testId<HTMLButtonElement>(
        container,
        "novelai-copy-undesired-single_edit",
      ).disabled,
    ).toBe(true);
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

  it("resets the NovelAI prompt sequence after deleting a copied handoff", async () => {
    const prepared = session({
      serviceSurface: "novelai_web",
      finalPrompt: "single image, square canvas, icon, brighter smile",
    });
    const onDeleteSession = vi.fn().mockResolvedValue({
      sessionClosed: true,
      payloadDeleted: true,
      cleanupDeferred: false,
    });
    await prepareNovelAiHandoff(
      props({
        onPrepare: vi.fn().mockResolvedValue(prepared),
        onDeleteSession,
      }),
    );

    await act(async () => {
      testId<HTMLButtonElement>(container, "ai-web-handoff-delete").click();
      await Promise.resolve();
    });
    await flushEffects();

    expect(onDeleteSession).toHaveBeenCalledWith("ai_request_test");
    expect(
      testId<HTMLElement>(container, "novelai-copy-state-single_edit")
        .textContent,
    ).toContain("현재 1/2");
    expect(
      testId<HTMLButtonElement>(
        container,
        "novelai-copy-undesired-single_edit",
      ).disabled,
    ).toBe(true);
  });

  it("resets the NovelAI prompt sequence after an accepted result closes the handoff", async () => {
    const prepared = session({
      serviceSurface: "novelai_web",
      finalPrompt: "single image, square canvas, icon, brighter smile",
    });
    const onCommitResult = vi.fn().mockResolvedValue(acceptedInspection());
    await prepareNovelAiHandoff(
      props({
        onPrepare: vi.fn().mockResolvedValue(prepared),
        onCommitResult,
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
    expect(
      testId<HTMLElement>(container, "novelai-copy-state-single_edit")
        .textContent,
    ).toContain("현재 1/2");
    expect(
      testId<HTMLButtonElement>(
        container,
        "novelai-copy-undesired-single_edit",
      ).disabled,
    ).toBe(true);
  });

  it("resets the NovelAI prompt sequence when delete reports a closed session", async () => {
    const prepared = session({
      serviceSurface: "novelai_web",
      finalPrompt: "single image, square canvas, icon, brighter smile",
    });
    const terminalError = new CommandError(
      "ai_handoff_closed",
      "이 웹 전달은 이미 닫혔습니다.",
    );
    const onDeleteSession = vi.fn().mockRejectedValue(terminalError);
    await prepareNovelAiHandoff(
      props({
        onPrepare: vi.fn().mockResolvedValue(prepared),
        onDeleteSession,
      }),
    );

    await act(async () => {
      testId<HTMLButtonElement>(container, "ai-web-handoff-delete").click();
      await Promise.resolve();
    });
    await flushEffects();

    expect(
      container.querySelector('[data-testid="ai-web-handoff-ready"]'),
    ).toBeNull();
    expect(
      testId<HTMLElement>(container, "novelai-copy-state-single_edit")
        .textContent,
    ).toContain("현재 1/2");
    expect(
      testId<HTMLButtonElement>(
        container,
        "novelai-copy-undesired-single_edit",
      ).disabled,
    ).toBe(true);
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
  it("clears the previously inspected filename after a later multi-file selection is rejected", async () => {
    const onCommitResult = vi.fn().mockResolvedValue(blockingInspection());
    await renderPanel(
      props({
        onRestoreLatest: vi.fn().mockResolvedValue(session()),
        onCommitResult,
      }),
    );
    const first = new File([new Uint8Array([137, 80, 78, 71])], "result.png", {
      type: "image/png",
    });
    const second = new File([new Uint8Array([137, 80, 78, 71])], "other.png", {
      type: "image/png",
    });

    await act(async () => {
      dropFiles(
        testId<HTMLDivElement>(container, "ai-web-handoff-result-drop"),
        [first],
      );
      await Promise.resolve();
    });
    await flushEffects();

    expect(container.textContent).toContain("검사한 파일: result.png");
    expect(onCommitResult).toHaveBeenCalledOnce();

    await act(async () => {
      dropFiles(
        testId<HTMLDivElement>(container, "ai-web-handoff-result-drop"),
        [first, second],
      );
    });

    expect(container.textContent).not.toContain("검사한 파일: result.png");
    expect(container.querySelector('[role="alert"]')?.textContent).toContain(
      "이미지 파일 한 장만",
    );
    expect(onCommitResult).toHaveBeenCalledOnce();
  });
});
