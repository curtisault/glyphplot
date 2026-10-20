// The browser tests' bare page: defines <glyph-plot> and puts the JS API on
// window.gp and the element's module on window.gpElement, so a test can count
// module instantiations (REQ-LOAD-010), mount charts by hand, read the
// manifest and define aliases (REQ-EL-030).
import * as gp from "glyphplot";
import * as gpElement from "glyphplot/element";

declare global {
  interface Window {
    gp: typeof gp;
    gpElement: typeof gpElement;
    gpReady: true;
  }
}

window.gp = gp;
window.gpElement = gpElement;
window.gpReady = true;
