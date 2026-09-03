<script lang="ts">
  import type { PuppetMovementMode } from "$lib/bindings";
  import { messages } from "$lib/i18n";

  let {
    movementMode = $bindable(),
    busy,
  }: { movementMode: PuppetMovementMode; busy: boolean } = $props();
</script>

<fieldset class="fieldset border border-base-300 bg-base-100 p-4">
  <legend class="fieldset-legend px-1">{$messages.puppet_movement()}</legend>
  <div class="grid grid-cols-2 gap-3">
    {#each [{ id: "free" as const, image: "/puppet-movement-free.svg", alt: $messages.puppet_free_alt(), label: $messages.puppet_free() }, { id: "bottom" as const, image: "/puppet-movement-bottom.svg", alt: $messages.puppet_bottom_alt(), label: $messages.puppet_bottom() }] as option (option.id)}
      <label
        class="card cursor-pointer overflow-hidden border border-base-300 bg-base-200"
        class:border-primary={movementMode === option.id}
      >
        <img
          src={option.image}
          alt={option.alt}
          class="w-full bg-linear-to-b from-base-100 to-base-300 object-cover"
        />
        <span class="card-body flex-row items-center gap-2 p-3">
          <input
            class="radio scale-50 radio-primary"
            type="radio"
            name="movement-mode"
            value={option.id}
            bind:group={movementMode}
            disabled={busy}
          />
          <span class="text-xs font-bold">{option.label}</span>
        </span>
      </label>
    {/each}
  </div>
</fieldset>
