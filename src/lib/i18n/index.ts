import { derived, writable } from "svelte/store";
import { commands, events, type LocaleChanged } from "$lib/bindings";
import * as compiledMessages from "../../paraglide/messages.js";
import {
  baseLocale,
  getLocale,
  isLocale,
  setLocale,
  type Locale,
} from "../../paraglide/runtime.js";

export type MessageCatalog = typeof compiledMessages;

export const locale = writable<Locale>(baseLocale);
export const localePreference = writable("system");

// A new object is emitted for each locale so Svelte reevaluates every message
// call without requiring a provider in every Tauri window.
export const messages = derived(locale, () => ({ ...compiledMessages }));

export const languageOptions = [
  {
    value: "system",
    label: (catalog: MessageCatalog) => catalog.language_system(),
  },
  {
    value: "en",
    label: (catalog: MessageCatalog) => catalog.language_english(),
  },
  {
    value: "zh-CN",
    label: (catalog: MessageCatalog) => catalog.language_chinese(),
  },
] as const;

async function applyLocaleSettings(settings: LocaleChanged) {
  const nextLocale = isLocale(settings.locale) ? settings.locale : baseLocale;
  await setLocale(nextLocale, { reload: false });
  localePreference.set(settings.preference);
  locale.set(nextLocale);
  document.documentElement.lang = nextLocale;
}

export async function initLocalization(): Promise<() => void> {
  const unlisten = await events.localeChanged.listen((event) => {
    void applyLocaleSettings(event.payload);
  });

  try {
    await applyLocaleSettings(await commands.getLocaleSettings());
    return unlisten;
  } catch (error) {
    unlisten();
    const fallback = getLocale();
    locale.set(fallback);
    document.documentElement.lang = fallback;
    throw error;
  }
}

export async function setLanguagePreference(preference: string) {
  await applyLocaleSettings(await commands.setLocalePreference(preference));
}

export function errorMessage(cause: unknown): string {
  const value = cause instanceof Error ? cause.message : String(cause);
  switch (value) {
    case "Display name cannot be empty.":
      return compiledMessages.error_display_name_empty();
    case "Server address is required.":
      return compiledMessages.error_server_address_required();
    case "Port must be a whole number from 1 to 65535.":
      return compiledMessages.error_port_invalid();
    case "Identification key is required.":
      return compiledMessages.error_friend_key_required();
    case "No server was selected.":
      return compiledMessages.error_server_not_selected();
    case "This server no longer exists.":
      return compiledMessages.error_server_missing();
    case "The selected friend no longer exists.":
      return compiledMessages.error_friend_missing();
    case "Your profile is still loading.":
      return compiledMessages.error_profile_loading();
    case "Choose a 64×64 PNG skin before continuing.":
      return compiledMessages.error_skin_required();
    case "Turn on Friendolls in macOS Accessibility settings before continuing.":
      return compiledMessages.error_accessibility_required();
    case "Identification key is not valid base64url.":
      return compiledMessages.error_key_invalid_base64();
    case "Identification key must encode a 32-byte public key.":
      return compiledMessages.error_key_invalid_length();
    case "Identification key is not a valid Ed25519 public key.":
      return compiledMessages.error_key_invalid_ed25519();
    case "You cannot add your own identification key.":
      return compiledMessages.error_own_key();
    case "The selected image is not a local file":
      return compiledMessages.error_image_not_local();
    case "The selected image could not be read":
      return compiledMessages.error_image_unreadable();
    case "Choose an image file":
      return compiledMessages.error_image_choose();
    case "The selected image could not be decoded":
      return compiledMessages.error_image_decode();
    case "Image compression failed":
      return compiledMessages.error_image_compression();
    case "Image is still too detailed after compression":
      return compiledMessages.error_image_too_detailed();
    case "No connected relay can currently reach this friend":
      return compiledMessages.error_friend_unreachable();
    case "Friend is busy; try again":
      return compiledMessages.error_friend_busy();
    case "Relay rejected the interaction":
      return compiledMessages.error_interaction_rejected();
    case "Friend is no longer available":
      return compiledMessages.error_friend_unavailable();
    case "Skin must be a valid PNG image":
      return compiledMessages.error_skin_invalid_png();
    default:
      {
        const imageLimit =
          /^The selected image must be at most (\d+) MiB$/u.exec(value);
        if (imageLimit) {
          return compiledMessages.error_image_too_large({
            max: Number(imageLimit[1]),
          });
        }
        const skinDimensions = /^Skin must be (\d+)×(\d+) pixels$/u.exec(value);
        if (skinDimensions) {
          return compiledMessages.error_skin_dimensions({
            width: Number(skinDimensions[1]),
            height: Number(skinDimensions[2]),
          });
        }
      }
      if (getLocale() !== "en" && /[\u3400-\u9fff]/u.test(value)) return value;
      return getLocale() === "en" ? value : compiledMessages.error_unexpected();
  }
}
