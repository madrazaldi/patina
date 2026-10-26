import { expect, test } from "bun:test";
import { splitArgs } from "./player";

test("splits_quoted_args", () => {
  expect(splitArgs(`--fs --title="a b" 'x y'`)).toEqual(["--fs", "--title=a b", "x y"]);
  expect(splitArgs("  ")).toEqual([]);
});
