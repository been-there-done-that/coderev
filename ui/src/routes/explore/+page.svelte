<script lang="ts">
	import { onMount } from 'svelte';
	import { exploreState, type GraphNode } from '$lib/explore.svelte';
	import ForceGraph3D from '$lib/components/ForceGraph3D.svelte';
	import {
		Search as SearchIcon,
		Loader2 as Loader2Icon,
		X as XIcon,
		Box as BoxIcon
	} from 'lucide-svelte';

	let searchQuery = $state('');

	async function handleSearch(e: Event) {
		e.preventDefault();
		await exploreState.search(searchQuery);
	}

	function handleNodeClick(node: GraphNode) {
		exploreState.selectNode(node.id);
	}

	function clearSelection() {
		exploreState.clearSelection();
	}

	onMount(() => {
		exploreState.loadInitialData();
	});

	let selectedSymbol = $derived(exploreState.getSelectedSymbol());
</script>

<svelte:head>
	<title>3D Code Explorer | Coderev</title>
</svelte:head>

<div class="relative flex h-screen w-screen flex-col overflow-hidden bg-black">
	<!-- Search Bar - Fixed at top -->
	<header
		class="glass-dark fixed top-6 left-1/2 z-50 flex h-14 w-full max-w-2xl -translate-x-1/2 items-center rounded-2xl px-4 shadow-2xl"
	>
		<SearchIcon class="mr-3 h-5 w-5 text-text-secondary" />
		<form onsubmit={handleSearch} class="flex-1">
			<input
				type="text"
				bind:value={searchQuery}
				placeholder="Search symbols in 3D space..."
				class="w-full border-none bg-transparent text-lg outline-none placeholder:text-text-secondary/50 focus:ring-0"
			/>
		</form>
		{#if exploreState.isLoading}
			<Loader2Icon class="ml-2 h-5 w-5 animate-spin text-brand-primary" />
		{/if}
		<div
			class="ml-4 flex items-center gap-2 rounded-lg bg-white/5 px-2 py-1 text-xs font-medium text-text-secondary"
		>
			<span class="rounded bg-white/10 px-1.5 py-0.5">⌘</span>
			<span>K</span>
		</div>
	</header>

	<!-- 3D Graph Canvas - Full Viewport -->
	<main class="flex-1">
		{#if exploreState.graphData.nodes.length > 0}
			<ForceGraph3D
				data={exploreState.graphData}
				selectedUri={exploreState.selectedUri}
				highlightedLinks={exploreState.highlightedLinks}
				onNodeClick={handleNodeClick}
			/>
		{:else if exploreState.isLoading}
			<div class="flex h-full items-center justify-center">
				<div class="flex flex-col items-center gap-4">
					<Loader2Icon class="h-12 w-12 animate-spin text-brand-primary" />
					<p class="text-lg text-text-secondary">Loading code graph...</p>
				</div>
			</div>
		{:else}
			<div class="flex h-full items-center justify-center">
				<div class="flex flex-col items-center gap-4 text-center">
					<BoxIcon class="h-16 w-16 text-text-secondary/30" />
					<p class="text-lg text-text-secondary">No symbols found</p>
					<p class="text-sm text-text-secondary/60">Try a different search query</p>
				</div>
			</div>
		{/if}
	</main>

	<!-- Selected Node Info - Bottom overlay -->
	{#if selectedSymbol}
		<div
			class="glass-dark fixed bottom-6 left-1/2 z-50 flex w-full max-w-xl -translate-x-1/2 items-center justify-between gap-4 rounded-2xl border border-white/10 px-6 py-4 shadow-2xl"
		>
			<div class="flex-1 overflow-hidden">
				<div class="mb-1 flex items-center gap-2">
					<span
						class="rounded border border-brand-primary/20 bg-brand-primary/10 px-2 py-0.5 text-[10px] font-bold tracking-widest text-brand-primary uppercase"
					>
						{selectedSymbol.kind}
					</span>
				</div>
				<h3 class="truncate text-xl font-bold text-white">{selectedSymbol.name}</h3>
				<p class="truncate text-xs text-text-secondary">{selectedSymbol.path}</p>
			</div>
			<button
				onclick={clearSelection}
				class="flex h-8 w-8 items-center justify-center rounded-lg bg-white/5 transition-colors hover:bg-white/10"
			>
				<XIcon class="h-4 w-4" />
			</button>
		</div>
	{/if}

	<!-- Stats overlay - Top right -->
	<div
		class="fixed top-6 right-6 z-40 rounded-lg bg-black/60 px-3 py-2 text-xs text-text-secondary backdrop-blur-sm"
	>
		<span class="font-medium text-white">{exploreState.graphData.nodes.length}</span> nodes
		<span class="mx-2 opacity-30">|</span>
		<span class="font-medium text-white">{exploreState.graphData.links.length}</span> edges
	</div>

	<!-- Legend - Bottom left -->
	<div
		class="fixed bottom-6 left-6 z-40 flex flex-col gap-2 rounded-lg bg-black/60 px-4 py-3 text-xs backdrop-blur-sm"
	>
		<div class="text-[10px] font-bold tracking-wider text-text-secondary uppercase">Legend</div>
		<div class="flex items-center gap-2">
			<span class="h-3 w-3 rounded-full bg-[#8b5cf6]"></span>
			<span class="text-text-secondary">Callable</span>
		</div>
		<div class="flex items-center gap-2">
			<span class="h-3 w-3 rounded-full bg-[#06b6d4]"></span>
			<span class="text-text-secondary">Container</span>
		</div>
		<div class="flex items-center gap-2">
			<span class="h-3 w-3 rounded-full bg-[#22c55e]"></span>
			<span class="text-text-secondary">Value</span>
		</div>
		<div class="mt-2 border-t border-white/10 pt-2">
			<div class="flex items-center gap-2">
				<span class="h-0.5 w-4 bg-[#3b82f6]"></span>
				<span class="text-text-secondary">Caller</span>
			</div>
			<div class="flex items-center gap-2">
				<span class="h-0.5 w-4 bg-[#22c55e]"></span>
				<span class="text-text-secondary">Callee</span>
			</div>
		</div>
	</div>
</div>

<style>
	:global(body) {
		overflow: hidden;
		background: black;
	}

	.glass-dark {
		background: rgba(0, 0, 0, 0.7);
		backdrop-filter: blur(16px);
		-webkit-backdrop-filter: blur(16px);
	}
</style>
