import { defineConfig } from "astro/config";
import dopedocs from "dopedocs/astro";
import { docs } from "./src/docs.ts";

export default defineConfig({
  site: "https://keyhandoff.dev",
  integrations: [dopedocs({ docs, stylesheet: [true, "/docs-theme.css"], search: true })],
});
