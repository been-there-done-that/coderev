import { type Symbol } from '$lib/types';

export interface GraphNode {
    id: string;
    name: string;
    kind: string;
    path: string;
    val: number;
    color?: string;
    symbol: Symbol;
}

export interface GraphLink {
    source: string;
    target: string;
    kind: 'caller' | 'callee';
    color?: string;
}

export interface GraphData {
    nodes: GraphNode[];
    links: GraphLink[];
}

// Color palette for symbol kinds
const kindColors: Record<string, string> = {
    callable: '#8b5cf6',
    container: '#06b6d4',
    value: '#22c55e',
    namespace: '#f59e0b',
    document: '#64748b',
};

function symbolToNode(symbol: Symbol): GraphNode {
    return {
        id: symbol.uri,
        name: symbol.name,
        kind: symbol.kind,
        path: symbol.path,
        val: symbol.kind === 'callable' ? 2 : 1,
        color: kindColors[symbol.kind] || kindColors.document,
        symbol
    };
}

class ExploreStateClass {
    symbols = $state<Symbol[]>([]);
    graphData = $state<GraphData>({ nodes: [], links: [] });
    selectedUri = $state<string | null>(null);
    highlightedLinks = $state<Set<string>>(new Set());
    isLoading = $state(false);
    query = $state('');

    async loadInitialData() {
        this.isLoading = true;
        try {
            const response = await fetch(
                `http://127.0.0.1:3000/api/v1/search?query=&limit=1000`
            );
            if (!response.ok) throw new Error('Failed to load symbols');
            const symbols: Symbol[] = await response.json();
            console.log('[explore] Loaded symbols:', symbols.length);
            this.symbols = symbols;
            this.graphData = {
                nodes: symbols.map(symbolToNode),
                links: []
            };
        } catch (e) {
            console.error('Failed to load initial data:', e);
        } finally {
            this.isLoading = false;
        }
    }

    async search(query: string) {
        this.isLoading = true;
        this.query = query;
        this.selectedUri = null;
        this.highlightedLinks = new Set();
        try {
            const response = await fetch(
                `http://127.0.0.1:3000/api/v1/search?query=${encodeURIComponent(query)}&limit=1000`
            );
            if (!response.ok) throw new Error('Search failed');
            const symbols: Symbol[] = await response.json();
            console.log('[explore] Search results:', symbols.length);
            this.symbols = symbols;
            // Create completely new graphData object
            this.graphData = {
                nodes: symbols.map(symbolToNode),
                links: []
            };
            console.log('[explore] Updated graphData nodes:', this.graphData.nodes.length);
        } catch (e) {
            console.error('Search failed:', e);
        } finally {
            this.isLoading = false;
        }
    }

    async selectNode(uri: string) {
        console.log('[explore] selectNode called with:', uri);
        this.selectedUri = uri;
        this.highlightedLinks = new Set();

        try {
            const response = await fetch(
                `http://127.0.0.1:3000/api/v1/trace?uri=${encodeURIComponent(uri)}&depth=1`
            );
            if (!response.ok) throw new Error('Failed to fetch trace');
            const data = await response.json();
            console.log('[explore] Trace response:', data);

            const callers: Symbol[] = data.callers || [];
            const callees: Symbol[] = data.callees || [];

            console.log('[explore] Callers:', callers.length, 'Callees:', callees.length);

            const newLinks: GraphLink[] = [];
            const existingNodeIds = new Set(this.graphData.nodes.map((n) => n.id));
            const newNodes: GraphNode[] = [];

            // Add caller links
            for (const caller of callers) {
                if (!existingNodeIds.has(caller.uri)) {
                    newNodes.push(symbolToNode(caller));
                    existingNodeIds.add(caller.uri);
                }
                newLinks.push({
                    source: caller.uri,
                    target: uri,
                    kind: 'caller',
                    color: '#3b82f6'
                });
            }

            // Add callee links
            for (const callee of callees) {
                if (!existingNodeIds.has(callee.uri)) {
                    newNodes.push(symbolToNode(callee));
                    existingNodeIds.add(callee.uri);
                }
                newLinks.push({
                    source: uri,
                    target: callee.uri,
                    kind: 'callee',
                    color: '#22c55e'
                });
            }

            console.log('[explore] New nodes:', newNodes.length, 'New links:', newLinks.length);

            // Create completely new graphData object to trigger reactivity
            this.graphData = {
                nodes: [...this.graphData.nodes, ...newNodes],
                links: [...this.graphData.links, ...newLinks]
            };

            console.log('[explore] Updated graphData - nodes:', this.graphData.nodes.length, 'links:', this.graphData.links.length);

            // Highlight the new links
            this.highlightedLinks = new Set(
                newLinks.map((l) => `${l.source}-${l.target}`)
            );
        } catch (e) {
            console.error('Failed to select node:', e);
        }
    }

    clearSelection() {
        this.selectedUri = null;
        this.highlightedLinks = new Set();
    }

    getSelectedSymbol(): Symbol | null {
        if (!this.selectedUri) return null;
        return this.symbols.find((s) => s.uri === this.selectedUri) || null;
    }
}

export const exploreState = new ExploreStateClass();
