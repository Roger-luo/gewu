export function setDocumentReady(ready: boolean): void {
  if (typeof document === "undefined") {
    return;
  }
  document.documentElement.dataset.gewuReady = ready ? "true" : "false";
}
