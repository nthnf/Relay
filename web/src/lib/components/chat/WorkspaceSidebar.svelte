<script lang="ts">
	import { resolve } from '$app/paths';
	import { Hash, Plus, Search, UserPlus } from '@lucide/svelte';
	import SecondarySidebar from './SecondarySidebar.svelte';

	type Channel = {
		channelId: string;
		routeId?: string;
		name: string;
		channelKind?: string;
	};

	type Workspace = {
		name: string;
	};

	let {
		workspace,
		workspaceRouteId,
		channels,
		activeChannelId,
		onAddMember,
		onCreateChannel
	}: {
		workspace: Workspace;
		workspaceRouteId: string;
		channels: Channel[];
		activeChannelId?: string;
		onAddMember?: () => void;
		onCreateChannel?: () => void;
	} = $props();

	let channelSearch = $state('');
	const firstChannelRouteId = $derived(channels[0]?.routeId ?? channels[0]?.channelId);
	const filteredChannels = $derived(
		channels.filter((channel) => channel.name.toLowerCase().includes(channelSearch.trim().toLowerCase()))
	);
</script>

<SecondarySidebar>
	<div>
		<div class="flex min-w-0 items-center justify-between gap-2">
			<a href={resolve(firstChannelRouteId ? `/workspace/${workspaceRouteId}/${firstChannelRouteId}` : `/workspace/${workspaceRouteId}`)} class="min-w-0 flex-1">
				<p class="truncate text-xl font-semibold tracking-[-0.04em] text-snow">{workspace.name}</p>
			</a>
			{#if onAddMember}
				<button class="grid h-7 w-7 shrink-0 place-items-center rounded-full text-parchment transition hover:bg-warm-charcoal/60 hover:text-mint" type="button" onclick={onAddMember} aria-label="Add workspace member">
					<UserPlus size={16} strokeWidth={2.2} />
				</button>
			{/if}
		</div>
		<a href={resolve(firstChannelRouteId ? `/workspace/${workspaceRouteId}/${firstChannelRouteId}` : `/workspace/${workspaceRouteId}`)} class="block">
			<p class="mt-1.5 text-xs text-parchment">Engineering channels</p>
		</a>
	</div>

	<label class="mt-5 flex items-center gap-3 rounded-md border border-warm-charcoal bg-carbon px-3 py-2.5 text-steel focus-within:border-signal">
		<span class="sr-only">Search channel</span>
		<input bind:value={channelSearch} class="min-w-0 flex-1 bg-transparent text-xs text-snow outline-none placeholder:text-steel" placeholder="Search Channel" />
		<Search size={16} strokeWidth={2} />
	</label>

	<div class="my-5 h-px bg-warm-charcoal"></div>

	<div class="mb-3 flex items-center justify-between text-xs font-medium text-parchment">
		<span>Text Channels</span>
		{#if onCreateChannel}
			<button class="grid h-7 w-7 place-items-center rounded-full text-steel transition hover:bg-warm-charcoal/60 hover:text-mint" type="button" onclick={onCreateChannel} aria-label="Create channel">
				<Plus size={16} strokeWidth={2} />
			</button>
		{:else}
			<Plus size={16} strokeWidth={2} class="text-steel" />
		{/if}
	</div>

	<nav class="space-y-1.5" aria-label="Workspace channels">
		{#each filteredChannels as channel (channel.channelId)}
			<a
				class={['flex items-center gap-2.5 rounded-md px-1 py-2.5 font-[Inter,system-ui,sans-serif] text-xs font-semibold leading-tight text-parchment transition hover:bg-chat-main hover:text-snow', activeChannelId === channel.channelId && 'bg-chat-main text-snow']}
				href={resolve(`/workspace/${workspaceRouteId}/${channel.routeId ?? channel.channelId}`)}
			>
				<span class="h-1.5 w-1.5 rounded-full bg-signal"></span>
				<Hash size={16} strokeWidth={2.4} class="text-snow" />
				<span class="truncate">{channel.name}</span>
			</a>
		{/each}
	</nav>
</SecondarySidebar>
