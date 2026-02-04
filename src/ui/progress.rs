use crate::ui::progress_message::{ProgressMessage, ProgressPhase};
use crate::ui::theme;
use crate::ui::Icons;
use indicatif::{HumanDuration, MultiProgress, ProgressBar};
use owo_colors::OwoColorize;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

pub struct ProgressManager {
    mp: MultiProgress,
    parsing: ProgressBar,
    linking: ProgressBar,
    embedding: ProgressBar,
    _handle: thread::JoinHandle<()>,
}

impl ProgressManager {
    pub fn new(total_files: usize) -> (Self, crossbeam::channel::Sender<ProgressMessage>) {
        let (tx, rx) = crossbeam::channel::unbounded::<ProgressMessage>();

        let mp = MultiProgress::new();

        let parsing = mp.add(ProgressBar::new(total_files as u64).with_message("Parsing files"));
        let parsing = if console::Term::stdout().is_term() {
            parsing
        } else {
            ProgressBar::hidden()
        };

        let linking = mp.add(ProgressBar::new_spinner().with_message("Linking symbols"));
        let linking = if console::Term::stdout().is_term() {
            linking
        } else {
            ProgressBar::hidden()
        };

        let embedding = mp.add(ProgressBar::new_spinner().with_message("Generating embeddings"));
        let embedding = if console::Term::stdout().is_term() {
            embedding
        } else {
            ProgressBar::hidden()
        };

        let parsed_count_clone = Arc::new(AtomicUsize::new(0));
        let parsing_clone = parsing.clone();
        let linking_clone = linking.clone();
        let embedding_clone = embedding.clone();

        let handle = thread::spawn(move || {
            for msg in rx {
                match msg {
                    ProgressMessage::Progress {
                        phase: ProgressPhase::Parsing,
                        current: _,
                        file,
                    } => {
                        parsed_count_clone.fetch_add(1, Ordering::Relaxed);
                        parsing_clone.inc(1);
                        if let Some(ref f) = file {
                            parsing_clone.set_message(format!("Parsing: {}", f));
                        }
                    }
                    ProgressMessage::Started {
                        phase: ProgressPhase::Linking,
                        total: _,
                    } => {
                        linking_clone.enable_steady_tick(Duration::from_millis(100));
                    }
                    ProgressMessage::Started {
                        phase: ProgressPhase::Embedding,
                        total: _,
                    } => {
                        embedding_clone.enable_steady_tick(Duration::from_millis(100));
                    }
                    ProgressMessage::Started {
                        phase: ProgressPhase::Semantic,
                        total: _,
                    } => {
                        // Semantic phase - no dedicated progress bar
                    }
                    ProgressMessage::Finished {
                        phase: ProgressPhase::Parsing,
                    } => {
                        parsing_clone.finish_with_message("Done");
                    }
                    ProgressMessage::Finished {
                        phase: ProgressPhase::Linking,
                    } => {
                        linking_clone.finish_with_message("Done");
                    }
                    ProgressMessage::Finished {
                        phase: ProgressPhase::Embedding,
                    } => {
                        embedding_clone.finish_with_message("Done");
                    }
                    ProgressMessage::Finished {
                        phase: ProgressPhase::Semantic,
                    } => {
                        // Semantic phase complete - no dedicated progress bar
                    }
                    _ => {}
                }
            }
        });

        (
            Self {
                mp,
                parsing,
                linking,
                embedding,
                _handle: handle,
            },
            tx,
        )
    }

    pub fn set_parsing_message(&self, msg: &str) {
        self.parsing.set_message(msg.to_string());
    }

    pub fn inc_parsing(&self) {
        self.parsing.inc(1);
    }

    pub fn finish_parsing(&self) {
        self.parsing.finish_with_message("Done");
    }

    pub fn start_linking(&self) {
        self.linking.enable_steady_tick(Duration::from_millis(100));
    }

    pub fn finish_linking(&self) {
        self.linking.finish_with_message("Done");
    }

    pub fn start_embedding(&self) {
        self.embedding
            .enable_steady_tick(Duration::from_millis(100));
    }

    pub fn set_embedding_message(&self, msg: &str) {
        self.embedding.set_message(msg.to_string());
    }

    pub fn finish_embedding(&self) {
        self.embedding.finish_with_message("Done");
    }

    pub fn clear(&self) {
        self.mp.clear().ok();
    }

    pub fn finish_with_summary(
        &self,
        duration: Duration,
        files: usize,
        symbols: usize,
        edges: usize,
    ) {
        self.clear();
        println!();
        println!(
            "{} {}",
            Icons::CHECK.style(theme().success.clone()),
            format!("Complete in {}", HumanDuration(duration)).style(theme().success.clone())
        );
        println!(
            "  {} {}  {} {}  {} {}",
            Icons::FILE.style(theme().info.clone()),
            files,
            Icons::PACKAGE.style(theme().info.clone()),
            symbols,
            Icons::LINK.style(theme().info.clone()),
            edges
        );
    }
}

pub struct Spinner {
    pb: ProgressBar,
}

impl Spinner {
    pub fn new(message: &str) -> Self {
        let pb = ProgressBar::new_spinner();
        pb.set_message(message.to_string());
        if console::Term::stdout().is_term() {
            pb.enable_steady_tick(Duration::from_millis(100));
        }
        Self { pb }
    }

    pub fn set_message(&self, msg: &str) {
        self.pb.set_message(msg.to_string());
    }

    pub fn finish_with_message(&self, msg: &str) {
        self.pb.finish_with_message(msg.to_string());
    }
}
