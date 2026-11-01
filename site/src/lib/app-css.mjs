// The app's own stylesheet (../../../src/styles.css), scoped to .kh so the site can
// draw real KeyHandoff screens with the app's tokens and classes. The app styles the
// whole page; here :root and body become the .kh window itself.
export function scopeAppCss(css) {
  const keyframes = [];
  const body = css
    .split("\n")
    .filter((line) => !line.startsWith("@import"))
    .filter((line) => !line.startsWith("html, body, #root"))
    .filter((line) => {
      if (!line.startsWith("@keyframes")) return true;
      keyframes.push(line);
      return false;
    })
    .join("\n")
    .replace(/^:root \{/m, ":scope {")
    .replace(/^body \{/m, ":scope {");
  return `@scope (.kh) {\n${body}\n}\n${keyframes.join("\n")}`;
}
