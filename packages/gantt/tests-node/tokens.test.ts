// The three colour-token blocks of tokens.css, read as text, no page (moved from theme-attribute.spec.ts).
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { expect } from "@playwright/test";

test("the_three_token_blocks_name_the_same_tokens", () => {
  const css = readFileSync(new URL("../src/styles/tokens.css", import.meta.url), "utf8");
  const tokensIn = (selector: string): string[] => {
    const start = css.indexOf(selector);
    expect(start, `${selector} is in tokens.css`).toBeGreaterThanOrEqual(0);
    const body = css.slice(css.indexOf("{", start) + 1, css.indexOf("}", start));
    return [...body.matchAll(/(--gantt-[a-z-]+)\s*:/g)].map((m) => m[1]!).sort();
  };
  const media = tokensIn(":where(.gantt):where(:not([data-theme]))");
  expect(media.length, "the media block names colour tokens").toBeGreaterThan(20);
  expect(tokensIn(':where(.gantt[data-theme="dark"])')).toEqual(media);
  const light = tokensIn(':where(.gantt[data-theme="light"])');
  const base = tokensIn(":where(.gantt) {");
  expect(light).toEqual(media);
  for (const token of media) expect(base, `${token} has a light default`).toContain(token);
});
