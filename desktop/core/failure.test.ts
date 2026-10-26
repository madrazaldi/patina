import { expect, test } from "bun:test";
import { PlexError } from "../plex";
import { explain, explainError, lowerQuality } from "./failure";

test("explains_what_mpv_says", () => {
  expect(explain("loading failed")[0]).toBe("network");
  expect(explain("unrecognized file format")[0]).toBe("format");
  expect(explain("no audio or video data played")[0]).toBe("format");
  expect(explain("audio output initialization failed")[0]).toBe("output");
  expect(explain("video output initialization failed")[0]).toBe("output");
  expect(explain("")[0]).toBe("other");
});

test("lowers_quality_one_step_at_a_time", () => {
  expect(lowerQuality("original")?.id).toBe("20m-1080");
  expect(lowerQuality("20m-1080")?.id).toBe("12m-1080");
  expect(lowerQuality("2m-720")?.id).toBe("1m-480");
  expect(lowerQuality("1m-480")).toBeNull();
  expect(lowerQuality("nonsense")?.id).toBe("20m-1080");
});

test("network_errors_are_recognised_through_the_error_chain", () => {
  const inner = PlexError.noConnection("Home");
  const e = new Error(`creating play queue: ${inner.message}`, { cause: inner });
  expect(explainError(e)[0]).toBe("network");
  expect(explainError(e)[1]).toBe('creating play queue: no reachable connection for server "Home"');
  expect(explainError(new Error("mpv is missing"))[0]).toBe("other");
});
