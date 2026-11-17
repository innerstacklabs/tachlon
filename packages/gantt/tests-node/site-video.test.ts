// The walkthrough section holds the narrated screencast: a muted, captioned player with no claim mark, over the files demos/overview-screencast made (gantt#1048).
import { readFileSync, statSync } from "node:fs";
import { test } from "node:test";
import { expect } from "@playwright/test";

const site = (name: string) => new URL(`../../../apps/site/${name}`, import.meta.url);
const html = readFileSync(site("index.html"), "utf8");

test("the_walkthrough_holds_a_captioned_screencast_whose_files_exist", () => {
  expect(html).toMatch(/<video controls playsinline muted preload="metadata" poster="shots\/video-poster\.webp" width="2880" height="1800">/);
  expect(html).toMatch(/<source src="media\/overview-narrated\.mp4"[\s\S]*<track kind="captions" src="media\/overview-narrated\.vtt"[^>]*\bdefault>/);
  expect(statSync(site("media/overview-narrated.mp4")).size, "the video every visitor may download").toBeLessThan(8_000_000);
  const cues = readFileSync(site("media/overview-narrated.vtt"), "utf8");
  expect(cues.startsWith("WEBVTT\n")).toBe(true);
  expect(cues.match(/-->/g)).toHaveLength(12);
});
