<script lang="ts">
	let { authenticated = false }: { authenticated?: boolean } = $props();
</script>

<header class="sticky top-0 z-20 border-b border-warm-charcoal/80 bg-abyss/85 backdrop-blur">
	<nav class="mx-auto flex h-16 max-w-7xl items-center justify-between px-5 md:px-8" aria-label="Main navigation">
		<a class="flex items-center gap-3" href="/">
			<span class="grid h-9 w-9 place-items-center rounded-md border border-signal bg-carbon font-[system-ui,sans-serif] text-sm font-bold text-mint shadow-[0_0_16px_rgba(0,217,146,0.16)]">R</span>
			<span class="font-[system-ui,sans-serif] text-lg font-semibold tracking-[-0.05em] text-snow">Relay</span>
		</a>

		<div class="flex items-center gap-2">
			{#if authenticated}
				<a class="rounded-md border border-warm-charcoal px-4 py-2 text-sm font-semibold text-snow hover:border-signal" href="/profile">Open Relay</a>
			{:else}
				<a class="rounded-md px-4 py-2 text-sm font-semibold text-parchment hover:text-snow" href="/auth/login">Sign in</a>
				<a class="rounded-md bg-signal px-4 py-2 text-sm font-semibold text-abyss hover:bg-mint" href="/auth/signup">Create account</a>
			{/if}
		</div>
	</nav>
</header>
