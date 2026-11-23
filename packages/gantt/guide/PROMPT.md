# Turn the chart's user guide into this product's own documentation

You are a coding assistant working in a product that embeds the chart. The folder `node_modules/{name}/dist-lib/guide/` holds a user guide for the chart, written for any product. Write this product's own user documentation from it: the product's name and voice, only the features the product has, and a list of the screenshots to take. Do the six steps in order. The folder's `README.md` holds the format of the files; `PROMPT.md` is this prompt, not a chapter.

## 1. Read how the product uses the chart

Open the code that mounts the chart and the pages around it. Write down for yourself:

- the options it passes to `mount`, and whether the chart always starts read-only;
- the events it listens to with `on`;
- the members of the chart, or of its `commands`, that its pages call: a button, a menu item, a key handler.

`npx {name} describe options`, `npx {name} describe events`, `npx {name} describe commands` and `npx {name} describe methods` list the names for the installed version.

## 2. Learn the product's name and voice

Find the product's name where the product writes it: its page title, its header, its README, its help text. Read two or three of its existing help pages for how it speaks: "you" or "we", formal or plain, and its words for a task, a schedule and a project. Ask the person only when the product does not answer.

## 3. Keep only the features the product has

Each file or `##` section carries a comment `<!-- needs: term, term -->` (under the file's title it covers the whole file). Keep the text when every term holds for this product:

| Term | Keep the text when |
|---|---|
| `chart` | always |
| `edit` | the product does not always start the chart read-only |
| `option:<name>` | the product passes that option to `mount` |
| `event:<name>` | the product listens to that event |
| `call:<member>` | the product's pages call that member of the chart or of its `commands` |

Drop the text of a file or section when a term fails. Write down each one you dropped and the term that failed.

## 4. Write the pages

Write one page for each file you kept, in the product's own format and in the folder where it keeps its help.

- Where a file says "the chart", write the product's own name for that part of its screen.
- Use the product's voice from step 2.
- A word in bold is the chart's own English text for a button or a field. Where the product changed it with `texts`, write the product's word; leave the rest as they are.
- A line `<!-- control: member | what it does -->` marks a step that a product's own control carries out. Find the control in the product's code that calls that member, and write the step with its name and place. Where the product has no such control, drop the step and renumber the rest.
- Take out every comment of the guide.

The guide is under the package's license (MIT): add the notice in `node_modules/{name}/LICENSE` to the product's third-party notices.

## 5. List the screenshots

A line `<!-- shot: id | what the picture shows -->` marks where a picture belongs. Leave a placeholder there, and write the list of pictures in a file beside the pages. Each entry has the id, the page, what the picture shows and the state to set up in the product's own themed app before taking it.

## 6. Check the pages

Read each page against the product's code and its screen:

- every feature a page describes is one the product has;
- every button or field a page names is one a person sees;
- no comment of the guide is left, no sentence says "the chart" where the product's own name belongs, and the package's name is on no page.

Then report: the pages you wrote, the files and sections you dropped with the term that failed, and the screenshot list.

## Never

- Never invent a feature. If the guide does not say it, the page does not.
- Never keep text whose `needs` term fails for this product.
- Never copy a picture. The guide has none; each picture is taken from the product's own app.
- Never write the package's name in a page: a person using the product does not know it.
- Never change the files in `node_modules`. Write the pages in the product.
- Never link to a file of the guide: the product's pages link to each other.
