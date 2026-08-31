<script lang="ts">
  import { page } from "$app/stores";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount } from "svelte";
  import { commands, type RemoteInput } from "$lib/bindings";
  import PanelMessage from "$lib/components/control-panel/panel-message.svelte";

  let name = $state("");
  let address = $state("");
  let port = $state("");
  let loading = $state(true);
  let busy = $state(false);
  let available = $state(false);
  let error = $state("");

  const remoteId = $derived($page.params.id ?? "");

  onMount(() => {
    let active = true;

    if (!remoteId) {
      error = "No server was selected.";
      loading = false;
      return;
    }

    void commands
      .getRemote(remoteId)
      .then((remote) => {
        if (!active) return;
        if (!remote) {
          error = "This server no longer exists.";
          return;
        }
        name = remote.name ?? "";
        address = remote.address;
        port = remote.port?.toString() ?? "";
        available = true;
      })
      .catch((cause) => {
        if (active) error = String(cause);
      })
      .finally(() => {
        if (active) loading = false;
      });

    return () => {
      active = false;
    };
  });

  async function closeWindow() {
    await getCurrentWindow().close();
  }

  function remoteInput(): RemoteInput | null {
    const parsedPort = port ? Number(port) : null;
    if (!address.trim()) {
      error = "Server address is required.";
      return null;
    }
    if (
      parsedPort !== null &&
      (!Number.isInteger(parsedPort) || parsedPort < 1 || parsedPort > 65535)
    ) {
      error = "Port must be a whole number from 1 to 65535.";
      return null;
    }
    return {
      address: address.trim(),
      name: name.trim() || null,
      port: parsedPort,
    };
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    error = "";
    const remote = remoteInput();
    if (!remote || !available) return;

    busy = true;
    try {
      const updated = await commands.updateRemote(remoteId, remote);
      if (!updated) {
        error = "This server no longer exists.";
        available = false;
        busy = false;
        return;
      }
      await closeWindow();
    } catch (cause) {
      error = String(cause);
      busy = false;
    }
  }
</script>

<svelte:head><title>Edit Server</title></svelte:head>
<svelte:window
  onkeydown={(event) => event.key === "Escape" && !busy && void closeWindow()}
/>

<form
  class="flex h-full flex-col bg-base-100 p-3 text-base-content"
  onsubmit={submit}
>
  <div class="min-h-0 flex-1 space-y-2">
    <div>
      <h1 class="text-sm font-bold">Edit server</h1>
      <p class="text-xs text-base-content/65">
        Update how Friendolls connects to this remote server.
      </p>
    </div>

    {#if error}<PanelMessage kind="error">{error}</PanelMessage>{/if}

    <fieldset
      class="fieldset border border-base-300 bg-base-100 p-3"
      disabled={loading || busy || !available}
    >
      <legend class="fieldset-legend px-1">Server details</legend>
      <label class="fieldset-label" for="remote-name"
        >Friendly name (optional)</label
      >
      <input
        id="remote-name"
        class="input w-full"
        bind:value={name}
        maxlength="64"
        autocomplete="off"
      />
      <label class="fieldset-label mt-1" for="remote-address">Address</label>
      <input
        id="remote-address"
        class="input w-full"
        bind:value={address}
        placeholder="example.net"
        autocomplete="off"
        required
      />
      <label class="fieldset-label mt-1" for="remote-port"
        >Port (optional)</label
      >
      <input
        id="remote-port"
        class="input w-28"
        bind:value={port}
        type="number"
        min="1"
        max="65535"
        inputmode="numeric"
        placeholder="Default"
      />
      <p class="fieldset-label">Leave port blank to use the server default.</p>
    </fieldset>
  </div>

  <div class="flex shrink-0 justify-end gap-1.5 border-t border-base-300 pt-2">
    <button
      class="btn min-w-16"
      type="submit"
      disabled={loading || busy || !available}>{busy ? "Saving…" : "OK"}</button
    >
    <button
      class="btn min-w-16"
      type="button"
      disabled={busy}
      onclick={closeWindow}>Cancel</button
    >
  </div>
</form>
