<script lang="ts">
	import AuthButton from '$lib/components/auth/AuthButton.svelte';
	import AuthCard from '$lib/components/auth/AuthCard.svelte';
	import AuthInput from '$lib/components/auth/AuthInput.svelte';
	import AuthShell from '$lib/components/auth/AuthShell.svelte';

	import type { ActionData } from './$types';

	let email = $state('');
	let password = $state('');
	let { form }: { form: ActionData } = $props();
</script>

<svelte:head><title>Sign in - Relay</title></svelte:head>

<AuthShell title="Sign in to Relay">
	{#if form?.error}
		<p class="mb-4 rounded-md border border-red-400/30 bg-red-500/10 px-4 py-3 text-sm text-red-200">
			{form.error}
		</p>
	{/if}

	<AuthCard>
		<div class="mb-4 rounded-md border border-[#2fd6a1]/30 bg-[#2fd6a1]/10 px-4 py-3 text-sm text-[#d7fff1]">
			<p class="font-semibold text-[#f5fffb]">Demo login</p>
			<p>Email: <code>demo@relay.local</code></p>
			<p>Password: <code>demo1234</code></p>
			<p class="mt-1 text-[#a8d8ca]">Any non-empty email and password will sign in.</p>
		</div>

		<form method="POST" action="?/signin" class="space-y-4">
			<AuthInput bind:value={email} label="Email address" name="email" type="email" autocomplete="email" />
			<AuthInput bind:value={password} label="Password" name="password" type="password" autocomplete="current-password" />

			<AuthButton>Sign in</AuthButton>
		</form>

	</AuthCard>

	<div class="mt-4 rounded-lg border border-[#3d3a39] bg-[#050507] p-4 text-center text-sm text-[#b8b3b0]">
		New to Relay? <a class="text-[#2fd6a1] hover:text-[#00d992]" href="/auth/signup">Create an account</a>
	</div>
</AuthShell>
