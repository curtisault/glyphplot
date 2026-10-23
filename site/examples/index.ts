// Every example, in gallery order (REQ-SITE-040): each module is
// site/examples/<slug>.ts exporting `example`. Filled by the examples work of
// Phase 6; the Elm site reads metadata from manifest.json and the running
// examples from here.
import type { Example } from "./api.ts";

export const examples: Example[] = [];

/** The example with this slug, or undefined. */
export function exampleOf(slug: string): Example | undefined {
  return examples.find((e) => e.slug === slug);
}
