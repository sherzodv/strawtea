<script lang="ts">
  import { BookOpen, ReceiptText } from '@lucide/svelte';
  import { route } from '../../lib/router';
</script>

<section class="stea-menu">
  <div class="stea-menu-grid" aria-label="Feature menu">
    <button class="stea-app-tile" type="button" on:click={() => route.navigate('/investlog')}>
      <span class="stea-app-icon">
        <BookOpen size={42} strokeWidth={1.8} />
      </span>
      <span>Investlog</span>
    </button>

    <button class="stea-app-tile" type="button" on:click={() => route.navigate('/spends')}>
      <span class="stea-app-icon">
        <ReceiptText size={42} strokeWidth={1.8} />
      </span>
      <span>Spends</span>
    </button>
  </div>
</section>
