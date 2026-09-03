<script lang="ts">
  import { messages } from "$lib/i18n";
  import {
    containsScheme,
    pastedServerAddress,
    type ServerConnectionType,
  } from "$lib/server-endpoint";

  let {
    idPrefix,
    address = $bindable(),
    port = $bindable(),
    connectionType = $bindable(),
    disabled = false,
    required = false,
    labelClass = "fieldset-label",
  }: {
    idPrefix: string;
    address: string;
    port: string;
    connectionType: ServerConnectionType;
    disabled?: boolean;
    required?: boolean;
    labelClass?: string;
  } = $props();

  let hasTypedScheme = $derived(containsScheme(address));
  let portPlaceholder = $derived(
    connectionType === "https"
      ? "443"
      : connectionType === "http"
        ? "80"
        : "27520",
  );

  function pasteAddress(event: ClipboardEvent) {
    const pasted = pastedServerAddress(
      event.clipboardData?.getData("text") ?? "",
    );
    if (!pasted) return;
    event.preventDefault();
    address = pasted.address;
    connectionType = pasted.connectionType;
    port = pasted.port;
  }
</script>

<label class={`${labelClass} mt-1`} for={`${idPrefix}-address`}
  >{$messages.common_address()}</label
>
<div class="join w-full">
  <select
    id={`${idPrefix}-connection-type`}
    class="select join-item w-auto"
    bind:value={connectionType}
    {disabled}
    aria-label={$messages.common_connection_type()}
  >
    <option value="https">https://</option>
    <option value="http">http://</option>
    <option value="direct">{$messages.common_connection_direct()}</option>
  </select>
  <input
    id={`${idPrefix}-address`}
    class:input-error={hasTypedScheme}
    class="input join-item min-w-0 flex-1"
    bind:value={address}
    placeholder="example.net"
    autocomplete="off"
    {required}
    {disabled}
    onpaste={pasteAddress}
    aria-invalid={hasTypedScheme}
    aria-describedby={hasTypedScheme ? `${idPrefix}-address-error` : undefined}
  />
</div>
{#if hasTypedScheme}
  <p id={`${idPrefix}-address-error`} class="text-error text-xs">
    {$messages.error_server_address_scheme()}
  </p>
{/if}

<label class={`${labelClass} mt-1`} for={`${idPrefix}-port`}
  >{$messages.common_optional_port()}</label
>
<input
  id={`${idPrefix}-port`}
  class="input w-28"
  bind:value={port}
  type="number"
  min="1"
  max="65535"
  inputmode="numeric"
  placeholder={portPlaceholder}
  {disabled}
/>
