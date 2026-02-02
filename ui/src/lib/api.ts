const API_BASE = import.meta.env.DEV ? 'http://localhost:3000' : '';

export interface Symbol {
    uri: string;
    name: string;
    kind: string;
    path: string;
    line_start: number;
    doc?: string;
    signature?: string;
    content: string;
}

export interface SearchResult {
    name: string;
    uri: string;
    kind: string;
    path: string;
    line: number;
    score: number;
    content: string;
}

export interface GraphParams {
    uri: string;
    depth?: number;
}

export interface GraphResult {
    nodes: Symbol[];
    edges: {
        from_uri: { uri: string };
        to_uri: { uri: string };
        kind: string;
    }[];
}

export interface SymbolResult {
    symbol: Symbol;
    edges_out: any[];
    edges_in: any[];
}

export async function getStats() {
    const res = await fetch(`${API_BASE}/stats`);
    return res.json();
}

export async function search(query: string) {
    if (!query) return [];
    const res = await fetch(`${API_BASE}/search?q=${encodeURIComponent(query)}`);
    return res.json() as Promise<SearchResult[]>;
}

export async function getSymbol(uri: string) {
    const res = await fetch(`${API_BASE}/symbol?uri=${encodeURIComponent(uri)}`);
    return res.json() as Promise<SymbolResult | null>;
}

export async function getGraph(uri: string) {
    const res = await fetch(`${API_BASE}/graph?uri=${encodeURIComponent(uri)}`);
    return res.json() as Promise<GraphResult>;
}
