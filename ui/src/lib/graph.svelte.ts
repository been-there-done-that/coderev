import { type Symbol, type ImpactResult } from '$lib/types';

export interface GraphState {
    query: string;
    results: any[];
    selectedSymbol: Symbol | null;
    impact: ImpactResult[];
    isLoading: boolean;
    isImpactLoading: boolean;
    error: string | null;
    requestId: string | null;
}

function createGraphState() {
    let state = $state<GraphState>({
        query: '',
        results: [],
        selectedSymbol: null,
        impact: [],
        isLoading: false,
        isImpactLoading: false,
        error: null,
        requestId: null
    });

    return {
        get query() { return state.query; },
        set query(v) { state.query = v; },

        get results() { return state.results; },
        get selectedSymbol() { return state.selectedSymbol; },
        get impact() { return state.impact; },
        get isLoading() { return state.isLoading; },
        get isImpactLoading() { return state.isImpactLoading; },
        get error() { return state.error; },
        get requestId() { return state.requestId; },

        set selectedSymbol(s) { state.selectedSymbol = s; state.impact = []; },

        async search(query: string) {
            state.isLoading = true;
            state.error = null;
            state.query = query;

            try {
                const id = crypto.randomUUID();
                state.requestId = id;

                const response = await fetch(`http://127.0.0.1:3000/api/v1/search?query=${encodeURIComponent(query)}&limit=1000`, {
                    headers: { 'X-Request-ID': id }
                });

                if (!response.ok) throw new Error('Search failed');
                state.results = await response.json();
            } catch (e: any) {
                state.error = e.message;
            } finally {
                state.isLoading = false;
            }
        },

        async selectSymbol(symbol: Symbol) {
            state.selectedSymbol = symbol;
            state.impact = [];
            // Fetch trace is handled in TracePanel $effect for now
        },

        async fetchImpact() {
            if (!state.selectedSymbol) return;
            state.isImpactLoading = true;
            state.error = null;

            try {
                const id = crypto.randomUUID();
                const response = await fetch(`http://127.0.0.1:3000/api/v1/impact?uri=${encodeURIComponent(state.selectedSymbol.uri)}&depth=3`, {
                    headers: { 'X-Request-ID': id }
                });

                if (!response.ok) throw new Error('Impact analysis failed');
                state.impact = await response.json();
            } catch (e: any) {
                state.error = e.message;
            } finally {
                state.isImpactLoading = false;
            }
        }
    };
}

export const graphState = createGraphState();
