# The user guide, for any product

Each file in this folder tells a person how to use one feature of the chart, in words that fit any
product that embeds it. The files are under the package's license (MIT, the package's `LICENSE`).

## What a file says about itself

A file carries HTML comments, which a rendered page does not show.

`<!-- needs: term, term -->` says when the text applies. Under the file's title it covers the whole
file; under a `##` heading it covers that section instead. All the text of a file is covered one way
or the other. Every term must hold:

| Term | Holds when |
|---|---|
| `chart` | always: the control is in the chart's own markup |
| `edit` | the product does not always start the chart read-only |
| `option:<name>` | the product passes that `mount` option |
| `call:<member>` | the product's page calls that member of the chart or of its `commands` |
| `event:<name>` | the product listens to that event |

The package's `describe` command lists the names: `describe options`, `describe events` and
`describe commands` and `describe methods`, the chart's own methods; `describe refusals` lists why a call changes nothing.

`<!-- control: <member> | what it does -->` marks a step that a product's own control carries out.
Text that needs a `call:` holds at least one.

`<!-- shot: <id> | what the picture shows -->` marks where a picture belongs. A file holds no
picture: each product takes its own. An id is used once in the folder.

`<!-- ANCHOR: <name> -->` and `<!-- ANCHOR_END: <name> -->` mark a passage that another document can
include by name. `node scripts/check-guide.ts` fails an anchor with no end after it, an end with no anchor, and a name used twice in one file.

Inside a list, a comment is indented with its item: at the left margin it would split the list in
two.

## What a file leaves out

- The product's name: a file says "the chart".
- Pictures, a link to a `.md` file or to a path that starts with `/`, and the name of a sample schedule.
- A route through a control that only one page has. A `control` comment stands there instead.

A word in bold is the chart's own English text for a button or a field, so a product that passes
`texts` can find its own word for it. The two exceptions are `**Goal:**` and the phrase that opens a
numbered step.
