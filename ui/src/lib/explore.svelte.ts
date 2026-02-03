import { type Symbol } from '$lib/types';

export interface GraphNode {
    id: string;
    name: string;
    kind: string;
    path: string;
    val: number; // Size for the node
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

export interface ExploreState {
    symbols: Symbol[];
    graphData: GraphData;
    selectedUri: string | null;
    highlightedLinks: Set<string>;
    isLoading: boolean;
    query: string;
}

// Color palette for symbol kinds
const kindColors: Record<string, string> = {
    callable: '#8b5cf6', // violet
    container: '#06b6d4', // cyan
    value: '#22c55e', // green
    namespace: '#f59e0b', // amber
    document: '#64748b', // slate
};

function createExploreState() {
    let state = $state<ExploreState>({
        symbols: [],
        graphData: { nodes: [], links: [] },
        selectedUri: null,
        highlightedLinks: new Set(),
        isLoading: false,
        query: ''
    });

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

    function buildGraphFromSymbols(symbols: Symbol[]): GraphData {
        const nodes = symbols.map(symbolToNode);
        // Links will be populated when a node is selected
        return { nodes, links: [] };
    }

    return {
        get symbols() { return state.symbols; },
        get graphData() { return state.graphData; },
        get selectedUri() { return state.selectedUri; },
        get highlightedLinks() { return state.highlightedLinks; },
        get isLoading() { return state.isLoading; },
        get query() { return state.query; },
        set query(v) { state.query = v; },

        async loadInitialData() {
            state.isLoading = true;
            try {
                const response = await fetch(
                    `http://127.0.0.1:3000/api/v1/search?query=&limit=1000`
                );
                if (!response.ok) throw new Error('Failed to load symbols');
                const symbols: Symbol[] = await response.json();
                state.symbols = symbols;
                state.graphData = buildGraphFromSymbols(symbols);
            } catch (e) {
                console.error('Failed to load initial data:', e);
            } finally {
                state.isLoading = false;
            }
        },

        async search(query: string) {
            state.isLoading = true;
            state.query = query;
            state.selectedUri = null;
            state.highlightedLinks = new Set();
            try {
                const response = await fetch(
                    `http://127.0.0.1:3000/api/v1/search?query=${encodeURIComponent(query)}&limit=1000`
                );
                if (!response.ok) throw new Error('Search failed');
                const symbols: Symbol[] = await response.json();
                state.symbols = symbols;
                state.graphData = buildGraphFromSymbols(symbols);
            } catch (e) {
                console.error('Search failed:', e);
            } finally {
                state.isLoading = false;
            }
        },

        async selectNode(uri: string) {
            state.selectedUri = uri;
            state.highlightedLinks = new Set();

            try {
                const response = await fetch(
                    `http://127.0.0.1:3000/api/v1/trace?uri=${encodeURIComponent(uri)}&depth=1`
                );
                if (!response.ok) throw new Error('Failed to fetch trace');
                const data = await response.json();

                const callers: Symbol[] = data.callers || [];
                const callees: Symbol[] = data.callees || [];

                // Build new links for the selected node
                const newLinks: GraphLink[] = [];
                const existingNodeIds = new Set(state.graphData.nodes.map((n) => n.id));
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
                        color: '#3b82f6' // blue for callers
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
                        color: '#22c55e' // green for callees
                    });
                }

                // Update graph data
                state.graphData = {
                    nodes: [...state.graphData.nodes, ...newNodes],
                    links: [...state.graphData.links, ...newLinks]
                };

                // Highlight the new links
                state.highlightedLinks = new Set(
                    newLinks.map((l) => `${l.source}-${l.target}`)
                );
            } catch (e) {
                console.error('Failed to select node:', e);
            }
        },

        clearSelection() {
            state.selectedUri = null;
            state.highlightedLinks = new Set();
        },

        getSelectedSymbol(): Symbol | null {
            if (!state.selectedUri) return null;
            return state.symbols.find((s) => s.uri === state.selectedUri) || null;
        }
    };
}

export const exploreState = createExploreState();
