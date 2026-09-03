import { readFileSync } from "node:fs";

const settings = JSON.parse(
  readFileSync(new URL("../project.inlang/settings.json", import.meta.url)),
);
const baseMessages = readMessages(settings.baseLocale);
const baseKeys = Object.keys(baseMessages).filter((key) => key !== "$schema");

for (const locale of settings.locales) {
  const messages = readMessages(locale);
  const keys = Object.keys(messages).filter((key) => key !== "$schema");
  const missing = baseKeys.filter((key) => !(key in messages));
  const extra = keys.filter((key) => !(key in baseMessages));
  const blank = keys.filter(
    (key) => typeof messages[key] !== "string" || messages[key].trim() === "",
  );

  if (missing.length || extra.length || blank.length) {
    throw new Error(
      [
        `${locale} catalog does not match ${settings.baseLocale}.`,
        missing.length ? `Missing: ${missing.join(", ")}` : "",
        extra.length ? `Extra: ${extra.join(", ")}` : "",
        blank.length ? `Blank: ${blank.join(", ")}` : "",
      ]
        .filter(Boolean)
        .join("\n"),
    );
  }
}

function readMessages(locale) {
  return JSON.parse(
    readFileSync(new URL(`../messages/${locale}.json`, import.meta.url)),
  );
}

console.log(`Validated ${settings.locales.length} localization catalogs.`);
