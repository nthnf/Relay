<script lang="ts">
	import { ArrowRight, MessageSquareText, ShieldCheck, Zap } from '@lucide/svelte';

	let { authenticated = false }: { authenticated?: boolean } = $props();
</script>

<section class="relative overflow-hidden border-b border-warm-charcoal bg-abyss">
	<div class="absolute inset-0 bg-[radial-gradient(circle_at_75%_20%,rgba(0,217,146,0.18),transparent_34%),radial-gradient(circle_at_20%_70%,rgba(184,179,176,0.08),transparent_30%)]"></div>
	<div class="relative mx-auto grid min-h-[calc(100vh-4rem)] max-w-7xl items-center gap-10 px-5 py-20 md:px-8 lg:grid-cols-[minmax(0,1fr)_28rem]">
		<div>
			<p class="text-xs font-semibold uppercase tracking-[0.2em] text-mint">Realtime workspaces for focused teams</p>
			<h1 class="mt-5 max-w-4xl font-[system-ui,sans-serif] text-5xl font-semibold leading-none tracking-[-0.07em] text-snow md:text-7xl">Chat that feels fast, durable, and private.</h1>
			<p class="mt-6 max-w-2xl text-base leading-7 text-steel">Relay combines Discord-style team spaces with backend-first messaging contracts: durable chat history, workspace ownership, direct messages, and realtime delivery without making the socket the source of truth.</p>

			<div class="mt-8 flex flex-wrap gap-3">
				<a class="inline-flex items-center gap-2 rounded-md bg-signal px-5 py-3 text-sm font-semibold text-abyss hover:bg-mint" href={authenticated ? '/profile' : '/auth/signup'}>
					{authenticated ? 'Open Relay' : 'Start now'}
					<ArrowRight size={16} strokeWidth={2.4} />
				</a>
				<a class="rounded-md border border-warm-charcoal px-5 py-3 text-sm font-semibold text-snow hover:border-signal" href="/auth/login">Sign in</a>
			</div>
		</div>

		<div class="rounded-2xl border border-warm-charcoal bg-carbon p-4 shadow-[rgba(0,0,0,0.45)_0px_24px_80px]">
			<div class="rounded-xl border border-warm-charcoal bg-chat-main p-4">
				<div class="mb-5 flex items-center justify-between border-b border-warm-charcoal pb-3">
					<div>
						<p class="text-xs font-semibold uppercase tracking-[0.14em] text-mint">Relay Systems</p>
						<p class="mt-1 text-sm text-snow"># platform-ops</p>
					</div>
					<span class="h-2.5 w-2.5 rounded-full bg-signal"></span>
				</div>

				<div class="space-y-3">
					{@render Message('JM', 'Julie Mark', 'Deploy finished. Realtime fanout caught every active session.')}
					{@render Message('DA', 'Demo Agent', 'Durable write succeeded first, then the socket delivered it.')}
					{@render Message('MC', 'Monica Christine', 'Invite link is ready for the new workspace members.')}
				</div>
			</div>

			<div class="mt-4 grid gap-3 sm:grid-cols-3">
				{@render Stat(Zap, 'Low latency')}
				{@render Stat(ShieldCheck, 'Auth gated')}
				{@render Stat(MessageSquareText, 'Durable chat')}
			</div>
		</div>
	</div>
</section>

{#snippet Message(initials: string, name: string, body: string)}
	<div class="flex gap-3 rounded-lg p-2 hover:bg-warm-charcoal/25">
		<span class="grid h-9 w-9 shrink-0 place-items-center rounded-full border border-warm-charcoal bg-[radial-gradient(circle_at_35%_20%,var(--color-mint),var(--color-carbon)_70%)] text-[0.65rem] font-bold text-snow">{initials}</span>
		<div>
			<p class="text-xs font-semibold text-parchment">{name}</p>
			<p class="mt-1 text-xs leading-5 text-snow">{body}</p>
		</div>
	</div>
{/snippet}

{#snippet Stat(icon: typeof Zap, label: string)}
	{@const Icon = icon}
	<div class="rounded-lg border border-warm-charcoal bg-chat-main p-3 text-center text-xs font-semibold text-parchment">
		<Icon class="mx-auto mb-2 text-mint" size={18} strokeWidth={2} />
		{label}
	</div>
{/snippet}
