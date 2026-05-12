<script lang="ts">
	import AuthButton from '$lib/components/auth/AuthButton.svelte';
	import AuthCard from '$lib/components/auth/AuthCard.svelte';
	import AuthShell from '$lib/components/auth/AuthShell.svelte';
	import { goto } from '$app/navigation';
	import { onDestroy, onMount } from 'svelte';

	let email = $state('');
	let error = $state('');
	let success = $state('');
	let resending = $state(false);
	let cooldown = $state(30);
	let timer: number | undefined;

	const canResend = $derived(cooldown === 0 && !resending);

	onMount(() => {
		email = new URLSearchParams(window.location.search).get('email') ?? '';

		if (!email) {
			void goto('/auth/signup');
			return;
		}

		startCooldown();
	});

	onDestroy(() => {
		if (timer) window.clearInterval(timer);
	});

	function startCooldown() {
		cooldown = 30;
		if (timer) window.clearInterval(timer);

		timer = window.setInterval(() => {
			cooldown = Math.max(0, cooldown - 1);

			if (cooldown === 0 && timer) {
				window.clearInterval(timer);
				timer = undefined;
			}
		}, 1_000);
	}

	async function resend() {
		if (!canResend) return;

		error = '';
		success = '';
		resending = true;

		try {
			const response = await fetch('/auth/resend-verification', {
				method: 'POST',
				headers: { 'content-type': 'application/json' },
				body: JSON.stringify({ email })
			});

			if (!response.ok) {
				throw new Error((await response.text()) || 'Unable to resend verification email');
			}

			success = 'Verification email has been resent.';
			startCooldown();
		} catch (cause) {
			error = cause instanceof Error ? cause.message : 'Unable to resend verification email';
		} finally {
			resending = false;
		}
	}
</script>

<svelte:head><title>Verify email - Relay</title></svelte:head>

<AuthShell title="Verify your email">
	{#if error}
		<p class="mb-4 rounded-md border border-red-400/30 bg-red-500/10 px-4 py-3 text-sm text-red-200">{error}</p>
	{/if}
	{#if success}
		<p class="mb-4 rounded-md border border-[#00d992]/40 bg-[#00d992]/10 px-4 py-3 text-sm text-[#2fd6a1]">{success}</p>
	{/if}

	<AuthCard>
		<div class="space-y-5 text-center">
			<div class="mx-auto grid h-14 w-14 place-items-center rounded-full border border-[#3d3a39] bg-[#050507] text-2xl text-[#2fd6a1]">@</div>
			<div>
				<h2 class="font-[system-ui,sans-serif] text-2xl font-semibold tracking-[-0.04em] text-[#f2f2f2]">Email verification request has been sent</h2>
				<p class="mt-3 text-sm leading-6 text-[#8b949e]">
					Check your inbox{email ? ` for ${email}` : ''}. The verification link will sign you in automatically.
				</p>
			</div>

			<div class="rounded-lg border border-[#3d3a39] bg-[#050507] p-4 text-sm text-[#b8b3b0]">
				Didn't receive it?
				<button class="font-semibold text-[#2fd6a1] hover:text-[#00d992] disabled:cursor-not-allowed disabled:text-[#8b949e]" type="button" disabled={!canResend} onclick={resend}>
					Click here to resend{cooldown > 0 ? ` (${cooldown}s)` : ''}
				</button>
			</div>
		</div>
	</AuthCard>
</AuthShell>
