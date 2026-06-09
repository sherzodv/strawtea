<script lang="ts">
  import { Mail } from '@lucide/svelte';
  import { hasAuthConfig, signInWithGoogle } from '../../lib/auth';
</script>

<main class="login-screen">
  <section class="login-panel">
    <div class="brand-mark">S</div>
    <h1>Strawtea</h1>
    <p>Personal finance and automation, starting with stocks.</p>
    {#if !hasAuthConfig}
      <p class="error">Set VITE_SUPABASE_URL and VITE_SUPABASE_ANON_KEY to enable login.</p>
    {/if}
    <button class="primary-button" type="button" disabled={!hasAuthConfig} on:click={signInWithGoogle}>
      <Mail size={20} />
      Continue with Gmail
    </button>
  </section>
</main>
