<script lang="ts">
	import { graphState } from '$lib/graph.svelte';
	import { onMount } from 'svelte';
	import type { Symbol } from '$lib/types';
	import {
		Loader2 as Loader2Icon,
		ChevronRight as ChevronRightIcon,
		ChevronLeft as ChevronLeftIcon,
		Activity as ActivityIcon
	} from 'lucide-svelte';

	let callers = $state<Symbol[]>([]);
	let callees = $state<Symbol[]>([]);
	let isLoadingTrace = $state(false);

	$effect(() => {
		if (graphState.selectedSymbol) {
			fetchTrace(graphState.selectedSymbol.uri);
		}
	});

	async function fetchTrace(uri: string) {
		isLoadingTrace = true;
		try {
			const res = await fetch(
				`http://127.0.0.1:3000/api/v1/trace?uri=${encodeURIComponent(uri)}&depth=1`,
				{
					headers: { 'X-Request-ID': crypto.randomUUID() }
				}
			);
			if (res.ok) {
				const data = await res.json();
				callers = data.callers;
				callees = data.callees;
			}
		} catch (e) {
			console.error('Failed to fetch trace', e);
		} finally {
			isLoadingTrace = false;
		}
	}
</script>

<div class="space-y-8 overflow-y-auto pr-2 pb-20">
	{#if isLoadingTrace}
		<div class="flex h-40 items-center justify-center">
			<Loader2Icon class="h-8 w-8 animate-spin text-brand-primary" />
		</div>
	{:else if graphState.selectedSymbol}
		<!-- Callers -->
		<section>
			<h3
				class="mb-4 flex items-center gap-2 text-[10px] font-bold tracking-widest text-text-secondary uppercase"
			>
				<ChevronLeftIcon class="h-3 w-3" />
				Callers ({callers.length})
			</h3>
			<div class="space-y-2">
				{#each callers as caller}
					<button
						onclick={() => graphState.selectSymbol(caller)}
						class="flex w-full items-center gap-3 rounded-lg border border-white/5 bg-white/5 p-3 text-left transition-all hover:bg-white/10"
					>
						<div class="h-2 w-2 rounded-full bg-brand-primary"></div>
						<div class="flex flex-1 flex-col overflow-hidden">
							<span class="truncate text-sm font-medium">{caller.name}</span>
							<span class="truncate text-[10px] text-text-secondary">{caller.path}</span>
						</div>
					</button>
				{:else}
					<p class="text-[10px] italic text-text-secondary opacity-50">No callers found</p>
				{/each}
			</div>
		</section>

		<!-- Selected Symbol Summary -->
		<section class="rounded-xl border border-brand-primary/20 bg-brand-primary/5 p-4">
			<span class="mb-1 text-[10px] font-bold tracking-tighter text-brand-primary uppercase"
				>Focusing on</span
			>
			<h4 class="text-xl font-bold">{graphState.selectedSymbol.name}</h4>
			<p class="mt-1 truncate text-xs text-text-secondary">{graphState.selectedSymbol.path}</p>
		</section>

		<!-- Callees -->
		<section>
			<h3
				class="mb-4 flex items-center justify-between text-[10px] font-bold tracking-widest text-text-secondary uppercase"
			>
				<span class="flex items-center gap-2"
					>Callees ({callees.length}) <ChevronRightIcon class="h-3 w-3" /></span
				>
			</h3>
			<div class="space-y-2">
				{#each callees as callee}
					<button
						onclick={() => graphState.selectSymbol(callee)}
						class="flex w-full items-center gap-3 rounded-lg border border-white/5 bg-white/5 p-3 text-left transition-all hover:bg-white/10"
					>
						<div class="flex flex-1 flex-col overflow-hidden">
							<span class="truncate text-sm font-medium">{callee.name}</span>
							<span class="truncate text-[10px] text-text-secondary">{callee.path}</span>
						</div>
						<div class="h-2 w-2 rounded-full bg-brand-secondary"></div>
					</button>
				{:else}
					<p class="text-[10px] italic text-text-secondary opacity-50">No callees found</p>
				{/each}
			</div>
		</section>

		<!-- Impact Analysis -->
		<div class="space-y-6 border-t border-white/5 pt-4">
			{#if graphState.impact.length > 0}
				<section>
					<h3
						class="mb-4 flex items-center justify-between text-[10px] font-bold tracking-widest text-brand-secondary uppercase"
					>
						<span class="flex items-center gap-2"
							>Impact Analysis (Blast Radius) <ActivityIcon class="h-3 w-3" /></span
						>
					</h3>
					<div class="space-y-2">
						{#each graphState.impact as item}
							<div
								class="flex flex-col gap-1 rounded-lg border border-white/5 bg-black/20 p-3 transition-all"
							>
								<div class="flex items-center justify-between">
									<span class="font-mono text-[10px] font-bold text-text-secondary uppercase"
										>{item.edge_kind}</span
									>
									<span class="text-[10px] text-text-secondary opacity-60">Depth: {item.depth}</span
									>
								</div>
								<button
									onclick={() => graphState.selectSymbol(item.symbol)}
									class="truncate text-left text-sm font-medium transition-colors hover:text-brand-primary"
								>
									{item.symbol.name}
								</button>
								<div class="mt-1 h-1 w-full overflow-hidden rounded-full bg-white/5">
									<div
										class="h-full bg-brand-secondary transition-all duration-1000"
										style="width: {item.confidence * 100}%"
									></div>
								</div>
							</div>
						{/each}
					</div>
				</section>
			{:else}
				<button
					onclick={() => graphState.fetchImpact()}
					disabled={graphState.isImpactLoading}
					class="group flex w-full items-center justify-between gap-2 rounded-xl border border-white/10 bg-white/5 px-4 py-3 text-sm font-medium transition-all hover:bg-white/10 disabled:opacity-50"
				>
					<span class="flex items-center gap-2">
						{#if graphState.isImpactLoading}
							<Loader2Icon class="h-4 w-4 animate-spin text-brand-secondary" />
							Analyzing...
						{:else}
							<ActivityIcon
								class="h-4 w-4 text-brand-secondary transition-transform group-hover:scale-110"
							/>
							Deep Impact Analysis
						{/if}
					</span>
					{#if !graphState.isImpactLoading}
						<ChevronRightIcon class="h-4 w-4 transition-transform group-hover:translate-x-1" />
					{/if}
				</button>
			{/if}
		</div>
	{/if}
</div>
