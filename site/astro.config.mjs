import { defineConfig } from "astro/config";
import dopedocs from "dopedocs/astro";
import { prepareCode } from "dopedocs/highlight";
import { docs as source } from "./src/docs.ts";

// Colour the code samples at build time; the pages ship no highlighter.
const docs = await prepareCode(source);

export default defineConfig({
  site: "https://keyhandoff.dev",
  integrations: [dopedocs({ docs, stylesheet: [true, "/docs-theme.css"], search: true })],
});
