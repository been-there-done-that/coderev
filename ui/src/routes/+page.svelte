<script lang="ts">
	import { graphState } from '$lib/graph.svelte';
	import { onMount } from 'svelte';
	import {
		Search as SearchIcon,
		Activity as ActivityIcon,
		FileCode as FileCodeIcon,
		ChevronRight as ChevronRightIcon,
		Loader2 as Loader2Icon
	} from 'lucide-svelte';
	import TracePanel from '$lib/components/TracePanel.svelte';
	import GraphCanvas from '$lib/components/GraphCanvas.svelte';

	let searchQuery = $state('');

	async function handleSearch(e: Event) {
		e.preventDefault();
		await graphState.search(searchQuery);
	}

	onMount(() => {
		// Initial stats or discovery feed fetch
	});
</script>

<div class="relative flex h-screen w-screen overflow-hidden bg-surface-bg text-text-primary">
	<!-- Glow Background Effects -->
	<div
		class="glow-bg top-[-10%] left-[-10%] h-[400px] w-[400px] rounded-full bg-brand-primary/20"
	></div>
	<div
		class="glow-bg right-[-10%] bottom-[-10%] h-[400px] w-[400px] rounded-full bg-brand-secondary/20"
	></div>

	<!-- ZONE A: Search & Command Bar -->
	<header
		class="glass-dark fixed top-6 left-1/2 z-50 flex h-14 w-full max-w-2xl -translate-x-1/2 items-center rounded-2xl px-4 shadow-2xl"
	>
		<SearchIcon class="mr-3 h-5 w-5 text-text-secondary" />
		<form onsubmit={handleSearch} class="flex-1">
			<input
				type="text"
				bind:value={searchQuery}
				placeholder="Search symbols, paths, or natural language..."
				class="w-full border-none bg-transparent text-lg outline-none placeholder:text-text-secondary/50 focus:ring-0"
			/>
		</form>
		{#if graphState.isLoading}
			<Loader2Icon class="ml-2 h-5 w-5 animate-spin text-brand-primary" />
		{/if}
		<div
			class="ml-4 flex items-center gap-2 rounded-lg bg-white/5 px-2 py-1 text-xs font-medium text-text-secondary"
		>
			<span class="rounded bg-white/10 px-1.5 py-0.5">⌘</span>
			<span>K</span>
		</div>
	</header>

	<!-- Main Layout -->
	<main class="flex h-full w-full pt-20">
		<!-- Sidebar / Results -->
		<aside class="glass-dark z-10 flex h-full w-80 flex-col border-r border-white/5 p-4">
			<div class="mb-6 flex items-center justify-between">
				<h2 class="text-sm font-semibold tracking-wider text-text-secondary uppercase">Results</h2>
				<span class="rounded-full bg-brand-primary/10 px-2 py-0.5 text-[10px] text-brand-primary">
					{graphState.results.length} found
				</span>
			</div>

			<div class="flex-1 space-y-2 overflow-y-auto pr-2">
				{#each graphState.results as result}
					<button
						onclick={() => graphState.selectSymbol(result)}
						class="group flex w-full flex-col items-start rounded-xl p-3 text-left transition-all hover:bg-white/5 {graphState
							.selectedSymbol?.uri === result.uri
							? 'border border-white/10 bg-white/10'
							: 'border border-transparent'}"
					>
						<div class="mb-1 flex w-full items-center justify-between">
							<span class="font-mono text-xs text-brand-primary uppercase">{result.kind}</span>
							<ChevronRightIcon
								class="h-3 w-3 opacity-0 transition-opacity group-hover:opacity-100"
							/>
						</div>
						<span class="truncate font-medium">{result.name}</span>
						<span class="truncate text-xs text-text-secondary"
							>{result.path}:{result.line_start}</span
						>
					</button>
				{:else}
					<div class="flex h-40 flex-col items-center justify-center text-center opacity-40">
						<FileCodeIcon class="mb-3 h-10 w-10 text-text-secondary" />
						<p class="text-sm italic">Search to explore the code graph</p>
					</div>
				{/each}
			</div>
		</aside>

		<!-- ZONE B: Graph Canvas -->
		<section class="relative flex-1 bg-surface-bg/50">
			<GraphCanvas />
		</section>

		<!-- ZONE C: Inspector -->
		<aside
			class="glass-dark z-10 hidden h-full w-96 flex-col border-l border-white/5 p-4 shadow-2xl lg:flex"
		>
			<h2 class="mb-6 text-sm font-semibold tracking-wider text-text-secondary uppercase">
				Inspector
			</h2>
			{#if graphState.selectedSymbol}
				<TracePanel />
			{:else}
				<div class="flex h-full items-center justify-center text-center opacity-30">
					<p class="text-sm italic">Nothing selected</p>
				</div>
			{/if}
		</aside>
	</main>
</div>

<style>
	:global(body) {
		overflow: hidden;
	}
</style>
