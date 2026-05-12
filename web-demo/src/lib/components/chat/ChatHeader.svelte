<script lang="ts">
	import { Search, Users } from '@lucide/svelte';

	type Props = {
		title: string;
		subtitle?: string;
		kicker?: string;
		avatarText?: string;
		variant?: 'channel' | 'dm';
		onToggleMembers?: () => void;
	};

	let {
		title,
		subtitle,
		kicker,
		avatarText,
		variant = 'channel',
		onToggleMembers
	}: Props = $props();
</script>

<header class={['flex items-center justify-between border-b border-warm-charcoal px-4 py-3 md:px-6', variant === 'dm' ? 'bg-carbon' : 'bg-chat-main']}>
	<div class="flex min-w-0 items-center gap-3">
		{#if avatarText}
			<span class="grid h-9 w-9 shrink-0 place-items-center rounded-full border border-warm-charcoal bg-[radial-gradient(circle_at_35%_20%,var(--color-parchment),var(--color-carbon)_70%)] font-[system-ui,sans-serif] text-xs font-bold text-snow">
				{avatarText.slice(0, 1).toUpperCase()}
			</span>
		{/if}
		<div class="min-w-0">
			{#if kicker}
				<p class="text-[0.65rem] font-semibold uppercase tracking-[0.18em] text-signal lg:hidden">{kicker}</p>
			{/if}
			<h1 class="truncate font-[system-ui,sans-serif] text-lg leading-none tracking-[-0.04em] text-snow {variant === 'dm' ? 'font-normal' : 'font-semibold'}">
				{title}
			</h1>
			{#if subtitle}
				<p class="mt-1 truncate text-xs text-parchment">{subtitle}</p>
			{/if}
		</div>
	</div>
	<div class="flex items-center gap-3 text-lg text-parchment">
		{#if variant === 'channel' && onToggleMembers}
			<button class="grid h-8 w-8 place-items-center rounded-full transition hover:bg-warm-charcoal/60 hover:text-mint" type="button" onclick={onToggleMembers} aria-label="Toggle member list">
				<Users size={18} strokeWidth={2} />
			</button>
		{/if}
		<Search size={18} strokeWidth={2} />
	</div>
</header>
