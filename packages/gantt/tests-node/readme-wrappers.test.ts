// The README's React, Vue and Angular sections name every option `mount` takes. Read as text.
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { expect } from "@playwright/test";

const pkg = (file: string): string => readFileSync(new URL(`../${file}`, import.meta.url), "utf8");

test("the_readme_says_each_wrapper_takes_every_mount_option", () => {
  const body = /export interface MountOptions \{(.*?)^\}/ms.exec(pkg("src/gantt-types.ts"))![1]!;
  const options = [...body.matchAll(/^ {2}(\w+)\??:/gm)].map((m) => m[1]!);
  expect(options, "MountOptions lists the options").toEqual(expect.arrayContaining(["texts", "fonts", "wasmUrl"]));
  const readme = pkg("README.md");
  for (const name of ["React", "Vue", "Angular"]) {
    const section = new RegExp(`^## Use it in ${name}\\n(.*?)(?=^## |$(?![\\s\\S]))`, "ms").exec(readme)?.[1] ?? "";
    expect(section, `the ${name} section exists`).not.toEqual("");
    expect(options.filter((o) => !section.includes(`\`${o}\``)), `options the ${name} section leaves out`).toEqual([]);
  }
  expect(readme).not.toContain("do not pass it yet");
});
