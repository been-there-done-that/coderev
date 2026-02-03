import { type Symbol, type ImpactResult, type AnalysisResult } from '$lib/types';

export interface GraphState {
    query: string;
    results: any[];
    selectedSymbol: Symbol | null;
    impact: ImpactResult[];
    analysis: AnalysisResult | null;
    isLoading: boolean;
    isImpactLoading: boolean;
    isAnalysisLoading: boolean;
    error: string | null;
    requestId: string | null;
}

function createGraphState() {
    let state = $state<GraphState>({
        query: '',
        results: [],
        selectedSymbol: null,
        impact: [],
        analysis: null,
        isLoading: false,
        isImpactLoading: false,
        isAnalysisLoading: false,
        error: null,
        requestId: null
    });

    return {
        get query() { return state.query; },
        set query(v) { state.query = v; },

        get results() { return state.results; },
        get selectedSymbol() { return state.selectedSymbol; },
        get impact() { return state.impact; },
        get analysis() { return state.analysis; },
        get isLoading() { return state.isLoading; },
        get isImpactLoading() { return state.isImpactLoading; },
        get isAnalysisLoading() { return state.isAnalysisLoading; },
        get error() { return state.error; },
        get requestId() { return state.requestId; },

        set selectedSymbol(s) { state.selectedSymbol = s; state.impact = []; state.analysis = null; },

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
            state.analysis = null;
            // Fetch trace is handled in TracePanel $effect for now
            this.fetchAnalysis();
        },

        async fetchAnalysis() {
            if (!state.selectedSymbol) return;
            state.isAnalysisLoading = true;
            state.analysis = null;

            try {
                const response = await fetch(`http://127.0.0.1:3000/api/v1/analyze?uri=${encodeURIComponent(state.selectedSymbol.uri)}`);
                if (!response.ok) throw new Error('Analysis failed');
                state.analysis = await response.json();
            } catch (e: any) {
                console.error('Failed to fetch analysis', e);
            } finally {
                state.isAnalysisLoading = false;
            }
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
