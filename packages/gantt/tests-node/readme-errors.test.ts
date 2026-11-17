// Every error class the package exports is named in the README. Read as text.
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { expect } from "@playwright/test";

const pkg = (file: string): string => readFileSync(new URL(`../${file}`, import.meta.url), "utf8");

test("the_readme_names_every_error_class_the_package_exports", () => {
  const exported = [...pkg("src/index.ts").matchAll(/^export \{([^}]*)\} from/gm)].flatMap((m) => m[1]!.split(",").map((n) => n.trim()));
  const errors = exported.filter((n) => /Error$/.test(n));
  expect(errors, "the export lines hold the error classes").toEqual(expect.arrayContaining(["MountError", "PlainDataError"]));
  const readme = pkg("README.md");
  expect(errors.filter((n) => !readme.includes(`\`${n}\``)), "error classes exported but absent from the README").toEqual([]);
});
