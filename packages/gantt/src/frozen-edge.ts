/**
 * The column the frozen Task block cuts (gs#957). With the table scrolled right a scrolled column can sit half under the frozen block,
 * and a sliver of its cells shows beside the edge. The head cell whose box straddles the frozen edge (the right side; the left in a right-to-left chart) is named in `data-edge-cut`
 * on #table (its first class); the stylesheet hides that column's cells in the head and every row. Hidden cells keep their boxes, so
 * the column comes back as soon as it clears the edge. Only a change of the column writes to the page.
 */
export function frozenEdge(tableEl: HTMLElement, headCols: HTMLElement | null): () => void {
  return () => {
    let cut = "";
    const frozen = headCols?.querySelector<HTMLElement>(":scope > .name");
    if (frozen && tableEl.dataset.scrolled === "true") {
      const edge = getComputedStyle(frozen).direction === "rtl" ? frozen.getBoundingClientRect().left : frozen.getBoundingClientRect().right;
      for (const cell of headCols!.children) {
        const box = cell.getBoundingClientRect();
        if (cell !== frozen && !cell.classList.contains("code") && !cell.classList.contains("wbs") && box.left < edge && edge < box.right) {
          cut = cell.classList[0] ?? "";
          break;
        }
      }
    }
    if ((tableEl.dataset.edgeCut ?? "") === cut) return;
    if (cut === "") delete tableEl.dataset.edgeCut;
    else tableEl.dataset.edgeCut = cut;
  };
}
