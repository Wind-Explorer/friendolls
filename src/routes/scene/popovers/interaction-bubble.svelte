<script lang="ts">
  import { fly } from "svelte/transition";
  import type { FriendInteractionReceived } from "$lib/bindings";

  type Props = {
    interaction: FriendInteractionReceived;
    senderName: string;
    onDismiss: () => void;
    onOpenImage: (source: string) => void;
  };

  let { interaction, senderName, onDismiss, onOpenImage }: Props = $props();
  let imageSource = $derived(
    interaction.content.type === "image"
      ? `data:${interaction.content.mediaType};base64,${interaction.content.data}`
      : "",
  );
</script>

<aside
  class="scene-hitbox absolute bottom-[calc(100%+0.65rem)] left-1/2 z-30 w-36 -translate-x-1/2 overflow-hidden border border-base-300 bg-base-100/95 text-base-content shadow-lg backdrop-blur-sm"
  role="status"
  aria-live="polite"
  transition:fly={{ y: 8, duration: 180 }}
>
  <header class="flex items-center gap-1 border-b border-base-200 px-2 py-1">
    <span class="min-w-0 flex-1 truncate text-[0.65rem] font-medium">
      {senderName}
    </span>
    <button
      type="button"
      class="grid size-5 place-items-center text-xs text-base-content/50 transition-colors hover:text-base-content"
      aria-label="Dismiss interaction"
      onclick={onDismiss}
    >
      ×
    </button>
  </header>

  {#if interaction.content.type === "wave"}
    <div class="flex items-center justify-center gap-2 px-3 py-2 text-xs">
      <span
        class="origin-bottom-right animate-[wiggle_650ms_ease-in-out_2] text-xl"
        >👋</span
      >
      <span>waved</span>
    </div>
  {:else if interaction.content.type === "text"}
    <p
      class="max-h-24 overflow-auto whitespace-pre-wrap wrap-break-words px-2 py-2 text-xs leading-relaxed"
    >
      {interaction.content.text}
    </p>
  {:else}
    <button
      type="button"
      class="block w-full cursor-zoom-in bg-base-200"
      aria-label={`Open image from ${senderName}`}
      onclick={() => onOpenImage(imageSource)}
    >
      <img
        src={imageSource}
        alt={`Preview sent by ${senderName}`}
        class="h-24 w-full object-cover"
      />
      <span class="block px-2 py-1 text-[0.6rem] text-base-content/60">
        Click to enlarge
      </span>
    </button>
  {/if}
</aside>

<style>
  @keyframes wiggle {
    0%,
    100% {
      transform: rotate(0deg);
    }
    30% {
      transform: rotate(22deg);
    }
    65% {
      transform: rotate(-12deg);
    }
  }
</style>
