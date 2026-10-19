// What a visually hidden data table costs the page: build time (to the next layout)
// and DOM nodes, for 1k to 100k rows of three columns. Throwaway (see README.md).
// Run in a browser on about:blank (devtools console, or chrome-devtools evaluate_script).
async () => {
  const out = [];
  const css = "position:absolute;width:1px;height:1px;overflow:hidden;clip-path:inset(50%);white-space:nowrap";
  for (const rows of [1000, 10000, 100000]) {
    for (let rep = 0; rep < 3; rep++) {
      const wrap = document.createElement("div");
      wrap.style.cssText = css;
      const t0 = performance.now();
      const table = document.createElement("table");
      table.createCaption().textContent = "Price by time";
      const head = table.createTHead().insertRow();
      for (const h of ["Time", "Series", "Value"]) {
        const th = document.createElement("th");
        th.scope = "col";
        th.textContent = h;
        head.appendChild(th);
      }
      const body = table.createTBody();
      for (let i = 0; i < rows; i++) {
        const tr = body.insertRow();
        const th = document.createElement("th");
        th.scope = "row";
        th.textContent = new Date(1760000000000 + i * 60000).toISOString().slice(0, 16);
        tr.appendChild(th);
        tr.insertCell().textContent = "A";
        tr.insertCell().textContent = (100 + Math.sin(i) * 5).toFixed(2);
      }
      wrap.appendChild(table);
      document.body.appendChild(wrap);
      void wrap.offsetHeight; // force style and layout
      const built = performance.now() - t0;
      const nodes = wrap.getElementsByTagName("*").length;
      const t1 = performance.now();
      wrap.remove();
      void document.body.offsetHeight;
      out.push({ rows, rep: rep + 1, buildMs: +built.toFixed(1), removeMs: +(performance.now() - t1).toFixed(1), elements: nodes });
      await new Promise((r) => setTimeout(r, 50));
    }
  }
  return { ua: navigator.userAgent, out };
};
