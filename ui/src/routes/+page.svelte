<script lang="ts">
	import { onMount } from 'svelte';
	import * as api from '$lib/api';
	import type { SearchResult, SymbolResult, GraphResult } from '$lib/api';
	import { fade } from 'svelte/transition';

	let query = '';
	let results: SearchResult[] = [];
	let selectedSymbol: SymbolResult | null = null;
	let graph: GraphResult | null = null;
	let loading = false;
	let stats: any = null;

	onMount(async () => {
		stats = await api.getStats();
		// Load initial symbols
		results = await api.search('');
	});

	async function handleSearch() {
		loading = true;
		results = await api.search(query);
		loading = false;
	}

	async function selectSymbol(uri: string) {
		selectedSymbol = await api.getSymbol(uri);
		graph = await api.getGraph(uri);
	}
</script>

<div class="flex h-screen w-screen overflow-hidden bg-gray-900 font-sans text-gray-100">
	<!-- Left Panel: Search & Results -->
	<div class="flex w-80 flex-col border-r border-gray-800">
		<div class="border-b border-gray-800 p-4">
			<h1
				class="mb-4 bg-gradient-to-r from-blue-400 to-emerald-400 bg-clip-text text-xl font-bold text-transparent"
			>
				Coderev Graph
			</h1>

			<div class="relative">
				<input
					type="text"
					bind:value={query}
					on:keydown={(e) => e.key === 'Enter' && handleSearch()}
					placeholder="Search query..."
					class="w-full rounded border border-gray-700 bg-gray-800 px-3 py-2 text-sm transition-colors focus:border-blue-500 focus:outline-none"
				/>
			</div>
			{#if stats}
				<div class="mt-2 flex gap-2 text-xs text-gray-500">
					<span>{stats.symbols} syms</span>
					<span>{stats.edges} edges</span>
				</div>
			{/if}
		</div>

		<div class="flex-1 space-y-1 overflow-y-auto p-2">
			{#if loading}
				<div class="animate-pulse p-4 text-center text-gray-500">Searching...</div>
			{/if}
			{#each results as res}
				<button
					class="group w-full rounded p-2 text-left transition-colors hover:bg-gray-800"
					class:bg-blue-900={selectedSymbol?.symbol.uri === res.uri}
					on:click={() => selectSymbol(res.uri)}
				>
					<div class="text-sm font-medium text-gray-200 group-hover:text-blue-300">{res.name}</div>
					<div class="truncate text-xs text-gray-500">{res.path}</div>
					<div class="mt-0.5 text-[10px] text-gray-600 uppercase">{res.kind}</div>
				</button>
			{/each}
		</div>
	</div>

	<!-- Center Panel: Detail -->
	<div class="flex flex-1 flex-col border-r border-gray-800 bg-gray-900/50">
		{#if selectedSymbol}
			<div class="overflow-y-auto p-6" in:fade>
				<div class="mb-6">
					<div class="mb-1 font-mono text-sm text-gray-500">{selectedSymbol.symbol.uri}</div>
					<h2 class="mb-2 text-3xl font-bold text-white">{selectedSymbol.symbol.name}</h2>
					<div
						class="inline-flex items-center rounded border border-blue-800 bg-blue-900/30 px-2 py-0.5 text-xs font-medium text-blue-400"
					>
						{selectedSymbol.symbol.kind}
					</div>
				</div>

				{#if selectedSymbol.symbol.signature}
					<div class="mb-6 rounded border border-gray-800 bg-gray-800/50 p-4">
						<div class="mb-2 block text-xs font-bold tracking-wider text-gray-500 uppercase">
							Signature
						</div>
						<code class="font-mono text-sm whitespace-pre-wrap text-emerald-300"
							>{selectedSymbol.symbol.signature}</code
						>
					</div>
				{/if}

				{#if selectedSymbol.symbol.doc}
					<div class="mb-6">
						<div class="mb-2 block text-xs font-bold tracking-wider text-gray-500 uppercase">
							Documentation
						</div>
						<div class="prose prose-sm max-w-none text-gray-300 prose-invert">
							{selectedSymbol.symbol.doc}
						</div>
					</div>
				{/if}

				<div>
					<div class="mb-2 block text-xs font-bold tracking-wider text-gray-500 uppercase">
						Source Code
					</div>
					<pre
						class="overflow-x-auto rounded border border-gray-800 bg-gray-950 p-4 font-mono text-sm leading-relaxed text-gray-300 md:leading-loose"><code
							>{selectedSymbol.symbol.content}</code
						></pre>
				</div>
			</div>
		{:else}
			<div class="flex flex-1 items-center justify-center text-gray-600">
				Select a symbol to view details
			</div>
		{/if}
	</div>

	<!-- Right Panel: Graph -->
	<div class="flex w-80 flex-col border-l border-gray-800 bg-gray-900">
		<div class="border-b border-gray-800 p-4">
			<h3 class="font-bold text-gray-300">Relationships</h3>
		</div>
		<div class="flex-1 overflow-y-auto p-4">
			{#if graph}
				<div class="space-y-6">
					<div>
						<h4 class="mb-3 text-xs font-bold text-gray-500 uppercase">Callers (Incoming)</h4>
						{#if selectedSymbol?.edges_in.length}
							<div class="space-y-2">
								{#each selectedSymbol.edges_in as edge}
									<button
										class="block w-full rounded border border-gray-800 bg-gray-800/50 p-2 text-left text-xs transition-all hover:bg-gray-800"
										on:click={() =>
											selectSymbol(
												typeof edge.from_uri === 'string' ? edge.from_uri : edge.from_uri.uri
											)}
									>
										<div class="mb-0.5 font-mono text-blue-300">
											{edge.from_uri.uri?.split('#')[1] || edge.from_uri}
										</div>
										<div class="text-gray-500">{edge.kind}</div>
									</button>
								{/each}
							</div>
						{:else}
							<div class="text-sm text-gray-600 italic">No incoming edges</div>
						{/if}
					</div>

					<div>
						<h4 class="mb-3 text-xs font-bold text-gray-500 uppercase">Callees (Outgoing)</h4>
						{#if selectedSymbol?.edges_out.length}
							<div class="space-y-2">
								{#each selectedSymbol.edges_out as edge}
									<button
										class="block w-full rounded border border-gray-800 bg-gray-800/50 p-2 text-left text-xs transition-all hover:bg-gray-800"
										on:click={() =>
											selectSymbol(typeof edge.to_uri === 'string' ? edge.to_uri : edge.to_uri.uri)}
									>
										<div class="mb-0.5 font-mono text-emerald-300">
											{edge.to_uri.uri?.split('#')[1] || edge.to_uri}
										</div>
										<div class="text-gray-500">{edge.kind}</div>
									</button>
								{/each}
							</div>
						{:else}
							<div class="text-sm text-gray-600 italic">No outgoing edges</div>
						{/if}
					</div>
				</div>
			{:else}
				<div class="py-10 text-center text-sm text-gray-600">Graph visualization</div>
			{/if}
		</div>
	</div>
</div>
