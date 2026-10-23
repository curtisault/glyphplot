// Fetching a live third-party feed in the reader's browser (REQ-SITE-050):
// only when the reader asks (a button), never at build time, with a message
// rather than an error when the feed is down or answers with something else.

/** What a live load gave: the body, or why there is none, in words for the reader. */
export type Fetched<T> = { ok: true; value: T } | { ok: false; message: string };

/** GETs `url` as JSON or text within `ms`, never throwing. */
export async function fetchFeed<T>(url: string, as: "json" | "text", ms = 20_000): Promise<Fetched<T>> {
  const ctl = new AbortController();
  const timer = setTimeout(() => ctl.abort(), ms);
  try {
    const res = await fetch(url, { signal: ctl.signal, credentials: "omit" });
    if (!res.ok) return { ok: false, message: `the feed answered ${res.status} ${res.statusText}`.trim() };
    return { ok: true, value: (as === "json" ? await res.json() : await res.text()) as T };
  } catch (e) {
    const what = e instanceof Error && e.name === "AbortError" ? "it did not answer in time" : "it could not be reached";
    return { ok: false, message: what };
  } finally {
    clearTimeout(timer);
  }
}
