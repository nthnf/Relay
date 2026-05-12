<script lang="ts">
	import FeatureGrid from '$lib/components/landing/FeatureGrid.svelte';
	import HeroSection from '$lib/components/landing/HeroSection.svelte';
	import LandingNavbar from '$lib/components/landing/LandingNavbar.svelte';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
</script>

<svelte:head><title>Relay - Realtime team chat</title></svelte:head>

<div class="min-h-screen bg-abyss text-snow">
	<LandingNavbar authenticated={data.authenticated} />
	<HeroSection authenticated={data.authenticated} />
	<FeatureGrid />
</div>
