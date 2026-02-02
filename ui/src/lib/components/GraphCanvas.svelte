<script lang="ts">
	import { graphState } from '$lib/graph.svelte';
	import { onMount, untrack } from 'svelte';
	import type { Symbol } from '$lib/types';

	let canvasVisible = $state(false);

	$effect(() => {
		if (graphState.selectedSymbol) {
			untrack(() => {
				canvasVisible = true;
			});
		}
	});
</script>

<div class="relative flex h-full w-full flex-col items-center justify-center overflow-hidden">
	{#if graphState.selectedSymbol}
		<div
			class="z-10 w-full max-w-4xl p-12 transition-all duration-700 {canvasVisible
				? 'scale-100 opacity-100'
				: 'scale-95 opacity-0'}"
		>
			<div class="mb-8 flex items-center gap-4">
				<div
					class="flex h-12 w-12 items-center justify-center rounded-2xl border border-brand-primary/30 bg-brand-primary/20"
				>
					<span class="text-xl font-bold text-brand-primary uppercase"
						>{graphState.selectedSymbol.kind[0]}</span
					>
				</div>
				<div>
					<h1 class="mb-1 text-5xl leading-none font-extrabold tracking-tight text-text-primary">
						{graphState.selectedSymbol.name}
					</h1>
					<p class="font-mono text-sm tracking-widest text-brand-primary/60 uppercase">
						{graphState.selectedSymbol.uri}
					</p>
				</div>
			</div>

			<div class="mb-8 grid grid-cols-1 gap-6 md:grid-cols-2">
				<div class="glass rounded-2xl border-white/5 bg-white/5 p-6 shadow-2xl">
					<h3 class="mb-4 text-xs font-bold tracking-widest text-text-secondary uppercase">
						Implementation
					</h3>
					<pre
						class="max-h-[400px] overflow-x-auto rounded-xl border border-white/5 bg-black/40 p-4 font-mono text-[13px] leading-relaxed text-text-primary/90">
                        <code>{graphState.selectedSymbol.content}</code>
                    </pre>
				</div>

				<div class="flex flex-col gap-6">
					<div class="glass flex-1 rounded-2xl border-white/5 bg-white/5 p-6 shadow-2xl">
						<h3 class="mb-4 text-xs font-bold tracking-widest text-text-secondary uppercase">
							Inferred Architecture
						</h3>
						<div
							class="flex h-full min-h-[200px] flex-col items-center justify-center rounded-xl border-2 border-dashed border-white/5"
						>
							<div class="flex animate-pulse flex-col items-center opacity-40">
								<div class="mb-4 h-16 w-16 rounded-full border-4 border-brand-secondary/40"></div>
								<p class="text-xs font-medium">Synthesizing relationships...</p>
							</div>
						</div>
					</div>
				</div>
			</div>
		</div>
	{:else}
		<div class="text-center opacity-50">
			<div
				class="mx-auto mb-6 flex h-24 w-24 animate-[spin_20s_linear_infinite] items-center justify-center rounded-full border-4 border-dashed border-white/10"
			>
				<div class="h-12 w-12 rounded-full border-4 border-brand-primary/20"></div>
			</div>
			<h2 class="text-2xl font-light text-text-secondary">Coderev Code Intelligence Substrate</h2>
			<p class="mt-2 text-sm text-text-secondary/60">Connecting the dots across your repository</p>
		</div>
	{/if}

	<!-- Background Grid -->
	<div class="pointer-events-none absolute inset-0 z-0 opacity-20">
		<div
			class="h-full w-full bg-[radial-gradient(#ffffff15_1px,transparent_1px)] bg-size-[32px_32px]"
		></div>
	</div>
</div>
