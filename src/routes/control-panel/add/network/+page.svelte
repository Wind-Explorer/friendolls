<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { commands, type RemoteInput } from "$lib/bindings";
  import PanelMessage from "$lib/components/control-panel/panel-message.svelte";
  import ServerEndpointFields from "$lib/components/server-endpoint-fields.svelte";
  import { errorMessage, messages } from "$lib/i18n";
  import {
    containsScheme,
    storedServerAddress,
    type ServerConnectionType,
  } from "$lib/server-endpoint";

  let name = $state("");
  let address = $state("");
  let port = $state("");
  let connectionType = $state<ServerConnectionType>("https");
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
    if (containsScheme(address)) {
      error = $messages.error_server_address_scheme();
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
      address: storedServerAddress(address, connectionType),
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
      <ServerEndpointFields
        idPrefix="remote"
        bind:address
        bind:port
        bind:connectionType
        required
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
