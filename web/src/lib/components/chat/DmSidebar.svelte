<script lang="ts">
	import { Search, UserPlus } from '@lucide/svelte';
	import SecondarySidebar from './SecondarySidebar.svelte';

	type DmThread = {
		dmPairId: string;
		routeId: string;
		peerDisplayName?: string;
		peerUsername?: string;
	};

	let { threads, activeDmPairId }: { threads: DmThread[]; activeDmPairId?: string } = $props();
</script>

<SecondarySidebar>
	<div class="flex items-center gap-3">
		<label class="flex min-w-0 max-w-48 flex-1 items-center gap-3 rounded-md border border-warm-charcoal bg-carbon px-3 py-2.5 text-steel focus-within:border-signal">
			<span class="sr-only">Search conversations</span>
			<input class="min-w-0 flex-1 bg-transparent text-xs text-snow outline-none placeholder:text-steel" placeholder="Search Conversations" />
			<Search size={16} strokeWidth={2} />
		</label>
		<a class="grid h-10 w-10 shrink-0 place-items-center rounded-md border border-warm-charcoal text-xl text-parchment hover:border-signal hover:text-snow" href="/friends" aria-label="Add conversation">
			<UserPlus size={18} strokeWidth={2.2} />
		</a>
	</div>

	<nav class="mt-6 space-y-2" aria-label="Direct messages">
		{#each threads as thread, index (thread.dmPairId)}
			<a class={['flex items-center gap-3 rounded-md border border-transparent p-2.5 text-parchment transition hover:border-warm-charcoal hover:bg-chat-main', activeDmPairId === thread.dmPairId && 'border-warm-charcoal bg-chat-main']} href={`/dm/${thread.routeId}`}>
				<span class="grid h-10 w-10 shrink-0 place-items-center rounded-full border border-warm-charcoal bg-[radial-gradient(circle_at_35%_20%,var(--color-parchment),var(--color-carbon)_70%)] font-[system-ui,sans-serif] text-xs font-bold leading-none text-snow">{(thread.peerDisplayName || thread.peerUsername || '?').slice(0, 1).toUpperCase()}</span>
				<span class="min-w-0 flex-1">
					<strong class="block truncate font-[system-ui,sans-serif] text-sm leading-tight font-medium tracking-[-0.03em] text-snow">{thread.peerDisplayName || thread.peerUsername || `Conversation ${index + 1}`}</strong>
					<small class="font-[Inter,system-ui,sans-serif] text-xs leading-snug text-steel">Last active 10.20 PM</small>
				</span>
				<time class="font-[Inter,system-ui,sans-serif] text-xs leading-snug text-steel">10.20 PM</time>
			</a>
		{/each}
	</nav>
</SecondarySidebar>
