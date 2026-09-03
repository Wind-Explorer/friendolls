<script lang="ts">
  import { page } from "$app/stores";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount } from "svelte";
  import { commands, type RemoteInput } from "$lib/bindings";
  import PanelMessage from "$lib/components/control-panel/panel-message.svelte";
  import ServerEndpointFields from "$lib/components/server-endpoint-fields.svelte";
  import { errorMessage, messages } from "$lib/i18n";
  import {
    containsScheme,
    splitServerAddress,
    storedServerAddress,
    type ServerConnectionType,
  } from "$lib/server-endpoint";

  let name = $state("");
  let address = $state("");
  let port = $state("");
  let connectionType = $state<ServerConnectionType>("https");
  let loading = $state(true);
  let busy = $state(false);
  let available = $state(false);
  let error = $state("");

  const remoteId = $derived($page.params.id ?? "");

  $effect(() => {
    void getCurrentWindow().setTitle($messages.action_edit_server_title());
  });

  onMount(() => {
    let active = true;

    if (!remoteId) {
      error = $messages.error_server_not_selected();
      loading = false;
      return;
    }

    void commands
      .getRemote(remoteId)
      .then((remote) => {
        if (!active) return;
        if (!remote) {
          error = $messages.error_server_missing();
          return;
        }
        name = remote.name ?? "";
        const endpoint = splitServerAddress(remote.address);
        address = endpoint.address;
        connectionType = endpoint.connectionType;
        port = remote.port?.toString() ?? "";
        available = true;
      })
      .catch((cause) => {
        if (active) error = errorMessage(cause);
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
    if (!remote || !available) return;

    busy = true;
    try {
      const updated = await commands.updateRemote(remoteId, remote);
      if (!updated) {
        error = $messages.error_server_missing();
        available = false;
        busy = false;
        return;
      }
      await closeWindow();
    } catch (cause) {
      error = errorMessage(cause);
      busy = false;
    }
  }
</script>

<svelte:head><title>{$messages.action_edit_server_title()}</title></svelte:head>
<svelte:window
  onkeydown={(event) => event.key === "Escape" && !busy && void closeWindow()}
/>

<form
  class="flex h-full flex-col bg-base-100 p-3 text-base-content"
  onsubmit={submit}
>
  <div class="min-h-0 flex-1 space-y-2">
    <div>
      <h1 class="text-sm font-bold">{$messages.action_edit_server_heading()}</h1>
      <p class="text-xs text-base-content/65">
        {$messages.action_edit_server_help()}
      </p>
    </div>

    {#if error}<PanelMessage kind="error">{error}</PanelMessage>{/if}

    <fieldset
      class="fieldset border border-base-300 bg-base-100 p-3"
      disabled={loading || busy || !available}
    >
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
        disabled={loading || busy || !available}
        required
      />
    </fieldset>
  </div>

  <div class="flex shrink-0 justify-end gap-1.5 border-t border-base-300 pt-2">
    <button
      class="btn min-w-16"
      type="submit"
      disabled={loading || busy || !available}>{busy
        ? $messages.common_saving()
        : $messages.common_ok()}</button
    >
    <button
      class="btn min-w-16"
      type="button"
      disabled={busy}
      onclick={closeWindow}>{$messages.common_cancel()}</button
    >
  </div>
</form>
