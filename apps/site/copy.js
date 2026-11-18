// A Copy button copies the text of the element its data-copy names. Its label says what happened for two seconds, and a
// status line says it to a screen reader. When the clipboard refuses, the text is selected so the reader can copy it.
const status = document.getElementById("copy-status");
for (const button of document.querySelectorAll("button[data-copy]")) {
  const label = button.textContent;
  let timer = 0;
  button.addEventListener("click", async () => {
    const source = document.getElementById(button.dataset.copy);
    if (!source) return;
    let said;
    try {
      await navigator.clipboard.writeText(source.textContent.trim());
      button.textContent = "Copied";
      said = "The prompt is copied.";
    } catch {
      const range = document.createRange();
      range.selectNodeContents(source);
      const selection = getSelection();
      selection.removeAllRanges();
      selection.addRange(range);
      button.textContent = "Selected: press Ctrl+C";
      said = "The prompt is selected. Press Control C, or Command C on a Mac, to copy it.";
    }
    if (status) status.textContent = said;
    clearTimeout(timer);
    timer = setTimeout(() => {
      button.textContent = label;
      if (status) status.textContent = "";
    }, 2000);
  });
}
