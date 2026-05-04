<script lang="ts">
	import type { HTMLInputAttributes } from 'svelte/elements';

	let {
		label,
		name,
		type = 'text',
		autocomplete,
		required = true,
		value = $bindable('')
	}: {
		label: string;
		name: string;
		type?: string;
		autocomplete?: HTMLInputAttributes['autocomplete'];
		required?: boolean;
		value?: string;
	} = $props();
</script>

<label class="block text-sm font-medium text-[#b8b3b0]">
	{label}
	<input
		bind:value
		{name}
		{type}
		{autocomplete}
		{required}
		class="mt-2 w-full rounded-md border border-[#3d3a39] bg-[#050507] px-3 py-2 text-sm text-[#f2f2f2] outline-none focus:border-[#00d992]"
	/>
</label>
