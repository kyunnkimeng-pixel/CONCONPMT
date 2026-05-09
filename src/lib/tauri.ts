import { invoke } from "@tauri-apps/api/core";

export class CommandError extends Error {
  code: string;

  constructor(code: string, message: string) {
    super(message);
    this.name = "CommandError";
    this.code = code;
  }
}

interface SerializedCommandError {
  code?: unknown;
  message?: unknown;
}

export async function invokeCommand<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw normalizeCommandError(error);
  }
}

export function getCommandErrorMessage(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }

  return "요청을 처리할 수 없습니다.";
}

function normalizeCommandError(error: unknown) {
  if (isSerializedCommandError(error)) {
    return new CommandError(
      typeof error.code === "string" ? error.code : "backend_error",
      typeof error.message === "string" ? error.message : "요청을 처리할 수 없습니다.",
    );
  }

  if (error instanceof Error) {
    return error;
  }

  if (typeof error === "string") {
    return new CommandError("backend_error", error);
  }

  return new CommandError("backend_error", "요청을 처리할 수 없습니다.");
}

function isSerializedCommandError(error: unknown): error is SerializedCommandError {
  return typeof error === "object" && error !== null && "message" in error;
}
