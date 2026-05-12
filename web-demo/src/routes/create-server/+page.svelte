<script lang="ts">
	import { ArrowRight } from '@lucide/svelte';
	import PrimarySidebar from '$lib/components/chat/PrimarySidebar.svelte';
	import ChannelSetupCard from '$lib/components/create-server/ChannelSetupCard.svelte';
	import ServerDetailsCard from '$lib/components/create-server/ServerDetailsCard.svelte';
	import type { ActionData, PageData } from './$types';

	let { data, form }: { data: PageData; form: ActionData } = $props();
	let creating = $state(false);
</script>

<svelte:head><title>Create Server</title></svelte:head>

<div class="flex min-h-screen bg-abyss text-snow">
	<PrimarySidebar sidebar={data.sidebar} />

	<main class="min-h-screen min-w-0 flex-1 overflow-y-auto bg-chat-main px-4 py-8 md:px-6 md:py-10">
		<form method="POST" class="mx-auto grid min-h-[calc(100vh-5rem)] max-w-7xl grid-rows-[1fr_auto] gap-4" onsubmit={() => (creating = true)}>
			<div class="grid gap-4 md:grid-cols-2">
				<ServerDetailsCard error={form?.error} name={form?.name ?? ''} />
				<ChannelSetupCard firstChannelName={form?.firstChannelName ?? 'general'} />
			</div>

			<button class="flex w-full items-center justify-center gap-3 rounded-xl bg-signal px-5 py-4 text-sm font-semibold text-abyss hover:bg-mint" type="submit">
				Initialize Server
				<ArrowRight size={17} strokeWidth={2.4} />
			</button>
		</form>
	</main>

	{#if creating}
		<div class="fixed inset-0 z-50 grid place-items-center bg-abyss/80 backdrop-blur-sm" aria-live="polite" aria-label="Creating server">
			<div class="h-12 w-12 animate-spin rounded-full border-2 border-warm-charcoal border-t-signal"></div>
		</div>
	{/if}
</div>
