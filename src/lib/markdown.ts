import { marked } from "marked";
import DOMPurify from "dompurify";
import { openUrl } from "@tauri-apps/plugin-opener";

marked.setOptions({ gfm: true, breaks: false });

export function renderMarkdown(md: string): string {
  const html = marked.parse(md, { async: false }) as string;
  return DOMPurify.sanitize(html);
}

/**
 * Click handler for rendered-markdown containers: external links open in the
 * default browser; relative links are passed to the caller (e.g. to open a
 * sibling artifact in-app). Prevents the webview from navigating away.
 */
export function handleMarkdownClick(
  e: MouseEvent,
  onRelativeLink?: (href: string) => void,
): void {
  const anchor = (e.target as HTMLElement).closest("a");
  if (!anchor) return;
  const href = anchor.getAttribute("href");
  if (!href) return;
  e.preventDefault();
  if (/^https?:\/\//i.test(href)) {
    void openUrl(href);
  } else if (!href.startsWith("#")) {
    onRelativeLink?.(decodeURI(href));
  }
}
