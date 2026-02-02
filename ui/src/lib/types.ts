export type SymbolKind = 'namespace' | 'container' | 'callable' | 'value' | 'document';

export interface Symbol {
    uri: string;
    kind: SymbolKind;
    name: string;
    path: string;
    line_start: number;
    line_end: number;
    doc: string | null;
    signature: string | null;
    content: string;
}

export interface Edge {
    from_uri: string;
    to_uri: string;
    kind: string;
    confidence: number;
}

export interface ImpactResult {
    symbol: Symbol;
    depth: number;
    edge_kind: string;
    confidence: number;
}
