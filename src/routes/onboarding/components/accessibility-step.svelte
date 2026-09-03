<script lang="ts">
  import PanelMessage from "$lib/components/control-panel/panel-message.svelte";
  import { messages } from "$lib/i18n";

  let {
    permissionGranted,
    busy,
    onrequest,
  }: { permissionGranted: boolean; busy: boolean; onrequest: () => void } =
    $props();
</script>

<div class="space-y-4">
  <PanelMessage kind={permissionGranted ? "success" : "warning"}>
    {permissionGranted
      ? $messages.accessibility_granted()
      : $messages.accessibility_waiting()}
  </PanelMessage>
  <fieldset class="fieldset border border-base-300 bg-base-100 p-4">
    <legend class="fieldset-legend px-1">{$messages.accessibility_why()}</legend>
    <p class="text-xs leading-relaxed">{$messages.accessibility_explanation()}</p>
    <ol class="mt-3 list-decimal space-y-1 pl-5 text-xs">
      <li>{$messages.accessibility_step_open()}</li>
      <li>{$messages.accessibility_step_enable()}</li>
      <li>{$messages.accessibility_step_return()}</li>
    </ol>
    <button
      class="btn mt-4 w-fit"
      type="button"
      disabled={busy || permissionGranted}
      onclick={onrequest}
    >
      {permissionGranted
        ? $messages.accessibility_access_granted()
        : $messages.accessibility_open_settings()}
    </button>
  </fieldset>
</div>
