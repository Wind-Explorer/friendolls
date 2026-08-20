import { commands } from "$lib/bindings";

const HITBOX_SELECTOR = ".scene-hitbox";

export function startHitboxSync(): () => void {
  let resizeObserver: ResizeObserver;

  const syncHitboxes = () => {
    const elements = Array.from(document.querySelectorAll(HITBOX_SELECTOR));
    elements.forEach((element) => resizeObserver.observe(element));

    const hitboxes = elements.map((element) => {
      const { x, y, width, height } = element.getBoundingClientRect();
      return { x, y, width, height };
    });

    commands
      .updateSceneHitboxes(hitboxes)
      .catch((error) => console.error("Failed to update scene hitboxes", error));
  };

  resizeObserver = new ResizeObserver(syncHitboxes);
  const mutationObserver = new MutationObserver(syncHitboxes);
  mutationObserver.observe(document.body, {
    attributes: true,
    attributeFilter: ["class", "style"],
    childList: true,
    subtree: true,
  });
  window.addEventListener("resize", syncHitboxes);
  syncHitboxes();

  return () => {
    resizeObserver.disconnect();
    mutationObserver.disconnect();
    window.removeEventListener("resize", syncHitboxes);
    commands
      .updateSceneHitboxes([])
      .catch((error) => console.error("Failed to clear scene hitboxes", error));
  };
}
