<script lang="ts">
  import { onMount } from "svelte";
  import { fade, scale } from "svelte/transition";
  import { messages } from "$lib/i18n";

  type Props = {
    source: string;
    senderName: string;
    onClose: () => void;
  };

  let { source, senderName, onClose }: Props = $props();
  let closeButton: HTMLButtonElement;

  onMount(() => closeButton.focus({ preventScroll: true }));
</script>

<svelte:window onkeydown={(event) => event.key === "Escape" && onClose()} />

<div
  class="scene-hitbox fixed inset-0 z-50 grid place-items-center bg-black/55 p-4 backdrop-blur-sm"
  role="presentation"
  onclick={(event) => event.currentTarget === event.target && onClose()}
  transition:fade={{ duration: 150 }}
>
  <div
    class="relative grid max-h-full max-w-full place-items-center border border-white/20 bg-black/70 p-2 shadow-2xl"
    role="dialog"
    aria-modal="true"
    aria-label={$messages.image_from({ sender: senderName })}
    transition:scale={{ start: 0.96, duration: 180 }}
  >
    <img
      src={source}
      alt={$messages.image_sent_by({ sender: senderName })}
      class="max-h-[calc(100vh-3rem)] max-w-[calc(100vw-3rem)] object-contain"
    />
    <div
      class="absolute right-1 top-1 flex items-center gap-2 bg-black/70 px-1 py-1 text-white"
    >
      <span class="max-w-48 truncate pl-1 text-[0.65rem]">{senderName}</span>
      <button
        bind:this={closeButton}
        type="button"
        class="grid size-6 place-items-center text-lg leading-none hover:bg-white/15 focus-visible:outline focus-visible:outline-white"
        aria-label={$messages.image_close()}
        onclick={onClose}
      >
        ×
      </button>
    </div>
  </div>
</div>
