<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { commands, type RemoteInput } from "$lib/bindings";
  import PanelMessage from "$lib/components/control-panel/panel-message.svelte";
  import { errorMessage, messages } from "$lib/i18n";

  let name = $state("");
  let address = $state("");
  let port = $state("");
  let busy = $state(false);
  let error = $state("");

  $effect(() => {
    void getCurrentWindow().setTitle($messages.action_add_server_title());
  });

  async function closeWindow() {
    await getCurrentWindow().close();
  }

  function remoteInput(): RemoteInput | null {
    const parsedPort = port ? Number(port) : null;
    if (!address.trim()) {
      error = $messages.error_server_address_required();
      return null;
    }
    if (
      parsedPort !== null &&
      (!Number.isInteger(parsedPort) || parsedPort < 1 || parsedPort > 65535)
    ) {
      error = $messages.error_port_invalid();
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
    if (!remote) return;

    busy = true;
    try {
      await commands.createRemote(remote);
      await closeWindow();
    } catch (cause) {
      error = errorMessage(cause);
      busy = false;
    }
  }
</script>

<svelte:head><title>{$messages.action_add_server_title()}</title></svelte:head>
<svelte:window
  onkeydown={(event) => event.key === "Escape" && !busy && void closeWindow()}
/>

<form
  class="flex h-full flex-col bg-base-100 p-3 text-base-content"
  onsubmit={submit}
>
  <div class="min-h-0 flex-1 space-y-2">
    {#if error}<PanelMessage kind="error">{error}</PanelMessage>{/if}

    <fieldset class="fieldset border border-base-300 bg-base-100 p-3">
      <legend class="fieldset-legend px-1">{$messages.common_server_details()}</legend>
      <label class="fieldset-label" for="remote-name"
        >{$messages.common_optional_friendly_name()}</label
      >
      <input
        id="remote-name"
        class="input w-full"
        bind:value={name}
        maxlength="64"
        autocomplete="off"
      />
      <label class="fieldset-label mt-1" for="remote-address">{$messages.common_address()}</label>
      <input
        id="remote-address"
        class="input w-full"
        bind:value={address}
        placeholder="example.net"
        autocomplete="off"
        required
      />
      <label class="fieldset-label mt-1" for="remote-port"
        >{$messages.common_optional_port()}</label
      >
      <input
        id="remote-port"
        class="input w-28"
        bind:value={port}
        type="number"
        min="1"
        max="65535"
        inputmode="numeric"
        placeholder="27520"
      />
    </fieldset>
  </div>

  <div class="flex shrink-0 justify-end gap-1.5 border-t border-base-300 pt-2">
    <button class="btn min-w-16" type="submit" disabled={busy}
      >{busy ? $messages.action_adding() : $messages.common_ok()}</button
    >
    <button
      class="btn min-w-16"
      type="button"
      disabled={busy}
      onclick={closeWindow}>{$messages.common_cancel()}</button
    >
  </div>
</form>
