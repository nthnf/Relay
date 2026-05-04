<script lang="ts">
	import { Edit3, Trash2 } from '@lucide/svelte';

	type Message = {
		messageId: string;
		authorUserId: string;
		senderName: string;
		body: string;
		createdAt?: Date;
		deletedAt?: Date;
		outgoing?: boolean;
	};

		type ContextMenuState = {
		open: boolean;
		x: number;
		y: number;
	};

	let {
		message,
		variant = 'channel',
		accent = 'left',
		currentUserId
	}: { message: Message; variant?: 'channel' | 'dm'; accent?: 'left' | 'right'; currentUserId?: string } = $props();
	let contextMenu = $state<ContextMenuState>({ open: false, x: 0, y: 0 });
	let editing = $state(false);
	let draft = $state('');
	let canManage = $derived(!message.deletedAt && Boolean(currentUserId) && currentUserId === message.authorUserId);

	function openContextMenu(event: MouseEvent) {
		if (!canManage) return;

		event.preventDefault();
		event.stopPropagation();

		contextMenu = {
			open: true,
			x: event.clientX,
			y: event.clientY
		};
	}

	function closeContextMenu() {
		if (!contextMenu.open) return;
		contextMenu = { ...contextMenu, open: false };
	}

	function startEditing() {
		draft = message.body;
		editing = true;
		closeContextMenu();
	}

	function stopEditing() {
		editing = false;
		draft = '';
	}

	function confirmDelete() {
		closeContextMenu();
		return window.confirm('Delete this message?');
	}

	function handleDocumentContextMenu() {
		closeContextMenu();
	}
</script>

<svelte:document onclick={closeContextMenu} oncontextmenu={handleDocumentContextMenu} />

<article
	class={[
		'flex w-full gap-3 rounded-md px-2 py-1.5 transition-colors hover:bg-warm-charcoal/25',
		variant === 'dm' && message.outgoing && 'justify-end'
	]}
	oncontextmenu={openContextMenu}
>
	{#if !(variant === 'dm' && message.outgoing)}
		<div class="hidden h-9 w-9 shrink-0 place-items-center rounded-full border border-warm-charcoal bg-[radial-gradient(circle_at_35%_20%,var(--color-mint),var(--color-carbon)_70%)] font-[system-ui,sans-serif] text-xs font-bold md:grid">
			{message.senderName.slice(0, 1).toUpperCase()}
		</div>
	{/if}

	<div class={[
		variant === 'dm' ? 'min-w-64 max-w-[70%]' : 'min-w-80 max-w-[70%]',
		'rounded-md border border-warm-charcoal bg-carbon p-4 shadow-[rgba(92,88,85,0.16)_0px_0px_12px]',
		accent === 'right' ? 'border-r-2 border-r-signal' : 'border-l-2 border-l-signal'
	]}>
		<div class="mb-2 flex items-center justify-between gap-3 text-[0.68rem] text-steel">
			<span class="font-[Inter,system-ui,sans-serif] font-semibold text-parchment">{message.senderName}</span>
			<span>{message.createdAt?.toLocaleString()}</span>
		</div>
		{#if editing}
			<form method="POST" action="?/edit" class="space-y-3">
				<input type="hidden" name="messageId" value={message.messageId} />
				<label class="sr-only" for={`edit-${message.messageId}`}>Edit message</label>
				<textarea
					id={`edit-${message.messageId}`}
					name="newBody"
					bind:value={draft}
					rows="3"
					class="w-full resize-none rounded-md border border-warm-charcoal bg-chat-main px-3 py-2 font-[Inter,system-ui,sans-serif] text-xs leading-6 text-snow outline-none focus:border-signal"
				></textarea>
				<div class="flex justify-end gap-2">
					<button type="button" class="rounded px-3 py-1.5 text-[0.68rem] font-semibold text-steel hover:bg-warm-charcoal/40 hover:text-snow" onclick={stopEditing}>Cancel</button>
					<button type="submit" class="rounded bg-signal px-3 py-1.5 text-[0.68rem] font-semibold text-abyss hover:bg-mint">Save</button>
				</div>
			</form>
		{:else}
			<p class="whitespace-pre-wrap font-[Inter,system-ui,sans-serif] text-xs leading-6 text-snow">{message.deletedAt ? 'Message deleted' : message.body}</p>
		{/if}
	</div>

	{#if variant === 'dm' && message.outgoing}
		<div class="hidden h-9 w-9 shrink-0 place-items-center rounded-full border border-warm-charcoal bg-[radial-gradient(circle_at_35%_20%,var(--color-mint),var(--color-carbon)_70%)] font-[system-ui,sans-serif] text-xs font-bold md:grid">
			{message.senderName.slice(0, 1).toUpperCase()}
		</div>
	{/if}
</article>

	{#if contextMenu.open && canManage}
	<div
		class="fixed z-50 min-w-44 overflow-hidden rounded-md border border-warm-charcoal bg-carbon/95 p-1 shadow-[rgba(0,0,0,0.35)_0px_12px_32px] backdrop-blur"
		style:left={`${contextMenu.x}px`}
		style:top={`${contextMenu.y}px`}
	>
		<button
			type="button"
			class="flex w-full items-center gap-2 rounded px-3 py-2 text-left text-xs font-medium text-parchment transition-colors hover:bg-warm-charcoal/40"
			onclick={startEditing}
		>
			<Edit3 class="size-4 text-steel" aria-hidden="true" />
			Edit message
		</button>
		<form method="POST" action="?/delete" onsubmit={confirmDelete}>
			<input type="hidden" name="messageId" value={message.messageId} />
			<button type="submit" class="flex w-full items-center gap-2 rounded px-3 py-2 text-left text-xs font-medium text-red-300 transition-colors hover:bg-warm-charcoal/40">
				<Trash2 class="size-4" aria-hidden="true" />
				Delete message
			</button>
		</form>
	</div>
{/if}
