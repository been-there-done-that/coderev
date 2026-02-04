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
	let isInitialized = $state(false);

	// Color constants
	const LINK_COLOR_CALLER = '#3b82f6';
	const LINK_COLOR_CALLEE = '#22c55e';
	const LINK_COLOR_DEFAULT = 'rgba(255,255,255,0.3)';
	const NODE_HIGHLIGHT_COLOR = '#f59e0b';

	// Compute data signature to detect changes
	let dataKey = $derived(`${data.nodes.length}-${data.links.length}`);

	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	function getLinkKey(link: any): string {
		const sourceId = typeof link.source === 'string' ? link.source : link.source?.id;
		const targetId = typeof link.target === 'string' ? link.target : link.target?.id;
		return `${sourceId}-${targetId}`;
	}

	async function initGraph() {
		if (!container) return;

		const ForceGraph3DModule = await import('3d-force-graph');
		const ForceGraph3D = ForceGraph3DModule.default;

		graph = new ForceGraph3D(container)
			.backgroundColor('rgba(0,0,0,0)')
			// eslint-disable-next-line @typescript-eslint/no-explicit-any
			.nodeLabel(
				(
					node: any
				) => `<div style="padding:8px;background:rgba(0,0,0,0.9);border-radius:8px;color:white;border:1px solid rgba(255,255,255,0.1);">
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
			.linkColor(LINK_COLOR_DEFAULT)
			.linkWidth(1)
			.linkOpacity(0.8)
			.linkDirectionalArrowLength(4)
			.linkDirectionalArrowRelPos(1)
			// eslint-disable-next-line @typescript-eslint/no-explicit-any
			.onNodeClick((node: any) => {
				if (node) {
					onNodeClick(node as GraphNode);
					const distance = 150;
					const distRatio = 1 + distance / Math.hypot(node.x, node.y, node.z);
					graph.cameraPosition(
						{
							x: node.x * distRatio,
							y: node.y * distRatio,
							z: node.z * distRatio
						},
						node,
						1000
					);
				}
			})
			.graphData({ nodes: [...data.nodes], links: [...data.links] });

		const chargeForce = graph.d3Force('charge');
		if (chargeForce) chargeForce.strength(-80);

		const linkForce = graph.d3Force('link');
		if (linkForce) {
			linkForce.distance(50);
			linkForce.strength(1);
		}

		const centerForce = graph.d3Force('center');
		if (centerForce) centerForce.strength(0.05);

		isInitialized = true;
	}

	// Update graph data when dataKey changes (nodes or links count changed)
	$effect(() => {
		const key = dataKey; // Explicitly read derived to track
		console.log('[ForceGraph3D] dataKey changed:', key);
		if (graph && isInitialized) {
			console.log(
				'[ForceGraph3D] Updating graph with data:',
				data.nodes.length,
				'nodes,',
				data.links.length,
				'links'
			);
			graph.graphData({ nodes: [...data.nodes], links: [...data.links] });
		}
	});

	// Update visual properties when selection changes
	$effect(() => {
		const uri = selectedUri; // Explicitly track
		const highlights = highlightedLinks; // Explicitly track

		if (graph && isInitialized) {
			// eslint-disable-next-line @typescript-eslint/no-explicit-any
			graph.nodeColor((node: any) => {
				if (node.id === uri) return NODE_HIGHLIGHT_COLOR;
				return node.color || '#ffffff';
			});

			// eslint-disable-next-line @typescript-eslint/no-explicit-any
			graph.linkColor((link: any) => {
				const linkKey = getLinkKey(link);
				if (highlights.has(linkKey)) {
					return link.kind === 'caller' ? LINK_COLOR_CALLER : LINK_COLOR_CALLEE;
				}
				return LINK_COLOR_DEFAULT;
			});

			// eslint-disable-next-line @typescript-eslint/no-explicit-any
			graph.linkWidth((link: any) => {
				const linkKey = getLinkKey(link);
				return highlights.has(linkKey) ? 3 : 1;
			});

			// eslint-disable-next-line @typescript-eslint/no-explicit-any
			graph.linkDirectionalParticles((link: any) => {
				const linkKey = getLinkKey(link);
				return highlights.has(linkKey) ? 4 : 0;
			});

			graph.linkDirectionalParticleSpeed(0.008);
			graph.linkDirectionalParticleWidth(3);
		}
	});

	onMount(() => {
		initGraph();

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
