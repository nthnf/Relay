<script lang="ts">
	const features = [
		['Workspace-first', 'Servers, channels, DMs, friends, and member invites live in one focused shell.'],
		['Durable by design', 'Chat remains the source of truth while realtime delivery stays a fast optimization.'],
		['Presence-ready', 'Presence is owned by realtime and exposed through a dedicated API contract.']
	];
</script>

<section class="bg-chat-main px-5 py-16 md:px-8">
	<div class="mx-auto max-w-7xl">
		<p class="text-xs font-semibold uppercase tracking-[0.18em] text-mint">Why Relay</p>
		<div class="mt-5 grid gap-4 md:grid-cols-3">
			{#each features as feature (feature[0])}
				<article class="rounded-xl border border-warm-charcoal bg-carbon p-6">
					<h2 class="font-[system-ui,sans-serif] text-xl font-semibold tracking-[-0.04em] text-snow">{feature[0]}</h2>
					<p class="mt-3 text-sm leading-6 text-steel">{feature[1]}</p>
				</article>
			{/each}
		</div>
	</div>
</section>
