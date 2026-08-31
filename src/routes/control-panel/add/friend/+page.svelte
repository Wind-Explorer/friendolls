<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onDestroy } from "svelte";
  import { commands } from "$lib/bindings";
  import PanelMessage from "$lib/components/control-panel/panel-message.svelte";
  import Info from "$lib/icons/info.svelte";

  let friendId = $state("");
  let busy = $state(false);
  let error = $state("");
  let preview = $state<{
    status: "idle" | "loading" | "resolved" | "unresolved" | "unavailable";
    text: string;
  }>({ status: "idle", text: "" });
  let lookupTimer: ReturnType<typeof setTimeout> | undefined;
  let lookupGeneration = 0;

  onDestroy(() => {
    lookupGeneration += 1;
    if (lookupTimer) clearTimeout(lookupTimer);
  });

  async function closeWindow() {
    await getCurrentWindow().close();
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    const id = friendId.trim();
    if (!id) {
      error = "Identification key is required.";
      return;
    }

    busy = true;
    error = "";
    try {
      await commands.createFriend(id);
      await closeWindow();
    } catch (cause) {
      error = String(cause);
      busy = false;
    }
  }

  function scheduleProfilePreview(event: Event) {
    const id = (event.currentTarget as HTMLTextAreaElement).value.trim();
    const generation = ++lookupGeneration;
    if (lookupTimer) clearTimeout(lookupTimer);
    if (!id) {
      preview = { status: "idle", text: "" };
      return;
    }

    preview = { status: "loading", text: "Resolving…" };
    lookupTimer = setTimeout(
      () => void resolveProfilePreview(id, generation),
      250,
    );
  }

  async function resolveProfilePreview(id: string, generation: number) {
    try {
      const displayName = await commands.resolveFriendDisplayName(id);
      if (generation !== lookupGeneration) return;
      preview = displayName
        ? { status: "resolved", text: displayName }
        : { status: "unresolved", text: "Unresolved user" };
    } catch {
      if (generation === lookupGeneration) {
        preview = { status: "unavailable", text: "Unresolved" };
      }
    }
  }
</script>

<svelte:head><title>Add Friend</title></svelte:head>
<svelte:window
  onkeydown={(event) => event.key === "Escape" && !busy && void closeWindow()}
/>

<form
  class="flex h-full flex-col bg-base-100 p-3 text-base-content"
  onsubmit={submit}
>
  <div class="min-h-0 flex-1 space-y-2">
    {#if error}<PanelMessage kind="error">{error}</PanelMessage>{/if}

    <PanelMessage>
      Their display name will appear after Friendolls learns their profile from
      a shared server.
    </PanelMessage>

    <fieldset class="fieldset border border-base-300 bg-base-100 p-3">
      <legend class="fieldset-legend px-1">Friend identity</legend>
      <label class="fieldset-label" for="friend-key"
        >Identification Key <div
          class="tooltip tooltip-primary"
          data-tip="Verify with your friend before adding."
        >
          <div class="*:size-3">
            <Info />
          </div>
        </div></label
      >
      <textarea
        id="friend-key"
        class="textarea h-20 w-full resize-none font-mono text-[10px]"
        bind:value={friendId}
        autocomplete="off"
        oninput={scheduleProfilePreview}
        required></textarea>
      <label class="fieldset-label mt-1" for="friend-display-name"
        >Display name</label
      >
      <input
        id="friend-display-name"
        class:italic={preview.status !== "resolved"}
        class="input w-full"
        value={preview.text}
        aria-busy={preview.status === "loading"}
        readonly
      />
    </fieldset>
  </div>

  <div class="flex shrink-0 justify-end gap-1.5 border-t border-base-300 pt-2">
    <button class="btn min-w-16" type="submit" disabled={busy}
      >{busy ? "Adding…" : "OK"}</button
    >
    <button
      class="btn min-w-16"
      type="button"
      disabled={busy}
      onclick={closeWindow}>Cancel</button
    >
  </div>
</form>
