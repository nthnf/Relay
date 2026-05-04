<script lang="ts">
	import { ArrowRight } from '@lucide/svelte';
	import PrimarySidebar from '$lib/components/chat/PrimarySidebar.svelte';
	import ChannelSetupCard from '$lib/components/create-server/ChannelSetupCard.svelte';
	import ServerDetailsCard from '$lib/components/create-server/ServerDetailsCard.svelte';
	import type { ActionData, PageData } from './$types';

	let { data, form }: { data: PageData; form: ActionData } = $props();
</script>

<svelte:head><title>Create Server</title></svelte:head>

<div class="flex min-h-screen bg-abyss text-snow">
	<PrimarySidebar sidebar={data.sidebar} />

	<main class="min-h-screen min-w-0 flex-1 overflow-y-auto bg-chat-main px-4 py-8 md:px-6 md:py-10">
		<form method="POST" class="mx-auto grid min-h-[calc(100vh-5rem)] max-w-7xl grid-rows-[1fr_auto] gap-4">
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
</div>
