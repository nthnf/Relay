<script lang="ts">
	import { MessageCircle, Plus, Users } from '@lucide/svelte';

	type WorkspaceItem = {
		workspaceId: string;
		routeId: string;
		firstChannelRouteId?: string;
		name: string;
	};

	type DmItem = {
		dmPairId: string;
		routeId: string;
		peerDisplayName?: string;
		peerUsername?: string;
	};

	type Viewer = {
		displayName?: string;
		username?: string;
	};

	type SidebarData = {
		viewer?: Viewer;
		workspaces: WorkspaceItem[];
		dms: DmItem[];
		pendingFriendRequestCount: number;
	};

	let {
		sidebar,
		active = 'workspace',
		activeWorkspaceId
	}: { sidebar: SidebarData | null; active?: 'workspace' | 'dm' | 'friend'; activeWorkspaceId?: string } = $props();

	const serverAvatars = ['R', 'OK', 'C', 'M'];
</script>

<aside class="hidden h-screen w-[4.5rem] shrink-0 flex-col border-r border-warm-charcoal bg-abyss md:flex">
	<div class="flex min-h-0 flex-1 flex-col items-center py-5">
		<a
			class="grid h-10 w-10 place-items-center rounded-full bg-signal text-2xl font-medium leading-none text-abyss transition hover:bg-mint"
			href="/create-server"
			aria-label="Add server"
		>
			<Plus size={22} strokeWidth={2.5} />
		</a>

		<nav class="mt-6 flex w-full flex-col items-center gap-4 text-parchment" aria-label="Primary navigation">
			<a
				class={['relative flex w-full flex-col items-center gap-1 font-[Inter,system-ui,sans-serif] text-[0.65rem] font-medium leading-tight transition hover:text-snow', active === 'dm' && 'text-snow']}
				href={sidebar?.dms[0] ? `/dm/${sidebar.dms[0].routeId}` : '/friends'}
				aria-label="DMs"
			>
				<span class={['grid h-9 w-9 place-items-center rounded-full border bg-carbon text-snow', active === 'dm' ? 'border-signal' : 'border-warm-charcoal']}><MessageCircle size={17} strokeWidth={2.2} /></span>
				<span>DMs</span>
			</a>
			<a
				class={['relative flex w-full flex-col items-center gap-1 font-[Inter,system-ui,sans-serif] text-[0.65rem] font-medium leading-tight transition hover:text-snow', active === 'friend' && 'text-snow']}
				href="/friends"
				aria-label="Friends"
			>
				<span class={['grid h-9 w-9 place-items-center rounded-full border bg-carbon text-snow', active === 'friend' ? 'border-signal' : 'border-warm-charcoal']}><Users size={17} strokeWidth={2.2} /></span>
				<span>Friend</span>
			</a>
		</nav>

		<div class="my-5 h-px w-10 bg-warm-charcoal"></div>

		<div class="flex min-h-0 w-full flex-1 flex-col items-center gap-3 overflow-y-auto px-2 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
			{#if sidebar?.workspaces.length}
				{#each sidebar.workspaces as workspace, index (workspace.workspaceId)}
					<a
						class={['relative grid h-10 w-10 place-items-center rounded-full border border-warm-charcoal bg-[radial-gradient(circle_at_35%_20%,var(--color-mint),var(--color-carbon)_62%)] font-[system-ui,sans-serif] text-[0.65rem] font-bold leading-none text-snow transition hover:border-signal', active === 'workspace' && (workspace.workspaceId === activeWorkspaceId || (!activeWorkspaceId && index === 0)) && 'before:absolute before:-left-4 before:h-8 before:w-1 before:rounded-r-full before:bg-signal']}
						href={workspace.firstChannelRouteId ? `/workspace/${workspace.routeId}/${workspace.firstChannelRouteId}` : `/workspace/${workspace.routeId}`}
						aria-label={workspace.name}
					>
						<span>{workspace.name.slice(0, 2).toUpperCase()}</span>
					</a>
				{/each}
			{:else}
				{#each serverAvatars as avatar (avatar)}
					<div class="grid h-10 w-10 place-items-center rounded-full border border-warm-charcoal bg-carbon font-[system-ui,sans-serif] text-[0.65rem] font-bold leading-none text-steel"><span>{avatar}</span></div>
				{/each}
			{/if}
		</div>
	</div>

	<div class="mx-auto mb-5 w-full px-3">
		<div class="mb-4 h-px bg-warm-charcoal"></div>
		<a class="relative mx-auto grid h-10 w-10 place-items-center rounded-full border border-warm-charcoal bg-carbon font-[system-ui,sans-serif] text-sm font-bold leading-none text-snow" href="/profile" aria-label="Current user profile">
			<span>{sidebar?.viewer?.displayName?.slice(0, 1).toUpperCase() ?? 'U'}</span>
			<i class="absolute right-0 bottom-0 h-3 w-3 rounded-full border-2 border-abyss bg-signal"></i>
		</a>
	</div>
</aside>
