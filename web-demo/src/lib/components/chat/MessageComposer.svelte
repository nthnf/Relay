<script lang="ts">
	let { error, body = '', placeholder = 'Type your message...' }: { error?: string; body?: string; placeholder?: string } = $props();
</script>

<form method="POST" action="?/send" class="border-t border-warm-charcoal bg-chat-main p-4 md:p-6">
	{#if error}
		<p class="mb-3 rounded-md border border-red-400/30 bg-red-500/10 px-3 py-2 text-xs text-red-200">{error}</p>
	{/if}

	<label class="sr-only" for="body">Message</label>
	<div class="mx-auto flex w-full max-w-6xl flex-col overflow-hidden rounded-lg border border-warm-charcoal bg-carbon focus-within:border-signal">
		<textarea id="body" name="body" rows="2" class="min-h-16 flex-1 resize-none bg-transparent px-4 py-2.5 font-[Inter,system-ui,sans-serif] text-sm text-snow outline-none placeholder:text-steel" {placeholder}>{body}</textarea>
		<div class="flex items-center justify-between gap-3 border-t border-warm-charcoal px-3 py-2">
			<div class="flex items-center gap-2 text-[0.68rem] text-steel">
				<span>⇧ Shift</span>
				<span>+</span>
				<span>↵ Enter for newline</span>
			</div>
			<button class="rounded-md bg-carbon px-4 py-2 font-[Inter,system-ui,sans-serif] text-[0.68rem] font-semibold text-mint outline outline-1 outline-mint hover:bg-black/20" type="submit">Send</button>
		</div>
	</div>
</form>
