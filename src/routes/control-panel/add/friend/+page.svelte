<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { commands } from "$lib/bindings";
  import PanelMessage from "$lib/components/control-panel/panel-message.svelte";
  import Info from "$lib/icons/info.svelte";

  let displayName = $state("");
  let friendId = $state("");
  let busy = $state(false);
  let error = $state("");

  async function closeWindow() {
    await getCurrentWindow().close();
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    const name = displayName.trim();
    const id = friendId.trim();
    if (!name || !id) {
      error = "Both display name and public key are required.";
      return;
    }

    busy = true;
    error = "";
    try {
      await commands.createFriend({ id, displayName: name });
      await closeWindow();
    } catch (cause) {
      error = String(cause);
      busy = false;
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
    <div>
      <h1 class="text-sm font-bold">Add a friend</h1>
      <p class="text-xs text-base-content/65">
        Enter the identity your friend shared with you.
      </p>
    </div>

    {#if error}<PanelMessage kind="error">{error}</PanelMessage>{/if}

    <fieldset class="fieldset border border-base-300 bg-base-100 p-3">
      <legend class="fieldset-legend px-1">Friend identity</legend>
      <label class="fieldset-label" for="friend-name">Display name</label>
      <input
        id="friend-name"
        class="input w-full"
        bind:value={displayName}
        maxlength="64"
        autocomplete="off"
        required
      />
      <label class="fieldset-label mt-1" for="friend-key"
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
        required></textarea>
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
