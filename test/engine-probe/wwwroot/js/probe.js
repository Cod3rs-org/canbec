// Feature probes called from Blazor. Each returns a short detail string or throws.
window.probe = {
  canvas2d(id) {
    const c = document.getElementById(id), g = c.getContext("2d");
    const bars = [30, 55, 42, 70, 25];
    bars.forEach((h, i) => { g.fillStyle = `hsl(${20 + i * 12} 85% 58%)`; g.fillRect(8 + i * 22, 80 - h, 16, h); });
    return "bar chart drawn";
  },
  webgl(id) {
    const gl = document.getElementById(id).getContext("webgl");
    if (!gl) throw new Error("no WebGL context");
    gl.clearColor(1.0, 0.48, 0.27, 1.0);
    gl.clear(gl.COLOR_BUFFER_BIT);
    return gl.getParameter(gl.VERSION);
  },
  camera() {
    if (!navigator.mediaDevices?.getUserMedia) throw new Error("getUserMedia missing");
    return "getUserMedia available";
  },
  storage() {
    localStorage.setItem("probe", "1");
    return localStorage.getItem("probe") === "1" ? "read/write ok" : "mismatch";
  },
  async fetchSelf() {
    const r = await fetch("index.html");
    return `HTTP ${r.status}`;
  },
  serviceWorker() {
    if (!("serviceWorker" in navigator)) throw new Error("not supported");
    return "supported";
  },
  userAgent: () => navigator.userAgent,
};
