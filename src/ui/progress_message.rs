#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ProgressPhase {
    Parsing,
    Linking,
    Embedding,
    Semantic,
}

#[derive(Clone, Debug)]
pub enum ProgressMessage {
    Started {
        phase: ProgressPhase,
        total: usize,
    },
    Progress {
        phase: ProgressPhase,
        current: usize,
        file: Option<String>,
    },
    Finished {
        phase: ProgressPhase,
    },
    FileNew(String),
    FileModified(String),
    FileDeleted(String),
    Error(String),
    Exit,
}
