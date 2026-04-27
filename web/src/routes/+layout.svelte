<script lang="ts">
	import './layout.css';
	import favicon from '$lib/assets/favicon.svg';
	import type { Snippet } from 'svelte';
	import type { LayoutData } from './$types';

	let { children, data }: { children: Snippet; data: LayoutData } = $props();
</script>

<svelte:head><link rel="icon" href={favicon} /></svelte:head>

{#if data.sidebar}
	<div class="grid min-h-screen grid-cols-[16rem_1fr] bg-slate-950 text-slate-100 max-md:grid-cols-1">
		<aside class="border-r border-white/10 bg-slate-950/95 p-4 max-md:border-b max-md:border-r-0">
			<a class="mb-5 block rounded-2xl bg-white/10 p-4" href="/">
				<p class="text-xs uppercase tracking-[0.2em] text-cyan-300">Relay</p>
				<p class="truncate font-semibold">{data.sidebar.viewer?.displayName ?? 'Signed in'}</p>
			</a>

			<nav class="space-y-6 text-sm">
				<section>
					<div class="mb-2 flex items-center justify-between text-xs uppercase tracking-[0.16em] text-slate-500">
						<span>Home</span>
					</div>
					<a class="block rounded-xl px-3 py-2 hover:bg-white/10" href="/friends">
						Friends ({data.sidebar.pendingFriendRequestCount} pending)
					</a>
				</section>

				<section>
					<div class="mb-2 flex items-center justify-between text-xs uppercase tracking-[0.16em] text-slate-500">
						<span>Workspaces</span>
					</div>
					{#each data.sidebar.workspaces as workspace (workspace.workspaceId)}
						<a class="block truncate rounded-xl px-3 py-2 hover:bg-white/10" href={`/workspace/${workspace.routeId}`}>
							{workspace.name}
						</a>
					{/each}
				</section>

				<section>
					<div class="mb-2 flex items-center justify-between text-xs uppercase tracking-[0.16em] text-slate-500">
						<span>DMs</span>
					</div>
					{#each data.sidebar.dms as dm (dm.dmPairId)}
						<a class="block truncate rounded-xl px-3 py-2 hover:bg-white/10" href={`/dm/${dm.routeId}`}>
							{dm.peerDisplayName || dm.peerUsername}
						</a>
					{/each}
				</section>
			</nav>
		</aside>

		<div class="min-w-0">{@render children()}</div>
	</div>
{:else}
	{@render children()}
{/if}
