import { defineConfig } from "@inlang/paraglide-js";

// Tracked with `git add -f`: the inlang SDK rewrites this folder's .gitignore to admit only settings.json.
export default defineConfig({
  outdir: "./src/paraglide",
  /* The webview reports the system language, so a Chinese machine reads the
     Chinese catalog and every other one falls back to English. `baseLocale`
     stays last, since it is the one strategy that always answers. */
  strategy: ["preferredLanguage", "baseLocale"],
});
