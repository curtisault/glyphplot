// The browser tests' bare page: defines <glyph-plot> and puts the JS API on
// window.gp, so a test can count module instantiations (REQ-LOAD-010), mount
// charts by hand and read the manifest.
import * as gp from "glyphplot";
import "glyphplot/element";

declare global {
  interface Window {
    gp: typeof gp;
    gpReady: true;
  }
}

window.gp = gp;
window.gpReady = true;
