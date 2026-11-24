# Turn the user guide into your own docs

The package carries a user guide for the chart, written for any product. Your assistant can turn it into your
product's own help pages, so you do not write them from scratch.

## Print the prompt

`npx tachlon describe guide` prints the prompt your assistant follows. `--json` prints the same prompt as data.
The prompt tells the assistant to do six things in order:

1. Read how your product mounts the chart: its options, the events it listens to, and the members it calls.
2. Learn your product's name and voice from its own pages.
3. Keep only the features your product has, using each file's `needs` comment.
4. Write one page for each file it kept, in your product's format.
5. List the screenshots to take, with the state to set up in your own themed app.
6. Check each page against your code, and report what it dropped and why.

Paste this one line into your assistant:

```text
Follow `npx tachlon describe guide` to write this product's user guide from the chart's own.
```

## The guide files

The files are in `node_modules/tachlon/dist-lib/guide/`: one file for each feature (open a schedule,
navigate, build, change, the task dialog, progress, baseline, checks, print and save), the prompt in
`PROMPT.md`, and a `README.md` that holds the format of the files. A file says "the chart" and never your
product's name, and it holds no picture: your assistant leaves a placeholder where a picture belongs and
lists it for you to take.

## The licence

The guide files and the prompt are under MIT-0, so a document your assistant derives from them needs no notice
or credit.
