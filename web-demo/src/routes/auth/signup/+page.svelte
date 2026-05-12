<script lang="ts">
	import { goto } from '$app/navigation';
	import AuthButton from '$lib/components/auth/AuthButton.svelte';
	import AuthCard from '$lib/components/auth/AuthCard.svelte';
	import AuthInput from '$lib/components/auth/AuthInput.svelte';
	import AuthShell from '$lib/components/auth/AuthShell.svelte';

	let email = $state('');
	let password = $state('');
	let username = $state('');
	let displayName = $state('');
	let error = $state('');
	let loading = $state(false);

	async function signup(event: SubmitEvent) {
		event.preventDefault();
		error = '';
		loading = true;

		try {
			const response = await fetch('/auth/register', {
				method: 'POST',
				headers: { 'content-type': 'application/json' },
				body: JSON.stringify({ email, password, username, displayName })
			});

			if (!response.ok) {
				throw new Error((await response.text()) || 'Unable to create account');
			}

			await goto(`/auth/verify-email?email=${encodeURIComponent(email)}`);
		} catch (cause) {
			error = cause instanceof Error ? cause.message : 'Unable to create account';
		} finally {
			loading = false;
		}
	}
</script>

<svelte:head><title>Sign up - Relay</title></svelte:head>

<AuthShell title="Create your account">
	{#if error}
		<p class="mb-4 rounded-md border border-red-400/30 bg-red-500/10 px-4 py-3 text-sm text-red-200">{error}</p>
	{/if}

	<AuthCard>
		<form class="space-y-4" onsubmit={signup}>
			<AuthInput bind:value={email} label="Email address" name="email" type="email" autocomplete="email" />
			<AuthInput bind:value={username} label="Username" name="username" autocomplete="username" />
			<AuthInput bind:value={displayName} label="Display name" name="displayName" autocomplete="name" />
			<AuthInput bind:value={password} label="Password" name="password" type="password" autocomplete="new-password" />
			<AuthButton>{loading ? 'Creating account...' : 'Create account'}</AuthButton>
		</form>
	</AuthCard>

	<div class="mt-4 rounded-lg border border-[#3d3a39] bg-[#050507] p-4 text-center text-sm text-[#b8b3b0]">
		Already have an account? <a class="text-[#2fd6a1] hover:text-[#00d992]" href="/auth/login">Sign in</a>
	</div>
</AuthShell>
