/**
 * Tell the app whether the page asked for no motion: `apply` is called once now and again on every change of the
 * `prefers-reduced-motion: reduce` media query. Returns the function that stops watching. Modelled on `watchTheme`.
 */
export function watchReducedMotion(apply: (reduced: boolean) => void): () => void {
  const media = matchMedia("(prefers-reduced-motion: reduce)");
  const send = (): void => apply(media.matches);
  send();
  media.addEventListener("change", send);
  return () => media.removeEventListener("change", send);
}
