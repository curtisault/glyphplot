// Types for elm.mjs, so the site's tsc checks vite.config.ts without Node's types.
import type { Plugin } from "vite";

/** The base the site is served under: `/`, or GLYPHPLOT_BASE. */
export function siteBase(): string;

/** Compiles imported `.elm` modules with `elm make` and exports their `Elm` object; `site` is the folder holding elm.json. */
export function elmPlugin(site: string): Plugin;
