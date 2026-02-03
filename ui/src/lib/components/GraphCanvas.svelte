<script lang="ts">
	import { graphState } from '$lib/graph.svelte';
	import { untrack } from 'svelte';
	import {
		Code as CodeIcon,
		Share2 as Share2Icon,
		Info as InfoIcon,
		Activity as ActivityIcon
	} from 'lucide-svelte';

	let canvasVisible = $state(false);

	$effect(() => {
		if (graphState.selectedSymbol) {
			untrack(() => {
				canvasVisible = true;
			});
		}
	});
</script>

<div class="relative flex h-full w-full flex-col overflow-hidden bg-surface-bg/30">
	{#if graphState.selectedSymbol}
		<div
			class="flex h-full w-full flex-col p-8 transition-all duration-700 {canvasVisible
				? 'scale-100 opacity-100'
				: 'scale-95 opacity-0'}"
		>
			<!-- Header Area -->
			<div class="mb-8 flex items-start justify-between">
				<div class="max-w-4xl">
					<div class="mb-3 flex items-center gap-3">
						<span
							class="rounded border border-brand-primary/20 bg-brand-primary/10 px-2 py-0.5 text-[10px] font-bold tracking-widest text-brand-primary uppercase shadow-sm"
						>
							{graphState.selectedSymbol.kind}
						</span>
						<span
							class="max-w-sm truncate font-mono text-[10px] tracking-tight text-text-secondary/60 italic"
						>
							{graphState.selectedSymbol.uri}
						</span>
					</div>
					<h1 class="mb-3 text-6xl leading-none font-black tracking-tighter text-white">
						{graphState.selectedSymbol.name}
					</h1>
					<p class="text-base font-medium text-text-secondary/80">
						Defined in <span class="font-mono text-brand-secondary/90"
							>{graphState.selectedSymbol.path}</span
						>
						on line <span class="text-white">{graphState.selectedSymbol.line_start}</span>
					</p>
				</div>

				<div class="flex gap-3">
					<button
						class="flex items-center gap-2 rounded-xl border border-white/5 bg-white/5 px-4 py-2.5 text-xs font-semibold text-text-secondary transition-all hover:border-white/10 hover:bg-white/10 hover:text-white"
					>
						<Share2Icon class="h-3.5 w-3.5" />
						Share Context
					</button>
				</div>
			</div>

			<!-- Panels Grid -->
			<div class="grid flex-1 grid-cols-12 gap-8 overflow-hidden">
				<!-- Code Implementation: 8/12 width -->
				<section
					class="glass-dark col-span-8 flex flex-col overflow-hidden rounded-2xl border border-white/5 shadow-2xl"
				>
					<div class="flex items-center gap-2 border-b border-white/5 bg-white/[0.02] px-6 py-4">
						<CodeIcon class="h-4 w-4 text-brand-primary" />
						<h2 class="text-[10px] font-bold tracking-widest text-text-secondary uppercase">
							Implementation
						</h2>
					</div>
					<div
						class="custom-scrollbar flex-1 overflow-auto bg-black/40 p-8 font-mono text-[14px] leading-relaxed"
					>
						<pre class="text-text-primary/90"><code>{graphState.selectedSymbol.content}</code></pre>
					</div>
				</section>

				<!-- Architectural Context: 4/12 width -->
				<div class="col-span-4 flex flex-col gap-8">
					<section class="glass-dark flex flex-col rounded-2xl border border-white/5 p-8 shadow-xl">
						<div class="mb-6 flex items-center gap-2">
							<InfoIcon class="h-4 w-4 text-brand-secondary" />
							<h2 class="text-[10px] font-bold tracking-widest text-text-secondary uppercase">
								Signature Detail
							</h2>
						</div>
						<div class="rounded-xl border border-white/5 bg-black/30 p-5">
							<code class="font-mono text-xs leading-normal break-all text-brand-secondary/90">
								{graphState.selectedSymbol.signature || 'No explicit signature found'}
							</code>
						</div>
					</section>

					<section
						class="glass-dark flex flex-1 flex-col rounded-2xl border border-white/5 p-8 shadow-xl"
					>
						<div class="mb-8 flex items-center gap-2">
							<ActivityIcon class="h-4 w-4 text-brand-primary" />
							<h2 class="text-[10px] font-bold tracking-widest text-text-secondary uppercase">
								Semantic Analysis
							</h2>
						</div>
						<div class="flex flex-1 flex-col items-center justify-center px-6 text-center">
							{#if graphState.isAnalysisLoading}
								<div
									class="mb-6 flex h-16 w-16 animate-pulse items-center justify-center rounded-full border-2 border-dashed border-brand-primary/30"
								>
									<div
										class="h-8 w-8 rounded-full border border-brand-primary/10 bg-brand-primary/5"
									></div>
								</div>
								<p class="text-[11px] font-bold tracking-[0.3em] text-text-secondary/60 uppercase">
									Synthesizing Architecture...
								</p>
							{:else if graphState.analysis}
								<div class="mb-4 flex w-full flex-col items-center">
									<span
										class="mb-2 text-[10px] font-bold tracking-widest text-brand-primary uppercase"
										>Importance</span
									>
									<div
										class="relative h-2 w-32 overflow-hidden rounded-full bg-white/5 shadow-inner"
									>
										<div
											class="absolute top-0 left-0 h-full bg-brand-primary transition-all duration-1000"
											style="width: {graphState.analysis.importance * 10}%"
										></div>
									</div>
								</div>
								<p class="text-xs leading-relaxed text-text-primary/80">
									{graphState.analysis.summary}
								</p>
								<div
									class="mt-6 rounded-lg bg-white/5 px-3 py-1 text-[10px] font-bold text-text-secondary uppercase"
								>
									Role: {graphState.analysis.module_role}
								</div>
							{:else}
								<p class="text-[10px] text-text-secondary/40 italic">Analysis unavailable</p>
							{/if}
						</div>
					</section>
				</div>
			</div>
		</div>
	{:else}
		<div class="flex flex-1 flex-col items-center justify-center px-12 text-center">
			<div
				class="mb-12 flex h-40 w-40 animate-[spin_40s_linear_infinite] items-center justify-center rounded-full border-2 border-dashed border-white/5 p-8"
			>
				<div
					class="flex h-full w-full animate-[spin_15s_linear_infinite_reverse] items-center justify-center rounded-full border border-brand-primary/10"
				>
					<div class="h-8 w-8 rounded-full border-4 border-brand-primary/30"></div>
				</div>
			</div>
			<div class="max-w-md">
				<h2 class="mb-4 text-4xl font-black tracking-tighter text-white">Coderev COCKPIT</h2>
				<div class="mx-auto mb-6 h-1 w-12 rounded-full bg-brand-primary"></div>
				<p class="text-lg leading-relaxed font-medium text-text-secondary/80">
					Select a symbol from the sidebar to visualize its semantic footprint and architectural
					relationships.
				</p>
				<div class="mt-12 flex items-center justify-center gap-6 opacity-30">
					<div class="flex items-center gap-2">
						<CodeIcon class="h-4 w-4" />
						<span class="text-[10px] font-bold tracking-widest uppercase">Precise AST</span>
					</div>
					<div class="flex items-center gap-2">
						<ActivityIcon class="h-4 w-4" />
						<span class="text-[10px] font-bold tracking-widest uppercase">Global Linkage</span>
					</div>
				</div>
			</div>
		</div>
	{/if}

	<!-- Background Grid -->
	<div class="pointer-events-none absolute inset-0 z-0 opacity-10">
		<div
			class="h-full w-full bg-[radial-gradient(#ffffff20_1.5px,transparent_1.5px)] bg-size-[40px_40px]"
		></div>
	</div>
</div>
