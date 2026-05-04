<script lang="ts">
	import ChatMessageBlock from './ChatMessageBlock.svelte';

	type Message = {
		messageId: string;
		authorUserId: string;
		senderName: string;
		body: string;
		createdAt?: Date;
		deletedAt?: Date;
		outgoing?: boolean;
	};

	let {
		messages,
		variant = 'channel',
		showDatePill = false,
		currentUserId
	}: { messages: Message[]; variant?: 'channel' | 'dm'; showDatePill?: boolean; currentUserId?: string } = $props();
</script>

<section class="overflow-y-auto px-3 py-5 md:px-4">
	{#if messages.length === 0}
		<p class="rounded-md border border-dashed border-dashed-slate/40 bg-carbon p-5 text-center text-sm text-steel">No messages yet.</p>
	{:else}
		<div class="space-y-4">
			{#if showDatePill}
				<div class="mx-auto w-fit rounded-full border border-warm-charcoal bg-carbon px-4 py-1.5 text-xs text-parchment">Today</div>
			{/if}
			{#each messages as message (message.messageId)}
				<ChatMessageBlock {message} {variant} {currentUserId} accent={variant === 'dm' && message.outgoing ? 'right' : 'left'} />
			{/each}
		</div>
	{/if}
</section>
