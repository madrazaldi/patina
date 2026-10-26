import { expect, test } from "bun:test";

import { clean, responseOffset } from "./downloads";

test("ranges_never_append_a_full_or_misaligned_response", () => {
  expect(responseOffset(200, null, 500)).toEqual([0, null]);
  expect(responseOffset(206, "bytes 500-999/1000", 500)).toEqual([500, 1000]);
  expect(() => responseOffset(206, "bytes 0-499/1000", 500)).toThrow();
  expect(() => responseOffset(206, null, 500)).toThrow();
  expect(() => responseOffset(206, "bytes 500-1000/1000", 500)).toThrow();
});

test("range errors keep the Rust texts", () => {
  expect(() => responseOffset(500, null, 0)).toThrow("Unexpected download response: 500");
  expect(() => responseOffset(206, null, 0)).toThrow("Missing Content-Range");
  expect(() => responseOffset(206, "bytes 5", 0)).toThrow("Invalid Content-Range");
  expect(() => responseOffset(206, "bytes 5/10", 0)).toThrow("Invalid byte range");
  expect(() => responseOffset(206, "bytes a-5/10", 0)).toThrow("invalid digit found in string");
  expect(() => responseOffset(206, "bytes -5/10", 0)).toThrow("cannot parse integer from empty string");
  expect(() => responseOffset(206, "bytes 1-5/10", 0)).toThrow("Server returned the wrong byte range");
});

test("file names are cleaned like the Rust version", () => {
  expect(clean("A: B/C")).toBe("A B C");
  expect(clean("  ..hidden..  ")).toBe("hidden");
  expect(clean("")).toBe("Untitled");
  expect(clean("///")).toBe("Untitled");
});
