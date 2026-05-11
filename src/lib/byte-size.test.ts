import { describe, expect, it } from "vitest";

import {
  bytesToMegabytesInput,
  megabytesInputToBytes,
} from "@/lib/byte-size";

describe("byte size helpers", () => {
  it("formats byte values as editable MB strings", () => {
    expect(bytesToMegabytesInput(2_097_152)).toBe("2");
    expect(bytesToMegabytesInput(524_288)).toBe("0.5");
    expect(bytesToMegabytesInput(1)).toBe("0.000001");
  });

  it("parses decimal MB input back to bytes", () => {
    expect(megabytesInputToBytes("2")).toBe(2_097_152);
    expect(megabytesInputToBytes("2.5")).toBe(2_621_440);
    expect(megabytesInputToBytes("0.5")).toBe(524_288);
    expect(megabytesInputToBytes("0,25")).toBe(262_144);
  });

  it("lets callers treat blank or invalid input as uncommitted", () => {
    expect(megabytesInputToBytes("")).toBeNull();
    expect(megabytesInputToBytes("abc")).toBeNull();
    expect(megabytesInputToBytes("2abc")).toBeNull();
    expect(megabytesInputToBytes("0")).toBeNull();
  });
});
