<script lang="ts">
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { AlertTriangle, UserRound } from '@lucide/svelte';

	const title = $derived(page.status === 404 ? 'Signal not found' : 'Relay interrupted');
	const detail = $derived(
		page.error?.message ||
		(page.status === 404
			? 'This screen is unavailable or has not finished converging yet.'
			: 'Something failed while loading this screen.')
	);
</script>

<svelte:head><title>{page.status} - Relay</title></svelte:head>

<main class="relative grid min-h-screen overflow-hidden bg-abyss px-5 py-10 text-snow">
	<div class="absolute inset-0 bg-[radial-gradient(circle_at_78%_0%,var(--color-signal)_0%,transparent_34%),radial-gradient(circle_at_0%_100%,rgba(47,214,161,0.18)_0%,transparent_30%),linear-gradient(135deg,var(--color-carbon),var(--color-abyss))]"></div>
	<section class="relative mx-auto grid min-h-[calc(100vh-5rem)] w-full max-w-5xl place-items-center">
		<div class="w-full">
			<div class="inline-flex items-center gap-3 rounded-full border border-warm-charcoal bg-carbon/80 px-4 py-2 text-xs font-semibold uppercase tracking-[0.16em] text-signal backdrop-blur">
				<AlertTriangle size={16} strokeWidth={2.2} />
				Error {page.status}
			</div>

			<h1 class="mt-8 max-w-3xl font-[system-ui,sans-serif] text-6xl font-semibold tracking-[-0.07em] text-snow sm:text-7xl lg:text-8xl">{title}</h1>
			<p class="mt-6 max-w-2xl text-base leading-8 text-parchment sm:text-lg">{detail}</p>

			<a class="mt-10 inline-flex items-center justify-center gap-2 rounded-md bg-signal px-5 py-3 text-sm font-semibold text-abyss transition hover:bg-mint" href={resolve('/profile')}>
				<UserRound size={16} strokeWidth={2.2} />
				Go to profile
			</a>
		</div>
	</section>
</main>
