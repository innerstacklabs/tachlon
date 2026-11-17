/** Save image draws a page at this many pixels to each unit of the page's own `viewBox`: 2112 by 1632 for a Letter page's 1056 by 816. */
const IMAGE_SCALE = 2;

/** The picture's size in pixels for the page SVG `svg`: twice the width and height of its `viewBox`. Throws when the page has none. */
export function imageSize(svg: string): { width: number; height: number } {
  const box = /viewBox="\s*[-\d.]+[\s,]+[-\d.]+[\s,]+([\d.]+)[\s,]+([\d.]+)\s*"/.exec(svg);
  if (!box) throw new Error("the printed page has no viewBox to size the image from");
  return { width: Number(box[1]) * IMAGE_SCALE, height: Number(box[2]) * IMAGE_SCALE };
}

/**
 * Draw one printed page (the SVG text `App.print_pages` returns) as a PNG on white, twice the size of the page's `viewBox`. The page
 * goes through an `Image` from a blob URL and a 2D canvas, which reads no GPU canvas, so it works with the chart's own canvas unused.
 * The page's text is drawn in the system sans-serif, as when it is printed. Runs when the page asks for it, never per frame.
 */
export async function pagePng(svg: string): Promise<Uint8Array<ArrayBuffer>> {
  const { width, height } = imageSize(svg);
  // The page has only a viewBox; a width and height on it give the image the size to draw at in every browser.
  const sized = svg.replace("<svg ", `<svg width="${width}" height="${height}" `);
  const url = URL.createObjectURL(new Blob([sized], { type: "image/svg+xml" }));
  try {
    const image = new Image();
    const loaded = new Promise<void>((resolve, reject) => {
      image.onload = () => resolve();
      image.onerror = () => reject(new Error("the printed page could not be drawn as an image"));
    });
    image.src = url;
    await loaded;
    const canvas = document.createElement("canvas");
    canvas.width = width;
    canvas.height = height;
    const context = canvas.getContext("2d");
    if (!context) throw new Error("this browser has no 2D canvas");
    context.fillStyle = "#fff";
    context.fillRect(0, 0, width, height);
    context.drawImage(image, 0, 0, width, height);
    const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, "image/png"));
    if (!blob) throw new Error("the image could not be written as a PNG");
    return new Uint8Array(await blob.arrayBuffer());
  } finally {
    URL.revokeObjectURL(url);
  }
}
