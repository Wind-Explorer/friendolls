import { commands } from "$lib/bindings";

export const DEFAULT_SKIN_URL = "/default-skin.png";

export async function resolveSkinSource(
  userId: string,
  skinHash: string | null,
) {
  if (!skinHash) return DEFAULT_SKIN_URL;

  try {
    const data = await commands.resolveSkin(userId, skinHash);
    return data ? `data:image/png;base64,${data}` : DEFAULT_SKIN_URL;
  } catch (error) {
    console.error(`Failed to resolve custom skin for ${userId}`, error);
    return DEFAULT_SKIN_URL;
  }
}
