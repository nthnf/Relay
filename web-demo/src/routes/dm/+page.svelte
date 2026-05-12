<script lang="ts">
	import { resolve } from '$app/paths';
	import { MessageCircle, UserPlus } from '@lucide/svelte';
	import DmSidebar from '$lib/components/chat/DmSidebar.svelte';
	import PrimarySidebar from '$lib/components/chat/PrimarySidebar.svelte';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
</script>

<svelte:head><title>Direct messages - Relay</title></svelte:head>

<div class="flex min-h-screen bg-abyss text-snow">
	<PrimarySidebar sidebar={data.sidebar} active="dm" />
	<DmSidebar threads={data.sidebar?.dms ?? []} />

	<main class="grid min-h-screen min-w-0 flex-1 place-items-center bg-chat-main px-6 py-10">
		<section class="w-full max-w-xl rounded-2xl border border-warm-charcoal bg-carbon p-8 text-center">
			<div class="mx-auto grid h-16 w-16 place-items-center rounded-full border border-warm-charcoal bg-[radial-gradient(circle_at_35%_20%,var(--color-mint),var(--color-carbon)_68%)] text-snow">
				<MessageCircle size={28} strokeWidth={2.2} />
			</div>
			<h1 class="mt-5 font-[system-ui,sans-serif] text-3xl font-semibold tracking-[-0.05em] text-snow">Direct messages</h1>
			<p class="mx-auto mt-3 max-w-sm text-sm leading-6 text-steel">Pick existing DM from sidebar, or add friend first to start private conversation.</p>
			<a class="mt-6 inline-flex items-center justify-center gap-2 rounded-md bg-signal px-5 py-3 text-sm font-semibold text-abyss hover:bg-mint" href={resolve('/friends')}>
				<UserPlus size={16} strokeWidth={2.2} />
				Find friends
			</a>
		</section>
	</main>
</div>
