<script lang="ts">
	import { AlertTriangle, Building2, LogOut, Trash2 } from '@lucide/svelte';
	import PrimarySidebar from '$lib/components/chat/PrimarySidebar.svelte';
	import type { ActionData, PageData } from './$types';

	let { data, form }: { data: PageData; form: ActionData } = $props();

	const workspaceName = $derived(form?.name ?? data.workspace.name);
	const workspaceIconUrl = $derived(form?.iconUrl ?? data.workspace.iconUrl ?? '');
	const initial = $derived(workspaceName.slice(0, 2).toUpperCase());
	let confirmAction = $state<'leave' | 'delete' | null>(null);

	function closeConfirm() {
		confirmAction = null;
	}
</script>

<svelte:head><title>{workspaceName} settings - Relay</title></svelte:head>

<div class="flex min-h-screen bg-abyss text-snow">
	<PrimarySidebar sidebar={data.sidebar} active="workspace" activeWorkspaceId={data.workspace.workspaceId} />

	<main class="h-screen min-w-0 flex-1 bg-chat-main px-5 py-10">
		<div class="h-full overflow-y-auto pr-1">
			<section class="mx-auto w-full max-w-3xl overflow-hidden rounded-2xl border border-warm-charcoal bg-carbon">
				<div class="h-32 border-b border-warm-charcoal bg-[radial-gradient(circle_at_78%_0%,var(--color-signal)_0%,transparent_34%),linear-gradient(135deg,var(--color-carbon),var(--color-abyss))]"></div>

				<div class="px-7 pb-7">
					<div class="-mt-14 grid h-28 w-28 place-items-center rounded-3xl border-4 border-carbon bg-[radial-gradient(circle_at_35%_20%,var(--color-mint),var(--color-carbon)_68%)] font-[system-ui,sans-serif] text-3xl font-bold text-snow">
						{initial}
					</div>

					<div class="mt-5 flex flex-wrap items-end justify-between gap-4">
						<div>
							<h1 class="font-[system-ui,sans-serif] text-3xl font-semibold tracking-[-0.05em] text-snow">{workspaceName}</h1>
							<p class="mt-1 text-sm text-steel">Workspace settings</p>
						</div>
						<div class="rounded-full border border-warm-charcoal bg-chat-main px-3 py-1.5 text-xs font-semibold text-mint">
							{data.channels.length} channels
						</div>
					</div>

					<div class="mt-7 grid gap-4">
						<form method="POST" action="?/update" class="rounded-xl border border-warm-charcoal bg-chat-main p-5">
							<div class="flex items-center gap-3">
								<Building2 class="text-parchment" size={20} strokeWidth={2} />
								<h2 class="font-[system-ui,sans-serif] text-xl font-semibold tracking-[-0.04em] text-snow">Workspace profile</h2>
							</div>

							<div class="mt-5 grid gap-4">
								<label class="space-y-1.5" for="name">
									<span class="text-xs font-semibold uppercase tracking-[0.08em] text-parchment">Name</span>
									<input id="name" name="name" value={workspaceName} class="w-full rounded-md border border-warm-charcoal bg-carbon px-3 py-2.5 text-sm text-snow outline-none focus:border-mint" maxlength="80" required />
								</label>
								<label class="space-y-1.5" for="iconUrl">
									<span class="text-xs font-semibold uppercase tracking-[0.08em] text-parchment">Icon URL</span>
									<input id="iconUrl" name="iconUrl" value={workspaceIconUrl} class="w-full rounded-md border border-warm-charcoal bg-carbon px-3 py-2.5 text-sm text-snow outline-none focus:border-mint" placeholder="https://example.com/icon.png" />
								</label>
							</div>

							{#if form?.updateError}
								<p class="mt-4 rounded-lg border border-red-300/30 bg-red-500/10 px-3 py-2 text-sm text-red-100">{form.updateError}</p>
							{:else if form?.updated}
								<p class="mt-4 rounded-lg border border-mint/30 bg-mint/10 px-3 py-2 text-sm text-mint">Workspace settings saved.</p>
							{/if}

							<button class="mt-5 rounded-md bg-mint px-4 py-2.5 text-sm font-semibold text-abyss transition hover:bg-parchment" type="submit">Save changes</button>
						</form>

						{#if data.isOwner}
							<section class="rounded-xl border border-red-400/30 bg-red-500/10 p-5">
								<div class="flex items-center gap-3">
									<AlertTriangle class="text-red-200" size={20} strokeWidth={2} />
									<h2 class="font-[system-ui,sans-serif] text-xl font-semibold tracking-[-0.04em] text-red-100">Delete workspace</h2>
								</div>
								<p class="mt-3 text-sm leading-6 text-red-100/80">Archive this workspace for every member and remove it from downstream projections after events are processed.</p>
								{#if form?.deleteError}
									<p class="mt-4 rounded-lg border border-red-300/30 bg-red-500/10 px-3 py-2 text-sm text-red-100">{form.deleteError}</p>
								{/if}
								<button class="mt-5 inline-flex items-center gap-2 rounded-md border border-red-300/30 px-4 py-2.5 text-sm font-semibold text-red-100 transition hover:border-red-200 hover:bg-red-400/10" type="button" onclick={() => (confirmAction = 'delete')}>
									<Trash2 size={16} strokeWidth={2} />
									Delete workspace
								</button>
							</section>
						{:else}
							<section class="rounded-xl border border-red-400/30 bg-red-500/10 p-5">
								<div class="flex items-center gap-3">
									<AlertTriangle class="text-red-200" size={20} strokeWidth={2} />
									<h2 class="font-[system-ui,sans-serif] text-xl font-semibold tracking-[-0.04em] text-red-100">Leave workspace</h2>
								</div>
								<p class="mt-3 text-sm leading-6 text-red-100/80">Remove yourself from this workspace and hide it from your sidebar.</p>
								{#if form?.leaveError}
									<p class="mt-4 rounded-lg border border-red-300/30 bg-red-500/10 px-3 py-2 text-sm text-red-100">{form.leaveError}</p>
								{/if}
								<button class="mt-5 inline-flex items-center gap-2 rounded-md border border-red-300/30 px-4 py-2.5 text-sm font-semibold text-red-100 transition hover:border-red-200 hover:bg-red-400/10" type="button" onclick={() => (confirmAction = 'leave')}>
									<LogOut size={16} strokeWidth={2} />
									Leave workspace
								</button>
							</section>
						{/if}
					</div>
				</div>
			</section>
		</div>
	</main>
</div>

{#if confirmAction}
	<div class="fixed inset-0 z-50 flex items-center justify-center px-4 py-6">
		<button class="absolute inset-0 bg-abyss/80 backdrop-blur-sm" type="button" aria-label="Cancel confirmation" onclick={closeConfirm}></button>
		<div class="relative w-full max-w-md overflow-hidden rounded-2xl border border-red-300/30 bg-carbon shadow-[rgba(0,0,0,0.45)_0px_24px_80px]" role="dialog" aria-modal="true" aria-labelledby="confirm-title">
			<div class="absolute inset-0 bg-[radial-gradient(circle_at_80%_0%,rgba(248,113,113,0.22)_0%,transparent_35%)]"></div>
			<div class="relative p-6">
				<div class="flex items-center gap-3">
					<span class="grid h-11 w-11 place-items-center rounded-xl border border-red-300/30 bg-red-500/10 text-red-100">
						{#if confirmAction === 'delete'}
							<Trash2 size={21} strokeWidth={2.1} />
						{:else}
							<LogOut size={21} strokeWidth={2.1} />
						{/if}
					</span>
					<div>
						<p class="text-xs font-semibold uppercase tracking-[0.16em] text-red-200">Confirm action</p>
						<h2 id="confirm-title" class="mt-1 font-[system-ui,sans-serif] text-2xl font-semibold tracking-[-0.05em] text-snow">
							{confirmAction === 'delete' ? 'Delete workspace?' : 'Leave workspace?'}
						</h2>
					</div>
				</div>

				<p class="mt-5 text-sm leading-6 text-parchment">
					{#if confirmAction === 'delete'}
						This archives {workspaceName} for all members. You will be redirected away after the workspace disappears from your sidebar.
					{:else}
						You will lose access to {workspaceName}. You can only return if another member invites you again.
					{/if}
				</p>

				<div class="mt-6 flex justify-end gap-2">
					<button class="rounded-md border border-warm-charcoal px-4 py-2.5 text-sm font-semibold text-snow transition hover:border-signal hover:text-mint" type="button" onclick={closeConfirm}>Cancel</button>
					<form method="POST" action={confirmAction === 'delete' ? '?/delete' : '?/leave'}>
						<button class="inline-flex items-center gap-2 rounded-md bg-red-300 px-4 py-2.5 text-sm font-semibold text-abyss transition hover:bg-red-200" type="submit">
							{#if confirmAction === 'delete'}
								<Trash2 size={16} strokeWidth={2.2} />
								Delete workspace
							{:else}
								<LogOut size={16} strokeWidth={2.2} />
								Leave workspace
							{/if}
						</button>
					</form>
				</div>
			</div>
		</div>
	</div>
{/if}
