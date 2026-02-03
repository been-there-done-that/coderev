<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import type { GraphData, GraphNode } from '$lib/explore.svelte';

	interface Props {
		data: GraphData;
		selectedUri: string | null;
		highlightedLinks: Set<string>;
		onNodeClick: (node: GraphNode) => void;
	}

	let { data, selectedUri, highlightedLinks, onNodeClick }: Props = $props();

	let container: HTMLDivElement;
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	let graph: any = null;

	// Color constants
	const LINK_COLOR_CALLER = '#3b82f6';
	const LINK_COLOR_CALLEE = '#22c55e';
	const LINK_COLOR_DEFAULT = 'rgba(255,255,255,0.15)';
	const NODE_HIGHLIGHT_COLOR = '#f59e0b';

	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	function getLinkKey(link: any): string {
		const sourceId = typeof link.source === 'string' ? link.source : link.source?.id;
		const targetId = typeof link.target === 'string' ? link.target : link.target?.id;
		return `${sourceId}-${targetId}`;
	}

	async function initGraph() {
		if (!container) return;

		// Dynamic import for 3d-force-graph (works better with SSR)
		const ForceGraph3DModule = await import('3d-force-graph');
		const ForceGraph3D = ForceGraph3DModule.default;

		graph = new ForceGraph3D(container)
			.backgroundColor('rgba(0,0,0,0)')
			// eslint-disable-next-line @typescript-eslint/no-explicit-any
			.nodeLabel(
				(
					node: any
				) => `<div style="padding:8px;background:rgba(0,0,0,0.8);border-radius:8px;color:white;">
				<strong>${node.name}</strong><br/>
				<span style="opacity:0.7;font-size:11px;">${node.kind}</span><br/>
				<span style="opacity:0.5;font-size:10px;">${node.path}</span>
			</div>`
			)
			// eslint-disable-next-line @typescript-eslint/no-explicit-any
			.nodeColor((node: any) => {
				if (node.id === selectedUri) return NODE_HIGHLIGHT_COLOR;
				return node.color || '#ffffff';
			})
			// eslint-disable-next-line @typescript-eslint/no-explicit-any
			.nodeVal((node: any) => node.val || 1)
			.nodeOpacity(0.9)
			// eslint-disable-next-line @typescript-eslint/no-explicit-any
			.linkColor((link: any) => {
				const linkKey = getLinkKey(link);
				if (highlightedLinks.has(linkKey)) {
					return link.kind === 'caller' ? LINK_COLOR_CALLER : LINK_COLOR_CALLEE;
				}
				return LINK_COLOR_DEFAULT;
			})
			// eslint-disable-next-line @typescript-eslint/no-explicit-any
			.linkWidth((link: any) => {
				const linkKey = getLinkKey(link);
				return highlightedLinks.has(linkKey) ? 2 : 0.5;
			})
			.linkOpacity(0.6)
			// eslint-disable-next-line @typescript-eslint/no-explicit-any
			.linkDirectionalParticles((link: any) => {
				const linkKey = getLinkKey(link);
				return highlightedLinks.has(linkKey) ? 4 : 0;
			})
			.linkDirectionalParticleSpeed(0.005)
			.linkDirectionalParticleWidth(2)
			// eslint-disable-next-line @typescript-eslint/no-explicit-any
			.onNodeClick((node: any) => {
				if (node) onNodeClick(node as GraphNode);
			})
			.graphData(data);

		// Configure force simulation for better spreading
		const chargeForce = graph.d3Force('charge');
		if (chargeForce) chargeForce.strength(-120);
		const linkForce = graph.d3Force('link');
		if (linkForce) linkForce.distance(80);
	}

	$effect(() => {
		if (graph && data) {
			graph.graphData(data);
		}
	});

	$effect(() => {
		if (graph) {
			// Force refresh when selection or highlights change
			graph.nodeColor(graph.nodeColor());
			graph.linkColor(graph.linkColor());
			graph.linkWidth(graph.linkWidth());
			graph.linkDirectionalParticles(graph.linkDirectionalParticles());
		}
	});

	onMount(() => {
		initGraph();

		// Handle resize
		const resizeObserver = new ResizeObserver(() => {
			if (graph && container) {
				graph.width(container.clientWidth);
				graph.height(container.clientHeight);
			}
		});
		resizeObserver.observe(container);

		return () => {
			resizeObserver.disconnect();
		};
	});

	onDestroy(() => {
		if (graph && graph._destructor) {
			graph._destructor();
		}
	});
</script>

<div bind:this={container} class="h-full w-full"></div>

<style>
	div :global(canvas) {
		outline: none;
	}
</style>
