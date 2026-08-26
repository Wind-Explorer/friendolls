<script lang="ts">
  import {
    friendStatusesListenerError,
    onlineFriendIds,
  } from "$lib/listeners/friend-statuses";
  import { friends, friendsListenerError } from "$lib/listeners/friends";
</script>

<section>
  <h1>Friend statuses</h1>

  {#if $friendsListenerError || $friendStatusesListenerError}
    <p aria-live="polite">
      {$friendsListenerError || $friendStatusesListenerError}
    </p>
  {:else if $friends.length === 0}
    <p>No friends configured.</p>
  {:else}
    <ul aria-live="polite">
      {#each $friends as friend (friend.id)}
        {@const online = $onlineFriendIds.has(friend.id)}
        <li>
          <strong>{friend.displayName}</strong>: {online ? "Online" : "Offline"}
          <small>({friend.id})</small>
        </li>
      {/each}
    </ul>
  {/if}
</section>
